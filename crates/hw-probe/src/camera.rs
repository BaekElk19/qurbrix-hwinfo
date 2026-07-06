use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BusInfo, CameraInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::{
    parse_lshw_video, parse_v4l2_list_devices, parse_v4l2_list_formats_ext, LshwVideoRecord,
};
use hw_source::CommandSpec;
use std::{collections::HashMap, path::Path};

pub struct CameraProbe;

#[async_trait]
impl Probe for CameraProbe {
    fn name(&self) -> &'static str {
        "camera"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Camera]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(
                &CommandSpec::new("v4l2-ctl", ["--list-devices"]),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_cameras(ctx).await;
            return fallback;
        }
        let mut devices = Vec::new();
        let lshw = camera_lshw_records(ctx).await;
        for cam in parse_v4l2_list_devices(&result.stdout) {
            let Some(node) = cam.nodes.into_iter().next() else {
                continue;
            };
            let device = Device::new(
                device_id::camera(&node),
                DeviceKind::Camera,
                cam.name,
                DeviceProperties::Camera(CameraInfo {
                    video_node: Some(node.clone()),
                    capabilities: Vec::new(),
                }),
            )
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
            let device = apply_camera_sysfs_enrichment(ctx, device, &node).await;
            let device = apply_camera_lshw_enrichment(device, &lshw, &node);
            devices.push(apply_camera_format_enrichment(ctx, device, &node).await);
        }
        ProbeResult::with_devices(devices)
    }
}

#[derive(Default)]
struct CameraLshwRecords {
    source: String,
    by_video_node: HashMap<String, LshwVideoRecord>,
}

async fn probe_sysfs_cameras(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    let lshw = camera_lshw_records(ctx).await;
    let mut paths = ctx.runner.glob("/sys/class/video4linux/video*").await.paths;
    paths.sort();

    for path in paths {
        let Some(node_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let video_node = format!("/dev/{node_name}");
        let name = read_sysfs_value(ctx, &path, "name")
            .await
            .unwrap_or_else(|| video_node.clone());

        let device = Device::new(
            device_id::camera(&video_node),
            DeviceKind::Camera,
            name,
            DeviceProperties::Camera(CameraInfo {
                video_node: Some(video_node.clone()),
                capabilities: Vec::new(),
            }),
        )
        .with_source(SourceEvidence {
            source: path.display().to_string(),
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
        let device = apply_camera_sysfs_enrichment(ctx, device, &video_node).await;
        let device = apply_camera_lshw_enrichment(device, &lshw, &video_node);
        devices.push(apply_camera_format_enrichment(ctx, device, &video_node).await);
    }

    devices
}

async fn camera_lshw_records(ctx: &ProbeContext<'_>) -> CameraLshwRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("lshw", ["-class", "multimedia"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return CameraLshwRecords::default();
    }

    let by_video_node = parse_lshw_video(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.logical_name.clone()?, record)))
        .collect();
    CameraLshwRecords {
        source: result.source,
        by_video_node,
    }
}

async fn apply_camera_format_enrichment(
    ctx: &ProbeContext<'_>,
    mut device: Device,
    video_node: &str,
) -> Device {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("v4l2-ctl", ["--device", video_node, "--list-formats-ext"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return device;
    }

    let capabilities = parse_v4l2_list_formats_ext(&result.stdout);
    if capabilities.is_empty() {
        return device;
    }

    let mut contributed = false;
    if let DeviceProperties::Camera(info) = &mut device.properties {
        for capability in capabilities {
            if !info.capabilities.contains(&capability) {
                info.capabilities.push(capability);
                contributed = true;
            }
        }
    }
    if contributed
        && !device
            .sources
            .iter()
            .any(|source| source.source == result.source)
    {
        device = device.with_source(SourceEvidence {
            source: result.source,
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn apply_camera_lshw_enrichment(
    mut device: Device,
    lshw: &CameraLshwRecords,
    video_node: &str,
) -> Device {
    let Some(record) = lshw.by_video_node.get(video_node) else {
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
    if record.driver.is_some() {
        let mut driver = device.driver.take().unwrap_or(DriverInfo {
            name: None,
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
        let original = driver.clone();
        driver.name = driver.name.or_else(|| record.driver.clone());
        contributed |= driver != original;
        device.driver = Some(driver);
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

async fn apply_camera_sysfs_enrichment(
    ctx: &ProbeContext<'_>,
    mut device: Device,
    video_node: &str,
) -> Device {
    let Some(node_name) = Path::new(video_node)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return device;
    };
    let sysfs_path = Path::new("/sys/class/video4linux").join(node_name);
    let driver = read_sysfs_value(ctx, &sysfs_path.join("device"), "uevent")
        .await
        .and_then(|uevent| parse_uevent_value(&uevent, "DRIVER"));
    let usb = read_camera_usb_identity(ctx, &sysfs_path).await;
    let sysfs_contributed = driver.is_some() || usb.has_data();

    if let Some(driver) = driver {
        device = device.with_driver(DriverInfo {
            name: Some(driver),
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
    }
    if usb.has_data() {
        device.vendor = device.vendor.take().or(usb.manufacturer.clone());
        device.model = device.model.take().or(usb.product.clone());
        device.serial = device.serial.take().or(usb.serial.clone());
        device.bus = Some(BusInfo::Usb {
            bus: usb.bus,
            device: usb.device,
            vendor_id: usb.vendor_id,
            product_id: usb.product_id,
            speed: usb.speed,
            interface: usb.interface,
            class: usb.class,
        });
    }
    let source = sysfs_path.display().to_string();
    if sysfs_contributed
        && !device
            .sources
            .iter()
            .any(|evidence| evidence.source == source)
    {
        device = device.with_source(SourceEvidence {
            source,
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

#[derive(Default)]
struct CameraUsbIdentity {
    vendor_id: Option<String>,
    product_id: Option<String>,
    manufacturer: Option<String>,
    product: Option<String>,
    serial: Option<String>,
    bus: Option<String>,
    device: Option<String>,
    speed: Option<String>,
    interface: Option<String>,
    class: Option<String>,
}

impl CameraUsbIdentity {
    fn has_data(&self) -> bool {
        self.vendor_id.is_some()
            || self.product_id.is_some()
            || self.manufacturer.is_some()
            || self.product.is_some()
            || self.serial.is_some()
            || self.bus.is_some()
            || self.device.is_some()
            || self.speed.is_some()
            || self.interface.is_some()
            || self.class.is_some()
    }
}

async fn read_camera_usb_identity(ctx: &ProbeContext<'_>, sysfs_path: &Path) -> CameraUsbIdentity {
    let usb_path = sysfs_path.join("device/..");
    CameraUsbIdentity {
        vendor_id: read_sysfs_value(ctx, &usb_path, "idVendor")
            .await
            .map(|value| value.to_ascii_lowercase()),
        product_id: read_sysfs_value(ctx, &usb_path, "idProduct")
            .await
            .map(|value| value.to_ascii_lowercase()),
        manufacturer: read_sysfs_value(ctx, &usb_path, "manufacturer").await,
        product: read_sysfs_value(ctx, &usb_path, "product").await,
        serial: read_sysfs_value(ctx, &usb_path, "serial").await,
        bus: read_sysfs_value(ctx, &usb_path, "busnum").await,
        device: read_sysfs_value(ctx, &usb_path, "devnum").await,
        speed: read_sysfs_value(ctx, &usb_path, "speed").await,
        interface: read_sysfs_value(ctx, &sysfs_path.join("device"), "bInterfaceNumber").await,
        class: read_sysfs_value(ctx, &sysfs_path.join("device"), "bInterfaceClass")
            .await
            .map(|value| value.to_ascii_lowercase()),
    }
}

fn parse_uevent_value(input: &str, key: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate == key && !value.trim().is_empty()).then(|| value.trim().to_string())
    })
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
