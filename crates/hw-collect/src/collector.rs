use crate::{merge::dedup_devices, status::status_from_warnings};
use anyhow::Result;
use hw_model::{ScanConfig, ScanReport};
use hw_probe::{
    AudioProbe, BatteryProbe, BiosProbe, BluetoothProbe, CameraProbe, CdromProbe, CpuProbe,
    GpuProbe, InputProbe, MemoryProbe, MonitorProbe, NetworkProbe, PciProbe, PrinterProbe, Probe,
    ProbeContext, StorageProbe, UsbProbe,
};
use hw_source::{RealSourceRunner, SourceRunner};

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
    }

    let mut devices = dedup_devices(devices);
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
