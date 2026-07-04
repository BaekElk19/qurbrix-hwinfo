use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CameraInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::parse_v4l2_list_devices;
use hw_source::CommandSpec;

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
            return ProbeResult::default();
        }
        let devices = parse_v4l2_list_devices(&result.stdout)
            .into_iter()
            .flat_map(|cam| {
                let source = result.source.clone();
                cam.nodes
                    .into_iter()
                    .map(move |node| {
                        Device::new(
                            device_id::camera(&node),
                            DeviceKind::Camera,
                            cam.name.clone(),
                            DeviceProperties::Camera(CameraInfo {
                                video_node: Some(node),
                                capabilities: Vec::new(),
                            }),
                        )
                        .with_source(SourceEvidence {
                            source: source.clone(),
                            kind: SourceKind::Command,
                            status: SourceStatus::Success,
                            summary: None,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        ProbeResult::with_devices(devices)
    }
}
