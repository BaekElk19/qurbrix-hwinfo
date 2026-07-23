use crate::{canonicalize_devices, error::Result};
use hw_model::{CoreIdentityGroup, Device, QuickProbeReport};
use hw_probe::{
    BiosProbe, CpuProbe, GpuProbe, MemoryProbe, NetworkProbe, Probe, ProbeContext, ProbeResult,
    StorageProbe, SystemProbe,
};
use hw_source::{RealSourceRunner, SourceRunner};
use std::{collections::BTreeSet, time::Duration};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickProbeConfig {
    pub timeout: Duration,
}

impl Default for QuickProbeConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
        }
    }
}

pub async fn quick_probe(config: QuickProbeConfig) -> Result<QuickProbeReport> {
    quick_probe_with_runner(&RealSourceRunner, config).await
}

pub async fn quick_probe_with_runner(
    runner: &dyn SourceRunner,
    config: QuickProbeConfig,
) -> Result<QuickProbeReport> {
    let context = ProbeContext::new(runner, config.timeout);
    let mut devices = Vec::new();
    let mut warnings = Vec::new();
    let mut trusted_absent = BTreeSet::new();

    let system = SystemProbe.probe(&context).await;
    append_result(system, &mut devices, &mut warnings);
    let bios = BiosProbe.probe(&context).await;
    append_result(bios, &mut devices, &mut warnings);
    run_group(
        CpuProbe,
        CoreIdentityGroup::Cpu,
        &context,
        &mut devices,
        &mut warnings,
        &mut trusted_absent,
    )
    .await;
    run_group(
        MemoryProbe,
        CoreIdentityGroup::Memory,
        &context,
        &mut devices,
        &mut warnings,
        &mut trusted_absent,
    )
    .await;
    run_group(
        StorageProbe,
        CoreIdentityGroup::Storage,
        &context,
        &mut devices,
        &mut warnings,
        &mut trusted_absent,
    )
    .await;
    run_group(
        NetworkProbe,
        CoreIdentityGroup::PhysicalNetwork,
        &context,
        &mut devices,
        &mut warnings,
        &mut trusted_absent,
    )
    .await;
    run_group(
        GpuProbe,
        CoreIdentityGroup::Gpu,
        &context,
        &mut devices,
        &mut warnings,
        &mut trusted_absent,
    )
    .await;

    if !devices.iter().any(|device| {
        matches!(
            device.kind,
            hw_model::DeviceKind::System | hw_model::DeviceKind::Motherboard
        )
    }) && warnings.is_empty()
    {
        trusted_absent.insert(CoreIdentityGroup::Platform);
    }
    canonicalize_devices(&devices, warnings, trusted_absent, now_rfc3339())
}

async fn run_group(
    probe: impl Probe,
    group: CoreIdentityGroup,
    context: &ProbeContext<'_>,
    devices: &mut Vec<Device>,
    warnings: &mut Vec<String>,
    trusted_absent: &mut BTreeSet<CoreIdentityGroup>,
) {
    let result = probe.probe(context).await;
    if result.devices.is_empty() && result.warnings.is_empty() {
        trusted_absent.insert(group);
    }
    append_result(result, devices, warnings);
}

fn append_result(result: ProbeResult, devices: &mut Vec<Device>, warnings: &mut Vec<String>) {
    devices.extend(result.devices);
    warnings.extend(result.warnings.into_iter().map(|warning| {
        let source = warning
            .source
            .map(|source| format!(" [{source}]"))
            .unwrap_or_default();
        format!("{}: {}{source}", warning.code, warning.message)
    }));
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
