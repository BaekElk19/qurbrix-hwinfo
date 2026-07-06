use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CameraInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus,
    SourceEvidence, SourceKind, SourceStatus,
};
use hw_parser::parse_v4l2_list_devices;
use hw_source::CommandSpec;
use std::path::Path;

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
            devices.push(apply_camera_driver(ctx, device, &node).await);
        }
        ProbeResult::with_devices(devices)
    }
}

async fn probe_sysfs_cameras(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
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
        devices.push(apply_camera_driver(ctx, device, &video_node).await);
    }

    devices
}

async fn apply_camera_driver(ctx: &ProbeContext<'_>, device: Device, video_node: &str) -> Device {
    let Some(node_name) = Path::new(video_node)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return device;
    };
    let sysfs_path = Path::new("/sys/class/video4linux").join(node_name);
    let Some(driver) = read_sysfs_value(ctx, &sysfs_path.join("device"), "uevent")
        .await
        .and_then(|uevent| parse_uevent_value(&uevent, "DRIVER"))
    else {
        return device;
    };

    let mut device = device.with_driver(DriverInfo {
        name: Some(driver),
        version: None,
        modules: Vec::new(),
        provider: None,
        status: DriverStatus::InUse,
    });
    let source = sysfs_path.display().to_string();
    if !device
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
