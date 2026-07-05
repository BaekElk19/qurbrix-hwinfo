use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BatteryInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::parse_upower_dump;
use hw_source::CommandSpec;
use std::path::Path;

pub struct BatteryProbe;

#[async_trait]
impl Probe for BatteryProbe {
    fn name(&self) -> &'static str {
        "battery"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Battery]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("upower", ["--dump"]), ctx.timeout)
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_batteries(ctx).await;
            return fallback;
        }
        let devices = parse_upower_dump(&result.stdout)
            .into_iter()
            .filter(|power| {
                !is_line_power_device(power.device_path.as_deref(), power.native_path.as_deref())
            })
            .map(|power| {
                let name = power
                    .native_path
                    .clone()
                    .unwrap_or_else(|| "battery".to_string());
                Device::new(
                    device_id::battery(&name),
                    DeviceKind::Battery,
                    name.clone(),
                    DeviceProperties::Battery(BatteryInfo {
                        power_type: Some("battery".to_string()),
                        vendor: power.vendor,
                        model: power.model,
                        serial: power.serial,
                        technology: power.technology,
                        state: power.state,
                        capacity_percent: power.capacity_percent,
                        energy_full_wh: power.energy_full_wh,
                        energy_design_wh: power.energy_design_wh,
                        energy_now_wh: power.energy_now_wh,
                        voltage_v: power.voltage_v,
                        cycle_count: None,
                        present: power.present,
                    }),
                )
                .with_source(SourceEvidence {
                    source: result.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult::with_devices(devices)
    }
}

fn is_line_power_device(device_path: Option<&str>, native_path: Option<&str>) -> bool {
    [device_path, native_path]
        .into_iter()
        .flatten()
        .map(str::to_ascii_lowercase)
        .any(|value| value.contains("line_power"))
}

async fn probe_sysfs_batteries(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    for path in ctx.runner.glob("/sys/class/power_supply/BAT*").await.paths {
        let Some(power_type) = read_trimmed(ctx, &path.join("type")).await else {
            continue;
        };
        if power_type != "Battery" {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let source = path.display().to_string();
        devices.push(
            Device::new(
                device_id::battery(name),
                DeviceKind::Battery,
                name.to_string(),
                DeviceProperties::Battery(BatteryInfo {
                    power_type: Some("battery".to_string()),
                    vendor: read_trimmed(ctx, &path.join("manufacturer")).await,
                    model: read_trimmed(ctx, &path.join("model_name")).await,
                    serial: read_trimmed(ctx, &path.join("serial_number")).await,
                    technology: read_trimmed(ctx, &path.join("technology")).await,
                    state: read_trimmed(ctx, &path.join("status")).await,
                    capacity_percent: read_trimmed(ctx, &path.join("capacity"))
                        .await
                        .and_then(|value| value.parse().ok()),
                    energy_full_wh: read_micro_units(ctx, &path.join("energy_full")).await,
                    energy_design_wh: read_micro_units(ctx, &path.join("energy_full_design")).await,
                    energy_now_wh: read_micro_units(ctx, &path.join("energy_now")).await,
                    voltage_v: read_micro_units(ctx, &path.join("voltage_now")).await,
                    cycle_count: read_trimmed(ctx, &path.join("cycle_count"))
                        .await
                        .and_then(|value| value.parse().ok()),
                    present: read_trimmed(ctx, &path.join("present"))
                        .await
                        .and_then(|value| match value.as_str() {
                            "1" => Some(true),
                            "0" => Some(false),
                            _ => None,
                        }),
                }),
            )
            .with_source(SourceEvidence {
                source,
                kind: SourceKind::Sysfs,
                status: SourceStatus::Success,
                summary: None,
            }),
        );
    }
    devices
}

async fn read_trimmed(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}

async fn read_micro_units(ctx: &ProbeContext<'_>, path: &Path) -> Option<f32> {
    read_trimmed(ctx, path)
        .await?
        .parse::<f32>()
        .ok()
        .map(|value| value / 1_000_000.0)
}
