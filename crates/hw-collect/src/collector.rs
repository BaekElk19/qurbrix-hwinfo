use crate::{merge::dedup_devices, status::status_from_warnings};
use anyhow::Result;
use hw_model::{DeviceKind, ScanConfig, ScanReport};
use hw_probe::{
    other_pci_from, AudioProbe, BatteryProbe, BiosProbe, BluetoothProbe, CameraProbe, CdromProbe,
    CpuProbe, GpuProbe, InputProbe, MemoryProbe, MonitorProbe, NetworkProbe, PciProbe,
    PrinterProbe, Probe, ProbeContext, StorageProbe, UsbProbe,
};
use hw_source::{RealSourceRunner, SourceRunner};
use std::collections::HashSet;

pub async fn collect_scan_report(config: ScanConfig) -> Result<ScanReport> {
    let runner = RealSourceRunner;
    collect_scan_report_with_runner(&runner, config).await
}

pub async fn collect_scan_report_with_runner(
    runner: &dyn SourceRunner,
    config: ScanConfig,
) -> Result<ScanReport> {
    let ctx = ProbeContext::new(runner, config.timeout);
    let probes: Vec<Box<dyn Probe>> = vec![
        Box::new(PciProbe),
        Box::new(UsbProbe),
        Box::new(CpuProbe),
        Box::new(MemoryProbe),
        Box::new(BiosProbe),
        Box::new(GpuProbe),
        Box::new(MonitorProbe),
        Box::new(StorageProbe),
        Box::new(NetworkProbe),
        Box::new(AudioProbe),
        Box::new(BluetoothProbe),
        Box::new(InputProbe),
        Box::new(CameraProbe),
        Box::new(BatteryProbe),
        Box::new(PrinterProbe),
        Box::new(CdromProbe),
    ];

    let mut devices = Vec::new();
    let mut warnings = Vec::new();
    let mut consumed = Vec::new();
    for probe in probes {
        if let Some(kinds) = &config.kinds {
            if !probe.kinds().iter().any(|kind| kinds.contains(kind)) {
                continue;
            }
        }
        if probe
            .kinds()
            .iter()
            .any(|kind| config.exclude_kinds.contains(kind))
        {
            continue;
        }
        let mut result = probe.probe(&ctx).await;
        devices.append(&mut result.devices);
        warnings.append(&mut result.warnings);
        consumed.append(&mut result.consumed);
    }

    let mut devices = dedup_devices(devices);
    if config.kinds.is_none() {
        let consumed_ids = consumed
            .into_iter()
            .map(|device_ref| device_ref.id)
            .collect::<HashSet<_>>();
        devices = devices
            .into_iter()
            .filter_map(|device| {
                if device.kind == DeviceKind::Pci {
                    if consumed_ids.contains(&device.id) {
                        None
                    } else {
                        Some(other_pci_from(&device))
                    }
                } else {
                    Some(device)
                }
            })
            .collect();
    }
    let mut report = ScanReport::empty();
    report.status = status_from_warnings(&warnings, devices.len());
    if !config.include_sources {
        for device in &mut devices {
            device.sources.clear();
        }
    }
    report.devices = devices;
    report.warnings = if config.include_warnings {
        warnings
    } else {
        Vec::new()
    };
    Ok(report)
}
