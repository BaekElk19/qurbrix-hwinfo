use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CdromInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::parse_proc_cdrom_info;
use std::path::Path;

pub struct CdromProbe;

#[async_trait]
impl Probe for CdromProbe {
    fn name(&self) -> &'static str {
        "cdrom"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Cdrom]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .read_file(Path::new("/proc/sys/dev/cdrom/info"))
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_cdroms(ctx).await;
            return fallback;
        }
        let info = parse_proc_cdrom_info(&result.stdout);
        let devices = info
            .drive_names
            .into_iter()
            .map(|drive| {
                Device::new(
                    device_id::other("cdrom", &drive),
                    DeviceKind::Cdrom,
                    drive.clone(),
                    DeviceProperties::Cdrom(CdromInfo {
                        device_node: Some(format!("/dev/{drive}")),
                        media_present: None,
                        capabilities: info.capabilities.clone(),
                    }),
                )
                .with_source(SourceEvidence {
                    source: result.source.clone(),
                    kind: SourceKind::Procfs,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult::with_devices(devices)
    }
}

async fn probe_sysfs_cdroms(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    let mut paths = ctx.runner.glob("/sys/class/block/sr*").await.paths;
    paths.sort();

    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_sr_node(name) {
            continue;
        }
        let device_node = format!("/dev/{name}");
        devices.push(
            Device::new(
                device_id::other("cdrom", name),
                DeviceKind::Cdrom,
                name.to_string(),
                DeviceProperties::Cdrom(CdromInfo {
                    device_node: Some(device_node),
                    media_present: None,
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

fn is_sr_node(name: &str) -> bool {
    name.strip_prefix("sr")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}
