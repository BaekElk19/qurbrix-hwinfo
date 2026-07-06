use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CdromInfo, Device, DeviceKind, DeviceProperties, ScanWarning, SourceEvidence,
    SourceKind, SourceStatus,
};
use hw_parser::{parse_lshw_cdrom, parse_proc_cdrom_info, LshwCdromRecord};
use hw_source::CommandSpec;
use std::{collections::HashMap, path::Path};

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
        let lshw = cdrom_lshw_records(ctx).await;
        let result = ctx
            .runner
            .read_file(Path::new("/proc/sys/dev/cdrom/info"))
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = probe_sysfs_cdroms(ctx, &lshw).await;
            return fallback;
        }
        let info = parse_proc_cdrom_info(&result.stdout);
        if info.drive_names.is_empty() {
            return ProbeResult {
                devices: probe_sysfs_cdroms(ctx, &lshw).await,
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "cdrom source produced no drive records",
                )
                .with_source(result.source)],
                consumed: Vec::new(),
            };
        }
        let mut devices = Vec::new();
        for drive in info.drive_names {
            let mut device = Device::new(
                device_id::other("cdrom", &drive),
                DeviceKind::Cdrom,
                drive.clone(),
                DeviceProperties::Cdrom(CdromInfo {
                    device_node: Some(format!("/dev/{drive}")),
                    media_present: None,
                    firmware: None,
                    capabilities: info.capabilities.clone(),
                }),
            )
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
            let sysfs_path = Path::new("/sys/class/block").join(&drive);
            device.vendor = read_trimmed(ctx, &sysfs_path.join("device/vendor")).await;
            device.model = read_trimmed(ctx, &sysfs_path.join("device/model")).await;
            device.serial = read_trimmed(ctx, &sysfs_path.join("device/serial")).await;
            let firmware = read_trimmed(ctx, &sysfs_path.join("device/rev")).await;
            let has_firmware = firmware.is_some();
            if let DeviceProperties::Cdrom(info) = &mut device.properties {
                info.firmware = firmware;
            }
            if device.vendor.is_some()
                || device.model.is_some()
                || device.serial.is_some()
                || has_firmware
            {
                device = device.with_source(SourceEvidence {
                    source: sysfs_path.display().to_string(),
                    kind: SourceKind::Sysfs,
                    status: SourceStatus::Success,
                    summary: None,
                });
            }
            devices.push(apply_cdrom_lshw_enrichment(device, &lshw));
        }
        ProbeResult::with_devices(devices)
    }
}

#[derive(Default)]
struct CdromLshwRecords {
    source: String,
    by_node: HashMap<String, LshwCdromRecord>,
}

async fn cdrom_lshw_records(ctx: &ProbeContext<'_>) -> CdromLshwRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("lshw", ["-class", "disk"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return CdromLshwRecords::default();
    }

    let by_node = parse_lshw_cdrom(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.logical_name.clone()?, record)))
        .collect();
    CdromLshwRecords {
        source: result.source,
        by_node,
    }
}

fn apply_cdrom_lshw_enrichment(mut device: Device, lshw: &CdromLshwRecords) -> Device {
    let Some(node) = (match &device.properties {
        DeviceProperties::Cdrom(info) => info.device_node.clone(),
        _ => None,
    }) else {
        return device;
    };
    let Some(record) = lshw.by_node.get(&node) else {
        return device;
    };
    let mut contributed = false;

    if device.vendor.is_none() && record.vendor.is_some() {
        device.vendor = record.vendor.clone();
        contributed = true;
    }
    if device.model.is_none() && record.product.is_some() {
        device.model = record.product.clone();
        if device.name == node || node.ends_with(&format!("/{}", device.name)) {
            device.name = record.product.clone().unwrap_or(device.name);
        }
        contributed = true;
    }
    if device.serial.is_none() && record.serial.is_some() {
        device.serial = record.serial.clone();
        contributed = true;
    }
    if let DeviceProperties::Cdrom(info) = &mut device.properties {
        if info.firmware.is_none() && record.firmware.is_some() {
            info.firmware = record.firmware.clone();
            contributed = true;
        }
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

async fn probe_sysfs_cdroms(ctx: &ProbeContext<'_>, lshw: &CdromLshwRecords) -> Vec<Device> {
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
                firmware: read_trimmed(ctx, &path.join("device/rev")).await,
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
        devices.push(apply_cdrom_lshw_enrichment(device, lshw));
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
