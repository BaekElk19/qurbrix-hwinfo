use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, Device, DeviceKind, DeviceProperties, InputInfo, InputKind, SourceEvidence,
    SourceKind, SourceStatus,
};
use hw_parser::parse_proc_bus_input_devices;
use std::path::Path;

pub struct InputProbe;

#[async_trait]
impl Probe for InputProbe {
    fn name(&self) -> &'static str {
        "input"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Input]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .read_file(Path::new("/proc/bus/input/devices"))
            .await;
        if !result.is_success() {
            return ProbeResult::source_failure(self.name(), &result);
        }
        let devices = parse_proc_bus_input_devices(&result.stdout)
            .into_iter()
            .enumerate()
            .map(|(idx, input)| {
                let event = input
                    .handlers
                    .iter()
                    .find(|v| v.starts_with("event"))
                    .cloned()
                    .unwrap_or_else(|| idx.to_string());
                let name = input
                    .name
                    .clone()
                    .unwrap_or_else(|| "Input device".to_string());
                let lower = name.to_ascii_lowercase();
                let input_kind =
                    if input.handlers.iter().any(|h| h == "kbd") || lower.contains("keyboard") {
                        InputKind::Keyboard
                    } else if lower.contains("touchpad") {
                        InputKind::Touchpad
                    } else if lower.contains("touchscreen") {
                        InputKind::Touchscreen
                    } else if input.handlers.iter().any(|h| h.starts_with("mouse"))
                        || lower.contains("mouse")
                    {
                        InputKind::Mouse
                    } else {
                        InputKind::UnknownInput
                    };
                Device::new(
                    device_id::input_event(&event),
                    DeviceKind::Input,
                    name,
                    DeviceProperties::Input(InputInfo {
                        input_kind,
                        event_node: Some(format!("/dev/input/{event}")),
                        phys: input.phys,
                        uniq: input.uniq,
                        handlers: input.handlers,
                        bus_type: input.bus,
                        vendor_id: input.vendor_id,
                        product_id: input.product_id,
                        version: input.version,
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
