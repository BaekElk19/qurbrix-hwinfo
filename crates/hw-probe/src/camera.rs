use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CameraInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
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
        let devices = parse_v4l2_list_devices(&result.stdout)
            .into_iter()
            .filter_map(|cam| {
                let node = cam.nodes.into_iter().next()?;
                Some(
                    Device::new(
                        device_id::camera(&node),
                        DeviceKind::Camera,
                        cam.name,
                        DeviceProperties::Camera(CameraInfo {
                            video_node: Some(node),
                            capabilities: Vec::new(),
                        }),
                    )
                    .with_source(SourceEvidence {
                        source: result.source.clone(),
                        kind: SourceKind::Command,
                        status: SourceStatus::Success,
                        summary: None,
                    }),
                )
            })
            .collect();
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

        devices.push(
            Device::new(
                device_id::camera(&video_node),
                DeviceKind::Camera,
                name,
                DeviceProperties::Camera(CameraInfo {
                    video_node: Some(video_node),
                    capabilities: Vec::new(),
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
