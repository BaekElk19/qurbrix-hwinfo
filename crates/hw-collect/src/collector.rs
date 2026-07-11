use crate::{merge::dedup_devices, status::status_from_warnings};
use anyhow::Result;
use hw_model::{device_id, BusInfo, Device, DeviceKind, DeviceProperties, ScanConfig, ScanReport};
use hw_probe::{
    other_pci_from, AudioProbe, BatteryProbe, BiosProbe, BluetoothProbe, CameraProbe, CdromProbe,
    CpuProbe, GpuProbe, InputProbe, MemoryProbe, MonitorProbe, NetworkProbe, PciProbe,
    PrinterProbe, Probe, ProbeContext, StorageProbe, SystemProbe, UsbProbe,
};
use hw_source::{RealSourceRunner, SourceRunner};
use std::collections::HashSet;
use std::time::Instant;

pub async fn collect_scan_report(config: ScanConfig) -> Result<ScanReport> {
    let runner = RealSourceRunner;
    collect_scan_report_with_runner(&runner, config).await
}

pub async fn collect_scan_report_with_runner(
    runner: &dyn SourceRunner,
    config: ScanConfig,
) -> Result<ScanReport> {
    let started = Instant::now();
    let ctx = ProbeContext::new(runner, config.timeout);
    let probes: Vec<Box<dyn Probe>> = vec![
        Box::new(SystemProbe),
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
        let mut consumed_ids = consumed
            .into_iter()
            .map(|device_ref| device_ref.id)
            .collect::<HashSet<_>>();
        consumed_ids.extend(devices.iter().filter_map(backing_pci_id));
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
    report.metadata.scanner_version = Some(env!("CARGO_PKG_VERSION").to_string());
    report.metadata.duration_ms = Some(elapsed_millis(started));
    populate_system_metadata(&mut report, &devices);
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

fn populate_system_metadata(report: &mut ScanReport, devices: &[Device]) {
    let Some(info) = devices.iter().find_map(|device| match &device.properties {
        DeviceProperties::System(info) => Some(info),
        _ => None,
    }) else {
        return;
    };

    report.system.hostname = info.hostname.clone();
    report.system.os = info.os.clone();
    report.system.kernel = info.kernel.clone();
    report.system.architecture = info.architecture.clone();
    report.metadata.hostname = report.system.hostname.clone();
    report.metadata.os = report.system.os.clone();
    report.metadata.kernel = report.system.kernel.clone();
    report.metadata.architecture = report.system.architecture.clone();
}

fn backing_pci_id(device: &Device) -> Option<String> {
    if device.kind == DeviceKind::Pci {
        return None;
    }
    let Some(BusInfo::Pci { address, .. }) = &device.bus else {
        return None;
    };
    Some(device_id::pci(address))
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
