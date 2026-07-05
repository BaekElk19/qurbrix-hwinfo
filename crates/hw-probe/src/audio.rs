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
            return ProbeResult::source_failure(self.name(), &result);
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
