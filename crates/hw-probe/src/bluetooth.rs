use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BluetoothInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus,
    ScanWarning, SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::{
    parse_bluetoothctl_paired_devices, parse_hciconfig, parse_lshw_communication,
    LshwCommunicationRecord,
};
use hw_source::CommandSpec;
use std::{collections::HashMap, path::Path};

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
        let lshw = bluetooth_lshw_records(ctx).await;
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
        let controllers = parse_hciconfig(&hci.stdout);
        if controllers.is_empty() {
            warnings.push(
                ScanWarning::new(
                    "source_empty",
                    "bluetooth source produced no controller records",
                )
                .with_source(hci.source.clone()),
            );
        }
        let devices = controllers
            .into_iter()
            .enumerate()
            .map(|(idx, ctrl)| {
                let id_value = ctrl.address.clone().unwrap_or_else(|| idx.to_string());
                let logical_name = format!("hci{idx}");
                let device = Device::new(
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
                        hci_version: ctrl.hci_version,
                        lmp_version: ctrl.lmp_version,
                        manufacturer: ctrl.manufacturer,
                        device_class: ctrl.device_class,
                        features: ctrl.features,
                    }),
                )
                .with_source(SourceEvidence {
                    source: hci.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                });
                apply_bluetooth_lshw_enrichment(device, &lshw, &logical_name)
            })
            .collect();
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

#[derive(Default)]
struct BluetoothLshwRecords {
    source: String,
    by_logical_name: HashMap<String, LshwCommunicationRecord>,
}

async fn bluetooth_lshw_records(ctx: &ProbeContext<'_>) -> BluetoothLshwRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("lshw", ["-class", "communication"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return BluetoothLshwRecords::default();
    }

    let by_logical_name = parse_lshw_communication(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.logical_name.clone()?, record)))
        .collect();
    BluetoothLshwRecords {
        source: result.source,
        by_logical_name,
    }
}

fn apply_bluetooth_lshw_enrichment(
    mut device: Device,
    lshw: &BluetoothLshwRecords,
    logical_name: &str,
) -> Device {
    let Some(record) = lshw.by_logical_name.get(logical_name) else {
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
    if device.driver.is_none() && record.driver.is_some() {
        device = device.with_driver(DriverInfo {
            name: record.driver.clone(),
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
        contributed = true;
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
        let address = read_sysfs_value(ctx, &path, "address").await;
        let controller_name = rfkill_name.unwrap_or(hci_name.clone());
        let id_value = address.as_deref().unwrap_or(&hci_name);

        devices.push(
            Device::new(
                device_id::other("bluetooth", id_value),
                DeviceKind::Bluetooth,
                controller_name.clone(),
                DeviceProperties::Bluetooth(BluetoothInfo {
                    address,
                    controller_name: Some(controller_name),
                    powered,
                    discoverable: None,
                    paired_device_count: None,
                    paired_devices: Vec::new(),
                    hci_version: None,
                    lmp_version: None,
                    manufacturer: None,
                    device_class: None,
                    features: Vec::new(),
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
