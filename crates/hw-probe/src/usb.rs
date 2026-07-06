use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus, UsbInfo,
};
use hw_parser::parse_lsusb;
use hw_source::CommandSpec;
use std::{collections::HashMap, path::Path};

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
        let sysfs_records = read_sysfs_usb_records(ctx).await;
        let sysfs_by_bus_dev = sysfs_records
            .into_iter()
            .filter_map(|record| Some(((record.bus.clone()?, record.device.clone()?), record)))
            .collect::<HashMap<_, _>>();
        let devices =
            parse_lsusb(&result.stdout)
                .into_iter()
                .filter(|record| !is_usb_hub(record.product.as_deref()))
                .map(|mut record| {
                    let sysfs = record.bus.as_ref().zip(record.device.as_ref()).and_then(
                        |(bus, device)| sysfs_by_bus_dev.get(&(bus.clone(), device.clone())),
                    );
                    if let Some(sysfs) = sysfs {
                        record.class = record.class.or(sysfs.class.clone());
                        record.subclass = record.subclass.or(sysfs.subclass.clone());
                        record.protocol = record.protocol.or(sysfs.protocol.clone());
                        record.manufacturer = record.manufacturer.or(sysfs.manufacturer.clone());
                        record.product = record.product.or(sysfs.product.clone());
                        record.serial = record.serial.or(sysfs.serial.clone());
                        record.speed = record.speed.or(sysfs.speed.clone());
                    }
                    let max_power_ma = sysfs.and_then(|record| record.max_power_ma);
                    let id = device_id::usb(
                        record.bus.as_deref(),
                        record.device.as_deref(),
                        record.vendor_id.as_deref(),
                        record.product_id.as_deref(),
                        record.serial.as_deref(),
                    );
                    let mut device = Device::new(
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
                            max_power_ma,
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
                    });
                    if let Some(sysfs) = sysfs {
                        device = device.with_source(SourceEvidence {
                            source: sysfs.path.display().to_string(),
                            kind: SourceKind::Sysfs,
                            status: SourceStatus::Success,
                            summary: None,
                        });
                    }
                    device
                })
                .collect();
        ProbeResult::with_devices(devices)
    }
}

struct SysfsUsbRecord {
    path: std::path::PathBuf,
    bus: Option<String>,
    device: Option<String>,
    vendor_id: Option<String>,
    product_id: Option<String>,
    class: Option<String>,
    subclass: Option<String>,
    protocol: Option<String>,
    manufacturer: Option<String>,
    product: Option<String>,
    serial: Option<String>,
    speed: Option<String>,
    max_power_ma: Option<u32>,
}

async fn probe_sysfs_usb(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    for record in read_sysfs_usb_records(ctx).await {
        let id = device_id::usb(
            record.bus.as_deref(),
            record.device.as_deref(),
            record.vendor_id.as_deref(),
            record.product_id.as_deref(),
            record.serial.as_deref(),
        );

        devices.push(
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
                    subclass: record.subclass,
                    protocol: record.protocol,
                    manufacturer: record.manufacturer,
                    product: record.product,
                    serial: record.serial,
                    speed: record.speed,
                    max_power_ma: record.max_power_ma,
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
                source: record.path.display().to_string(),
                kind: SourceKind::Sysfs,
                status: SourceStatus::Success,
                summary: None,
            }),
        );
    }
    devices
}

async fn read_sysfs_usb_records(ctx: &ProbeContext<'_>) -> Vec<SysfsUsbRecord> {
    let mut records = Vec::new();
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
        let max_power_ma = read_sysfs_value(ctx, &path, "bMaxPower")
            .await
            .and_then(parse_usb_max_power_ma);

        records.push(SysfsUsbRecord {
            path,
            bus,
            device,
            vendor_id,
            product_id,
            class,
            subclass,
            protocol,
            manufacturer,
            product,
            serial,
            speed,
            max_power_ma,
        });
    }
    records
}

fn parse_usb_max_power_ma(value: String) -> Option<u32> {
    value.trim().trim_end_matches("mA").trim().parse().ok()
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
