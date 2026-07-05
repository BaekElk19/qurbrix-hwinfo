use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus, PciInfo,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::parse_lspci_nn_k;
use hw_source::CommandSpec;
use std::path::Path;

pub struct PciProbe;

#[async_trait]
impl Probe for PciProbe {
    fn name(&self) -> &'static str {
        "pci"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Pci]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout)
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_pci_devices(ctx).await;
            return fallback;
        }
        let devices = parse_lspci_nn_k(&result.stdout)
            .into_iter()
            .map(|record| {
                let name = format!(
                    "{} {}",
                    record.vendor.clone().unwrap_or_default(),
                    record.device.clone().unwrap_or_default()
                )
                .trim()
                .to_string();
                Device::new(
                    device_id::pci(&record.address),
                    DeviceKind::Pci,
                    if name.is_empty() {
                        record.address.clone()
                    } else {
                        name
                    },
                    DeviceProperties::Pci(PciInfo {
                        address: record.address.clone(),
                        class_name: record.class_name.clone(),
                        class_id: record.class_id.clone(),
                        vendor: record.vendor.clone(),
                        vendor_id: record.vendor_id.clone(),
                        device: record.device.clone(),
                        device_id: record.device_id.clone(),
                        subsystem_vendor_id: record.subsystem_vendor_id.clone(),
                        subsystem_device_id: record.subsystem_device_id.clone(),
                    }),
                )
                .with_bus(BusInfo::Pci {
                    address: record.address,
                    vendor_id: record.vendor_id,
                    device_id: record.device_id,
                    subsystem_vendor_id: record.subsystem_vendor_id,
                    subsystem_device_id: record.subsystem_device_id,
                    class: record.class_id,
                })
                .with_driver(DriverInfo {
                    name: record.kernel_driver,
                    version: None,
                    modules: record.kernel_modules,
                    provider: None,
                    status: DriverStatus::InUse,
                })
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

async fn probe_sysfs_pci_devices(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut paths = ctx.runner.glob("/sys/bus/pci/devices/*").await.paths;
    paths.sort();

    let mut devices = Vec::new();
    for path in paths {
        let Some(address) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_pci_address(address) {
            continue;
        }
        let vendor_id = read_pci_id(ctx, &path.join("vendor")).await;
        let device_id = read_pci_id(ctx, &path.join("device")).await;
        let class_id = read_pci_id(ctx, &path.join("class")).await;
        let subsystem_vendor_id = read_pci_id(ctx, &path.join("subsystem_vendor")).await;
        let subsystem_device_id = read_pci_id(ctx, &path.join("subsystem_device")).await;

        devices.push(
            Device::new(
                device_id::pci(address),
                DeviceKind::Pci,
                address.to_string(),
                DeviceProperties::Pci(PciInfo {
                    address: address.to_string(),
                    class_name: None,
                    class_id: class_id.clone(),
                    vendor: None,
                    vendor_id: vendor_id.clone(),
                    device: None,
                    device_id: device_id.clone(),
                    subsystem_vendor_id: subsystem_vendor_id.clone(),
                    subsystem_device_id: subsystem_device_id.clone(),
                }),
            )
            .with_bus(BusInfo::Pci {
                address: address.to_string(),
                vendor_id,
                device_id,
                subsystem_vendor_id,
                subsystem_device_id,
                class: class_id,
            })
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

fn is_pci_address(value: &str) -> bool {
    let mut parts = value.split([':', '.']);
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(domain), Some(bus), Some(device), Some(function), None)
            if is_hex_len(domain, 4)
                && is_hex_len(bus, 2)
                && is_hex_len(device, 2)
                && is_hex_len(function, 1)
    )
}

fn is_hex_len(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

async fn read_pci_id(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim().trim_start_matches("0x");
    (!value.is_empty()).then(|| value.to_ascii_lowercase())
}
