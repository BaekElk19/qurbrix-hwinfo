use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BatteryInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::parse_upower_dump;
use hw_source::CommandSpec;

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
            return ProbeResult::source_failure(self.name(), &result);
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
