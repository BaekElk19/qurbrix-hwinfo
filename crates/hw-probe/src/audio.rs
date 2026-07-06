use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, AudioInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::parse_proc_asound_cards;
use std::path::Path;

pub struct AudioProbe;

#[async_trait]
impl Probe for AudioProbe {
    fn name(&self) -> &'static str {
        "audio"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Audio]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.read_file(Path::new("/proc/asound/cards")).await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_audio_cards(ctx).await;
            return fallback;
        }
        let mut devices = Vec::new();
        for card in parse_proc_asound_cards(&result.stdout) {
            let enrichment = audio_enrichment(ctx, card.index).await;
            let device = Device::new(
                device_id::other("audio:card", &card.index.to_string()),
                DeviceKind::Audio,
                card.name
                    .clone()
                    .unwrap_or_else(|| format!("Audio card {}", card.index)),
                DeviceProperties::Audio(AudioInfo {
                    card_index: Some(card.index),
                    card_name: card.name,
                    codec: enrichment.codec.clone(),
                    subsystem: enrichment.subsystem.clone(),
                    ..Default::default()
                }),
            )
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
            devices.push(apply_audio_enrichment(device, enrichment));
        }
        ProbeResult::with_devices(devices)
    }
}

struct AudioEnrichment {
    driver: Option<String>,
    vendor: Option<String>,
    codec: Option<String>,
    codec_source: Option<String>,
    subsystem: Option<String>,
    sysfs_source: String,
    sysfs_contributed: bool,
}

async fn probe_sysfs_audio_cards(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    let mut cards = ctx
        .runner
        .glob("/sys/class/sound/card*")
        .await
        .paths
        .into_iter()
        .filter_map(|path| {
            let index = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(audio_card_index)?;
            Some((index, path))
        })
        .collect::<Vec<_>>();
    cards.sort_by_key(|(index, _)| *index);

    for (index, path) in cards {
        let card_name = read_trimmed(ctx, &path.join("id"))
            .await
            .unwrap_or_else(|| format!("Audio card {index}"));
        let enrichment = audio_enrichment(ctx, index).await;
        let device = Device::new(
            device_id::other("audio:card", &index.to_string()),
            DeviceKind::Audio,
            card_name.clone(),
            DeviceProperties::Audio(AudioInfo {
                card_index: Some(index),
                card_name: Some(card_name),
                codec: enrichment.codec.clone(),
                subsystem: enrichment.subsystem.clone(),
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source: path.display().to_string(),
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
        devices.push(apply_audio_enrichment(device, enrichment));
    }

    devices
}

async fn audio_enrichment(ctx: &ProbeContext<'_>, index: u32) -> AudioEnrichment {
    let sysfs_path = Path::new("/sys/class/sound").join(format!("card{index}"));
    let driver = read_trimmed(ctx, &sysfs_path.join("device/uevent"))
        .await
        .and_then(|uevent| parse_uevent_value(&uevent, "DRIVER"));
    let vendor = read_trimmed(ctx, &sysfs_path.join("device/vendor"))
        .await
        .and_then(normalize_audio_vendor_id);
    let subsystem_vendor = read_trimmed(ctx, &sysfs_path.join("device/subsystem_vendor"))
        .await
        .map(normalize_hex_id);
    let subsystem_device = read_trimmed(ctx, &sysfs_path.join("device/subsystem_device"))
        .await
        .map(normalize_hex_id);
    let subsystem = match (subsystem_vendor, subsystem_device) {
        (Some(vendor), Some(device)) => Some(format!("{vendor}:{device}")),
        _ => None,
    };
    let (codec, codec_source) = read_audio_codec(ctx, index).await;

    AudioEnrichment {
        sysfs_source: sysfs_path.display().to_string(),
        sysfs_contributed: driver.is_some() || vendor.is_some() || subsystem.is_some(),
        driver,
        vendor,
        codec,
        codec_source,
        subsystem,
    }
}

async fn read_audio_codec(ctx: &ProbeContext<'_>, index: u32) -> (Option<String>, Option<String>) {
    let mut paths = ctx
        .runner
        .glob(&format!("/proc/asound/card{index}/codec#*"))
        .await
        .paths;
    paths.sort();

    for path in paths {
        let Some(contents) = read_trimmed(ctx, &path).await else {
            continue;
        };
        if let Some(codec) = parse_uevent_value(&contents, "Codec") {
            return (Some(codec), Some(path.display().to_string()));
        }
    }

    (None, None)
}

fn apply_audio_enrichment(mut device: Device, enrichment: AudioEnrichment) -> Device {
    if let Some(vendor) = enrichment.vendor {
        device.vendor = Some(vendor);
    }

    if let Some(driver) = enrichment.driver {
        device = device.with_driver(DriverInfo {
            name: Some(driver),
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
    }

    if enrichment.sysfs_contributed
        && !device
            .sources
            .iter()
            .any(|source| source.source == enrichment.sysfs_source)
    {
        device = device.with_source(SourceEvidence {
            source: enrichment.sysfs_source,
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    if let Some(source) = enrichment.codec_source {
        device = device.with_source(SourceEvidence {
            source,
            kind: SourceKind::Procfs,
            status: SourceStatus::Success,
            summary: None,
        });
    }

    device
}

fn parse_uevent_value(input: &str, key: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':').or_else(|| line.split_once('='))?;
        (candidate == key && !value.trim().is_empty()).then(|| value.trim().to_string())
    })
}

fn normalize_hex_id(value: String) -> String {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(&value)
        .to_ascii_lowercase()
}

fn normalize_audio_vendor_id(value: String) -> Option<String> {
    match normalize_hex_id(value).as_str() {
        "8086" => Some("Intel".to_string()),
        "1002" | "1022" => Some("AMD".to_string()),
        "10ec" => Some("Realtek".to_string()),
        "14f1" => Some("Conexant".to_string()),
        "1102" => Some("Creative".to_string()),
        _ => None,
    }
}

fn audio_card_index(name: &str) -> Option<u32> {
    let suffix = name.strip_prefix("card")?;
    if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

async fn read_trimmed(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}
