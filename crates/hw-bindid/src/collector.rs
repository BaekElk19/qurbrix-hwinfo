use crate::{devices::component_keys_from_devices, model::BindIdReport};
use anyhow::Result;
use hw_model::Device;
use hw_probe::{
    BiosProbe, GpuProbe, MemoryProbe, NetworkProbe, Probe, ProbeContext, StorageProbe, SystemProbe,
};
use hw_source::{RealSourceRunner, SourceRunner};
use std::time::Duration;

pub async fn collect_bindid_report(timeout: Duration) -> Result<BindIdReport> {
    let runner = RealSourceRunner;
    collect_bindid_report_with_runner(&runner, timeout).await
}

pub async fn collect_bindid_report_with_runner(
    runner: &dyn SourceRunner,
    timeout: Duration,
) -> Result<BindIdReport> {
    let ctx = ProbeContext::new(runner, timeout);
    let probes: Vec<Box<dyn Probe>> = vec![
        Box::new(SystemProbe),
        Box::new(BiosProbe),
        Box::new(MemoryProbe),
        Box::new(StorageProbe),
        Box::new(NetworkProbe),
        Box::new(GpuProbe),
    ];
    let mut devices: Vec<Device> = Vec::new();
    let mut warnings = Vec::new();

    for probe in probes {
        let mut result = probe.probe(&ctx).await;
        devices.append(&mut result.devices);
        warnings.extend(result.warnings.into_iter().map(|warning| warning.message));
    }

    Ok(BindIdReport::from_parts(
        component_keys_from_devices(&devices),
        warnings,
    ))
}
