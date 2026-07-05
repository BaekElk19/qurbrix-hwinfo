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
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_inputs(ctx).await;
            return fallback;
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
                let input_kind = classify_input(&name, &input.handlers);
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

async fn probe_sysfs_inputs(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut events = ctx
        .runner
        .glob("/sys/class/input/event*")
        .await
        .paths
        .into_iter()
        .filter_map(|path| {
            let index = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(event_index)?;
            Some((index, path))
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|(index, _)| *index);

    let mut devices = Vec::new();
    for (index, path) in events {
        let event = format!("event{index}");
        let event_node = format!("/dev/input/{event}");
        let name = read_trimmed(ctx, &path.join("device/name"))
            .await
            .unwrap_or_else(|| event_node.clone());
        devices.push(
            Device::new(
                device_id::input_event(&event),
                DeviceKind::Input,
                name.clone(),
                DeviceProperties::Input(InputInfo {
                    input_kind: classify_input(&name, &[]),
                    event_node: Some(event_node),
                    phys: read_trimmed(ctx, &path.join("device/phys")).await,
                    uniq: read_trimmed(ctx, &path.join("device/uniq")).await,
                    handlers: Vec::new(),
                    bus_type: read_trimmed(ctx, &path.join("device/id/bustype")).await,
                    vendor_id: read_trimmed(ctx, &path.join("device/id/vendor"))
                        .await
                        .map(|value| value.to_ascii_lowercase()),
                    product_id: read_trimmed(ctx, &path.join("device/id/product"))
                        .await
                        .map(|value| value.to_ascii_lowercase()),
                    version: read_trimmed(ctx, &path.join("device/id/version")).await,
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

fn event_index(name: &str) -> Option<u32> {
    let suffix = name.strip_prefix("event")?;
    if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

fn classify_input(name: &str, handlers: &[String]) -> InputKind {
    let lower = name.to_ascii_lowercase();
    if handlers.iter().any(|h| h == "kbd") || lower.contains("keyboard") {
        InputKind::Keyboard
    } else if lower.contains("touchpad") {
        InputKind::Touchpad
    } else if lower.contains("touchscreen") {
        InputKind::Touchscreen
    } else if handlers.iter().any(|h| h.starts_with("mouse")) || lower.contains("mouse") {
        InputKind::Mouse
    } else {
        InputKind::UnknownInput
    }
}

async fn read_trimmed(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}
