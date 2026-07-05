use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus, UsbInfo,
};
use hw_parser::parse_lsusb;
use hw_source::CommandSpec;

pub struct UsbProbe;

#[async_trait]
impl Probe for UsbProbe {
    fn name(&self) -> &'static str {
        "usb"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Usb]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(
                &CommandSpec::new("lsusb", std::iter::empty::<&str>()),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            return ProbeResult::source_failure(self.name(), &result);
        }
        let devices = parse_lsusb(&result.stdout)
            .into_iter()
            .map(|record| {
                let id = device_id::usb(
                    record.bus.as_deref(),
                    record.device.as_deref(),
                    record.vendor_id.as_deref(),
                    record.product_id.as_deref(),
                    record.serial.as_deref(),
                );
                Device::new(
                    id,
                    DeviceKind::Usb,
                    record
                        .product
                        .clone()
                        .unwrap_or_else(|| "USB device".to_string()),
                    DeviceProperties::Usb(UsbInfo {
                        bus_number: record.bus.clone(),
                        device_number: record.device.clone(),
                        vendor_id: record.vendor_id.clone(),
                        product_id: record.product_id.clone(),
                        class: record.class.clone(),
                        subclass: record.subclass.clone(),
                        protocol: record.protocol.clone(),
                        manufacturer: record.manufacturer.clone(),
                        product: record.product.clone(),
                        serial: record.serial.clone(),
                        speed: record.speed.clone(),
                    }),
                )
                .with_bus(BusInfo::Usb {
                    bus: record.bus,
                    device: record.device,
                    vendor_id: record.vendor_id,
                    product_id: record.product_id,
                    interface: None,
                    class: record.class,
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
