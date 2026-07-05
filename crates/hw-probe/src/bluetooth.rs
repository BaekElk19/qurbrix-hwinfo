use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BluetoothInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::{parse_bluetoothctl_paired_devices, parse_hciconfig};
use hw_source::CommandSpec;
use std::path::Path;

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
            let mut fallback = ProbeResult::source_failure(self.name(), &hci);
            fallback.devices = probe_sysfs_bluetooth(ctx).await;
            return fallback;
        }
        let paired = ctx
            .runner
            .run_command(
                &CommandSpec::new("bluetoothctl", ["paired-devices"]),
                ctx.timeout,
            )
            .await;
        let mut warnings = Vec::new();
        let paired_names: Vec<String> = if paired.is_success() {
            parse_bluetoothctl_paired_devices(&paired.stdout)
                .into_iter()
                .map(|p| p.name)
                .collect()
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &paired).warnings);
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
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

async fn probe_sysfs_bluetooth(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    for path in ctx.runner.glob("/sys/class/bluetooth/hci*").await.paths {
        let hci_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Bluetooth controller")
            .to_string();
        let rfkill = ctx
            .runner
            .glob(&format!("{}/rfkill*", path.display()))
            .await
            .paths
            .into_iter()
            .next();
        let rfkill_name = match rfkill.as_deref() {
            Some(path) => read_sysfs_value(ctx, path, "name").await,
            None => None,
        };
        let powered = match rfkill.as_deref() {
            Some(path) => read_sysfs_value(ctx, path, "state")
                .await
                .and_then(|value| parse_rfkill_unblocked(&value)),
            None => None,
        };
        let controller_name = rfkill_name.unwrap_or(hci_name.clone());

        devices.push(
            Device::new(
                device_id::other("bluetooth", &hci_name),
                DeviceKind::Bluetooth,
                controller_name.clone(),
                DeviceProperties::Bluetooth(BluetoothInfo {
                    address: None,
                    controller_name: Some(controller_name),
                    powered,
                    discoverable: None,
                    paired_device_count: None,
                    paired_devices: Vec::new(),
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

async fn read_sysfs_value(ctx: &ProbeContext<'_>, path: &Path, name: &str) -> Option<String> {
    let result = ctx.runner.read_file(&path.join(name)).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_rfkill_unblocked(value: &str) -> Option<bool> {
    match value.trim() {
        "0" | "2" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}
