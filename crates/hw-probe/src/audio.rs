use crate::{
    sysfs_pci::{pci_bus_from_uevent, read_kernel_modules},
    Probe, ProbeContext, ProbeResult,
};
use async_trait::async_trait;
use hw_model::{
    device_id, AudioInfo, BusInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::{
    parse_hwinfo_sound, parse_lshw_multimedia, parse_pactl_card_profiles, parse_proc_asound_cards,
    HwinfoSoundRecord, LshwMultimediaRecord,
};
use hw_source::CommandSpec;
use std::{collections::HashMap, path::Path};

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
        let lshw = audio_lshw_records(ctx).await;
        let hwinfo = audio_hwinfo_records(ctx).await;
        let profiles = audio_profile_records(ctx).await;
        let mut devices = Vec::new();
        for card in parse_proc_asound_cards(&result.stdout) {
            let enrichment = audio_enrichment(ctx, card.index).await;
            let card_profiles = profiles.for_card(card.index);
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
                    profiles: card_profiles.clone(),
                }),
            )
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
            let device = apply_audio_enrichment(device, enrichment);
            let device = apply_audio_profile_source(device, &profiles, &card_profiles);
            let device = apply_audio_lshw_enrichment(device, &lshw);
            devices.push(apply_audio_hwinfo_enrichment(device, &hwinfo, card.index));
        }
        ProbeResult::with_devices(devices)
    }
}

#[derive(Default)]
struct AudioLshwRecords {
    source: String,
    by_pci_address: HashMap<String, LshwMultimediaRecord>,
}

#[derive(Default)]
struct AudioHwinfoRecords {
    source: String,
    by_pci_address: HashMap<String, HwinfoSoundRecord>,
    by_card_index: HashMap<u32, HwinfoSoundRecord>,
}

#[derive(Default)]
struct AudioProfileRecords {
    source: String,
    by_card_index: HashMap<u32, Vec<String>>,
}

impl AudioProfileRecords {
    fn for_card(&self, index: u32) -> Vec<String> {
        self.by_card_index.get(&index).cloned().unwrap_or_default()
    }
}

struct AudioEnrichment {
    driver: Option<String>,
    modules: Vec<String>,
    bus: Option<BusInfo>,
    vendor: Option<String>,
    codec: Option<String>,
    codec_source: Option<String>,
    subsystem: Option<String>,
    sysfs_source: String,
    sysfs_contributed: bool,
}

async fn probe_sysfs_audio_cards(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let lshw = audio_lshw_records(ctx).await;
    let hwinfo = audio_hwinfo_records(ctx).await;
    let profiles = audio_profile_records(ctx).await;
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
        let card_profiles = profiles.for_card(index);
        let device = Device::new(
            device_id::other("audio:card", &index.to_string()),
            DeviceKind::Audio,
            card_name.clone(),
            DeviceProperties::Audio(AudioInfo {
                card_index: Some(index),
                card_name: Some(card_name),
                codec: enrichment.codec.clone(),
                subsystem: enrichment.subsystem.clone(),
                profiles: card_profiles.clone(),
            }),
        )
        .with_source(SourceEvidence {
            source: path.display().to_string(),
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
        let device = apply_audio_enrichment(device, enrichment);
        let device = apply_audio_profile_source(device, &profiles, &card_profiles);
        let device = apply_audio_lshw_enrichment(device, &lshw);
        devices.push(apply_audio_hwinfo_enrichment(device, &hwinfo, index));
    }

    devices
}

async fn audio_lshw_records(ctx: &ProbeContext<'_>) -> AudioLshwRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("lshw", ["-class", "multimedia"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return AudioLshwRecords::default();
    }

    let by_pci_address = parse_lshw_multimedia(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((lshw_pci_address(record.bus_info.as_deref()?)?, record)))
        .collect();
    AudioLshwRecords {
        source: result.source,
        by_pci_address,
    }
}

async fn audio_hwinfo_records(ctx: &ProbeContext<'_>) -> AudioHwinfoRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("hwinfo", ["--sound"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return AudioHwinfoRecords::default();
    }

    let mut records = AudioHwinfoRecords {
        source: result.source,
        ..Default::default()
    };
    for record in parse_hwinfo_sound(&result.stdout) {
        if let Some(address) = record.pci_address.clone() {
            records.by_pci_address.insert(address, record.clone());
        }
        if let Some(index) = record.card_index {
            records.by_card_index.insert(index, record);
        }
    }
    records
}

async fn audio_profile_records(ctx: &ProbeContext<'_>) -> AudioProfileRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("pactl", ["list", "cards"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return AudioProfileRecords::default();
    }

    let by_card_index = parse_pactl_card_profiles(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.card_index?, record.profiles)))
        .filter(|(_, profiles)| !profiles.is_empty())
        .collect();
    AudioProfileRecords {
        source: result.source,
        by_card_index,
    }
}

async fn audio_enrichment(ctx: &ProbeContext<'_>, index: u32) -> AudioEnrichment {
    let sysfs_path = Path::new("/sys/class/sound").join(format!("card{index}"));
    let uevent = read_trimmed(ctx, &sysfs_path.join("device/uevent")).await;
    let driver = uevent
        .as_deref()
        .and_then(|uevent| parse_uevent_value(uevent, "DRIVER"));
    let bus = uevent.as_deref().and_then(pci_bus_from_uevent);
    let modules = read_kernel_modules(ctx, &sysfs_path.join("device")).await;
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
        sysfs_contributed: driver.is_some()
            || !modules.is_empty()
            || bus.is_some()
            || vendor.is_some()
            || subsystem.is_some(),
        driver,
        modules,
        bus,
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

    if let Some(bus) = enrichment.bus {
        device = device.with_bus(bus);
    }

    if enrichment.driver.is_some() || !enrichment.modules.is_empty() {
        device = device.with_driver(DriverInfo {
            name: enrichment.driver,
            version: None,
            modules: enrichment.modules,
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

fn apply_audio_profile_source(
    mut device: Device,
    profiles: &AudioProfileRecords,
    card_profiles: &[String],
) -> Device {
    if card_profiles.is_empty()
        || profiles.source.is_empty()
        || device
            .sources
            .iter()
            .any(|source| source.source == profiles.source)
    {
        return device;
    }
    device = device.with_source(SourceEvidence {
        source: profiles.source.clone(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    });
    device
}

fn apply_audio_hwinfo_enrichment(
    mut device: Device,
    hwinfo: &AudioHwinfoRecords,
    card_index: u32,
) -> Device {
    let record = device
        .bus
        .as_ref()
        .and_then(audio_pci_address)
        .and_then(|address| hwinfo.by_pci_address.get(address))
        .or_else(|| hwinfo.by_card_index.get(&card_index));
    let Some(record) = record else {
        return device;
    };
    let mut contributed = false;

    if device.vendor.is_none() && record.vendor.is_some() {
        device.vendor = record.vendor.clone();
        contributed = true;
    }
    if device.model.is_none() {
        let model = record.model.clone().or_else(|| record.device.clone());
        if model.is_some() {
            device.model = model;
            contributed = true;
        }
    }
    if record.driver.is_some() || !record.driver_modules.is_empty() {
        let mut driver = device.driver.take().unwrap_or(DriverInfo {
            name: None,
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
        let original = driver.clone();
        driver.name = driver.name.or_else(|| record.driver.clone());
        for module in &record.driver_modules {
            if !driver.modules.iter().any(|item| item == module) {
                driver.modules.push(module.clone());
            }
        }
        contributed |= driver != original;
        device.driver = Some(driver);
    }
    if contributed
        && !hwinfo.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == hwinfo.source)
    {
        device = device.with_source(SourceEvidence {
            source: hwinfo.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn apply_audio_lshw_enrichment(mut device: Device, lshw: &AudioLshwRecords) -> Device {
    let Some(address) = device.bus.as_ref().and_then(audio_pci_address) else {
        return device;
    };
    let Some(record) = lshw.by_pci_address.get(address) else {
        return device;
    };
    let mut contributed = false;

    if device.vendor.is_none() && record.vendor.is_some() {
        device.vendor = record.vendor.clone();
        contributed = true;
    }
    if device.model.is_none() && record.product.is_some() {
        device.model = record.product.clone();
        contributed = true;
    }
    if record.driver.is_some() {
        let mut driver = device.driver.take().unwrap_or(DriverInfo {
            name: None,
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
        let original = driver.clone();
        driver.name = driver.name.or_else(|| record.driver.clone());
        contributed |= driver != original;
        device.driver = Some(driver);
    }
    if contributed
        && !lshw.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == lshw.source)
    {
        device = device.with_source(SourceEvidence {
            source: lshw.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn audio_pci_address(bus: &BusInfo) -> Option<&str> {
    match bus {
        BusInfo::Pci { address, .. } => Some(address),
        _ => None,
    }
}

fn lshw_pci_address(value: &str) -> Option<String> {
    value.strip_prefix("pci@").map(ToString::to_string)
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
