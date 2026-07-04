use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BluetoothInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::{parse_bluetoothctl_paired_devices, parse_hciconfig};
use hw_source::CommandSpec;

pub struct BluetoothProbe;

#[async_trait]
impl Probe for BluetoothProbe {
    fn name(&self) -> &'static str {
        "bluetooth"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Bluetooth]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let hci = ctx
            .runner
            .run_command(&CommandSpec::new("hciconfig", ["-a"]), ctx.timeout)
            .await;
        if !hci.is_success() {
            return ProbeResult::default();
        }
        let paired = ctx
            .runner
            .run_command(
                &CommandSpec::new("bluetoothctl", ["paired-devices"]),
                ctx.timeout,
            )
            .await;
        let paired_names: Vec<String> = if paired.is_success() {
            parse_bluetoothctl_paired_devices(&paired.stdout)
                .into_iter()
                .map(|p| p.name)
                .collect()
        } else {
            Vec::new()
        };
        let devices = parse_hciconfig(&hci.stdout)
            .into_iter()
            .enumerate()
            .map(|(idx, ctrl)| {
                let id_value = ctrl.address.clone().unwrap_or_else(|| idx.to_string());
                Device::new(
                    device_id::other("bluetooth", &id_value),
                    DeviceKind::Bluetooth,
                    ctrl.name
                        .clone()
                        .unwrap_or_else(|| "Bluetooth controller".to_string()),
                    DeviceProperties::Bluetooth(BluetoothInfo {
                        address: ctrl.address,
                        controller_name: ctrl.name,
                        powered: Some(ctrl.flags.iter().any(|f| f == "UP")),
                        discoverable: Some(ctrl.flags.iter().any(|f| f == "ISCAN")),
                        paired_device_count: Some(paired_names.len() as u32),
                        paired_devices: paired_names.clone(),
                    }),
                )
                .with_source(SourceEvidence {
                    source: hci.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult::with_devices(devices)
    }
}
