use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, AudioInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
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
        let devices = parse_proc_asound_cards(&result.stdout)
            .into_iter()
            .map(|card| {
                Device::new(
                    device_id::other("audio:card", &card.index.to_string()),
                    DeviceKind::Audio,
                    card.name
                        .clone()
                        .unwrap_or_else(|| format!("Audio card {}", card.index)),
                    DeviceProperties::Audio(AudioInfo {
                        card_index: Some(card.index),
                        card_name: card.name,
                        ..Default::default()
                    }),
                )
                .with_source(SourceEvidence {
                    source: result.source.clone(),
                    kind: SourceKind::Procfs,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult::with_devices(devices)
    }
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
        devices.push(
            Device::new(
                device_id::other("audio:card", &index.to_string()),
                DeviceKind::Audio,
                card_name.clone(),
                DeviceProperties::Audio(AudioInfo {
                    card_index: Some(index),
                    card_name: Some(card_name),
                    ..Default::default()
                }),
            )
            .with_source(SourceEvidence {
                source: path.display().to_string(),
                kind: SourceKind::Sysfs,
                status: SourceStatus::Success,
                summary: None,
            }),
        );
    }

    devices
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
