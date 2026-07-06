use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, Device, DeviceKind, DeviceProperties, InputInfo, InputKind, ScanWarning,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::{parse_proc_bus_input_devices, InputCapabilities};
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
        let records = parse_proc_bus_input_devices(&result.stdout);
        if records.is_empty() {
            return ProbeResult {
                devices: probe_sysfs_inputs(ctx).await,
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "input source produced no device records",
                )
                .with_source(result.source)],
                consumed: Vec::new(),
            };
        }
        let devices = records
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
                let input_kind = classify_input(&name, &input.handlers, &input.capabilities);
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
        let capabilities = read_sysfs_input_capabilities(ctx, &path).await;
        devices.push(
            Device::new(
                device_id::input_event(&event),
                DeviceKind::Input,
                name.clone(),
                DeviceProperties::Input(InputInfo {
                    input_kind: classify_input(&name, &[], &capabilities),
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

async fn read_sysfs_input_capabilities(ctx: &ProbeContext<'_>, path: &Path) -> InputCapabilities {
    let capabilities_path = path.join("device/capabilities");
    InputCapabilities {
        ev: read_trimmed(ctx, &capabilities_path.join("ev")).await,
        key: read_trimmed(ctx, &capabilities_path.join("key")).await,
        rel: read_trimmed(ctx, &capabilities_path.join("rel")).await,
        abs: read_trimmed(ctx, &capabilities_path.join("abs")).await,
        properties: read_trimmed(ctx, &path.join("device/properties"))
            .await
            .or(read_trimmed(ctx, &capabilities_path.join("prop")).await),
    }
}

fn classify_input(name: &str, handlers: &[String], capabilities: &InputCapabilities) -> InputKind {
    let lower = name.to_ascii_lowercase();
    if handlers.iter().any(|h| h == "kbd") || lower.contains("keyboard") {
        InputKind::Keyboard
    } else if lower.contains("touchpad") {
        InputKind::Touchpad
    } else if lower.contains("touchscreen") {
        InputKind::Touchscreen
    } else if let Some(kind) = classify_input_capabilities(capabilities) {
        kind
    } else if handlers.iter().any(|h| h.starts_with("mouse")) || lower.contains("mouse") {
        InputKind::Mouse
    } else {
        InputKind::UnknownInput
    }
}

fn classify_input_capabilities(capabilities: &InputCapabilities) -> Option<InputKind> {
    const EV_REL: usize = 0x02;
    const EV_ABS: usize = 0x03;
    const REL_X: usize = 0x00;
    const REL_Y: usize = 0x01;
    const ABS_X: usize = 0x00;
    const ABS_Y: usize = 0x01;
    const INPUT_PROP_POINTER: usize = 0x00;
    const INPUT_PROP_DIRECT: usize = 0x01;
    const BTN_TOOL_PEN: usize = 0x140;
    const BTN_TOOL_RUBBER: usize = 0x141;
    const BTN_TOOL_BRUSH: usize = 0x142;
    const BTN_TOOL_PENCIL: usize = 0x143;
    const BTN_TOOL_AIRBRUSH: usize = 0x144;
    const BTN_TOOL_FINGER: usize = 0x145;
    const BTN_TOUCH: usize = 0x14a;
    const BTN_STYLUS: usize = 0x14b;
    const BTN_STYLUS2: usize = 0x14c;

    let has_abs_xy = capability_bit(capabilities.ev.as_deref(), EV_ABS)
        && capability_bit(capabilities.abs.as_deref(), ABS_X)
        && capability_bit(capabilities.abs.as_deref(), ABS_Y);
    let has_rel_xy = capability_bit(capabilities.ev.as_deref(), EV_REL)
        && capability_bit(capabilities.rel.as_deref(), REL_X)
        && capability_bit(capabilities.rel.as_deref(), REL_Y);
    let has_touch = capability_bit(capabilities.key.as_deref(), BTN_TOUCH);
    let has_finger_tool = capability_bit(capabilities.key.as_deref(), BTN_TOOL_FINGER);
    let has_pen_tool = [
        BTN_TOOL_PEN,
        BTN_TOOL_RUBBER,
        BTN_TOOL_BRUSH,
        BTN_TOOL_PENCIL,
        BTN_TOOL_AIRBRUSH,
        BTN_STYLUS,
        BTN_STYLUS2,
    ]
    .into_iter()
    .any(|bit| capability_bit(capabilities.key.as_deref(), bit));
    let direct = capability_bit(capabilities.properties.as_deref(), INPUT_PROP_DIRECT);
    let pointer = capability_bit(capabilities.properties.as_deref(), INPUT_PROP_POINTER);

    if has_abs_xy && has_pen_tool {
        Some(InputKind::Tablet)
    } else if has_abs_xy && has_touch && direct {
        Some(InputKind::Touchscreen)
    } else if has_abs_xy && has_touch && (pointer || has_finger_tool) {
        Some(InputKind::Touchpad)
    } else if has_rel_xy {
        Some(InputKind::Mouse)
    } else {
        None
    }
}

fn capability_bit(mask: Option<&str>, bit: usize) -> bool {
    let Some(mask) = mask else {
        return false;
    };
    let target_word = bit / 64;
    let target_bit = bit % 64;
    for (word_index, word) in mask.split_whitespace().rev().enumerate() {
        if word_index != target_word {
            continue;
        }
        let word = word.strip_prefix("0x").unwrap_or(word);
        return u64::from_str_radix(word, 16)
            .map(|value| value & (1u64 << target_bit) != 0)
            .unwrap_or(false);
    }
    false
}

async fn read_trimmed(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}
