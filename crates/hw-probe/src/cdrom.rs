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
            return ProbeResult::default();
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
