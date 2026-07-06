use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CdromInfo, Device, DeviceKind, DeviceProperties, ScanWarning, SourceEvidence,
    SourceKind, SourceStatus,
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
        if info.drive_names.is_empty() {
            return ProbeResult {
                devices: probe_sysfs_cdroms(ctx).await,
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "cdrom source produced no drive records",
                )
                .with_source(result.source)],
                consumed: Vec::new(),
            };
        }
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
        let mut device = Device::new(
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
        });
        device.vendor = read_trimmed(ctx, &path.join("device/vendor")).await;
        device.model = read_trimmed(ctx, &path.join("device/model")).await;
        device.serial = read_trimmed(ctx, &path.join("device/serial")).await;
        devices.push(device);
    }

    devices
}

fn is_sr_node(name: &str) -> bool {
    name.strip_prefix("sr")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

async fn read_trimmed(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}
