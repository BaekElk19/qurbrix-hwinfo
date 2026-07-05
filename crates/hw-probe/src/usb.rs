use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus, UsbInfo,
};
use hw_parser::parse_lsusb;
use hw_source::CommandSpec;
use std::path::Path;

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
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_usb(ctx).await;
            return fallback;
        }
        let devices = parse_lsusb(&result.stdout)
            .into_iter()
            .filter(|record| !is_usb_hub(record.product.as_deref()))
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

async fn probe_sysfs_usb(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    for path in ctx.runner.glob("/sys/bus/usb/devices/*").await.paths {
        let product = read_sysfs_value(ctx, &path, "product").await;
        if is_sysfs_usb_hub_or_controller(product.as_deref()) {
            continue;
        }

        let bus = read_sysfs_value(ctx, &path, "busnum").await;
        let device = read_sysfs_value(ctx, &path, "devnum").await;
        if bus.is_none() && device.is_none() {
            continue;
        }

        let vendor_id = read_sysfs_value(ctx, &path, "idVendor").await;
        let product_id = read_sysfs_value(ctx, &path, "idProduct").await;
        let class = read_sysfs_value(ctx, &path, "bDeviceClass").await;
        let subclass = read_sysfs_value(ctx, &path, "bDeviceSubClass").await;
        let protocol = read_sysfs_value(ctx, &path, "bDeviceProtocol").await;
        let manufacturer = read_sysfs_value(ctx, &path, "manufacturer").await;
        let serial = read_sysfs_value(ctx, &path, "serial").await;
        let speed = read_sysfs_value(ctx, &path, "speed").await;
        let id = device_id::usb(
            bus.as_deref(),
            device.as_deref(),
            vendor_id.as_deref(),
            product_id.as_deref(),
            serial.as_deref(),
        );

        devices.push(
            Device::new(
                id,
                DeviceKind::Usb,
                product.clone().unwrap_or_else(|| "USB device".to_string()),
                DeviceProperties::Usb(UsbInfo {
                    bus_number: bus.clone(),
                    device_number: device.clone(),
                    vendor_id: vendor_id.clone(),
                    product_id: product_id.clone(),
                    class: class.clone(),
                    subclass,
                    protocol,
                    manufacturer,
                    product,
                    serial,
                    speed,
                }),
            )
            .with_bus(BusInfo::Usb {
                bus,
                device,
                vendor_id,
                product_id,
                interface: None,
                class,
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

fn is_usb_hub(product: Option<&str>) -> bool {
    let product = product.unwrap_or_default().to_ascii_lowercase();
    product.contains("root hub")
        || product.ends_with(" hub")
        || product.contains(" hub ")
        || product.contains(" hub,")
}

fn is_sysfs_usb_hub_or_controller(product: Option<&str>) -> bool {
    let product = product.unwrap_or_default().to_ascii_lowercase();
    product == "hub" || product.contains("host controller") || is_usb_hub(Some(&product))
}
