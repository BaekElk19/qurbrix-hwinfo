use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus, PciInfo,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::parse_lspci_nn_k;
use hw_source::CommandSpec;

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
            return ProbeResult::source_failure(self.name(), &result);
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
