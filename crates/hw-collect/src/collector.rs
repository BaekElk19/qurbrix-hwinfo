use crate::{
    execution::{ProbeMetrics, ScanCollection, ScanExecutionOptions},
    merge::dedup_devices,
    status::status_from_warnings,
};
use anyhow::Result;
use futures::{stream::FuturesUnordered, StreamExt};
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, ScanConfig, ScanReport, ScanWarning,
};
use hw_probe::{
    other_pci_from, AudioProbe, BatteryProbe, BiosProbe, BluetoothProbe, CameraProbe, CdromProbe,
    CpuProbe, GpuProbe, InputProbe, MemoryProbe, MonitorProbe, NetworkProbe, PciProbe,
    PrinterProbe, Probe, ProbeContext, ProbeResult, StorageProbe, SystemProbe, UsbProbe,
};
use hw_source::{CachedSourceRunner, RealSourceRunner, SourceRunner};
use std::{collections::HashSet, future::Future, pin::Pin, time::Instant};

pub async fn collect_scan_report(config: ScanConfig) -> Result<ScanReport> {
    collect_scan_report_with_options(config, ScanExecutionOptions::default()).await
}

pub async fn collect_scan_report_with_options(
    config: ScanConfig,
    options: ScanExecutionOptions,
) -> Result<ScanReport> {
    Ok(collect_scan_report_detailed(config, options).await?.report)
}

pub async fn collect_scan_report_detailed(
    config: ScanConfig,
    options: ScanExecutionOptions,
) -> Result<ScanCollection> {
    collect_scan_report_with_runner_and_options(&RealSourceRunner, config, options).await
}

pub async fn collect_scan_report_with_runner(
    runner: &dyn SourceRunner,
    config: ScanConfig,
) -> Result<ScanReport> {
    Ok(
        collect_scan_report_with_runner_and_options(
            runner,
            config,
            ScanExecutionOptions::default(),
        )
        .await?
        .report,
    )
}

pub async fn collect_scan_report_with_runner_and_options(
    runner: &dyn SourceRunner,
    config: ScanConfig,
    options: ScanExecutionOptions,
) -> Result<ScanCollection> {
    let started = Instant::now();
    let deadline = options
        .global_deadline
        .map(|duration| tokio::time::Instant::now() + duration);
    let cached_runner = CachedSourceRunner::new(
        runner,
        options.max_external_commands,
        deadline,
        options.cache_sources,
    );
    let context = ProbeContext::new(&cached_runner, config.timeout);
    let probes = selected_probes(&config);
    let probe_count = probes.len();
    let (results, mut probe_metrics, deadline_exceeded) = if options.parallel_probes {
        run_parallel(probes, &context, deadline).await
    } else {
        run_serial(probes, &context, deadline).await
    };

    let mut devices = Vec::new();
    let mut warnings = Vec::new();
    let mut consumed = Vec::new();
    for result in results.into_iter().flatten() {
        devices.extend(result.devices);
        warnings.extend(result.warnings);
        consumed.extend(result.consumed);
    }
    if deadline_exceeded {
        for metric in probe_metrics.iter().filter(|metric| metric.timed_out) {
            warnings.push(
                ScanWarning::new(
                    "scan.probe_deadline",
                    format!(
                        "probe {} did not finish before the global deadline",
                        metric.name
                    ),
                )
                .with_source(metric.name.clone()),
            );
        }
    }
    probe_metrics.sort_by(|left, right| left.name.cmp(&right.name));

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
    debug_assert_eq!(probe_count, probe_metrics.len());
    Ok(ScanCollection {
        report,
        probe_metrics,
        source_metrics: cached_runner.metrics().await,
        deadline_exceeded,
    })
}

type ProbeFuture<'a> = Pin<Box<dyn Future<Output = (usize, String, ProbeResult, u64)> + Send + 'a>>;

async fn run_parallel(
    probes: Vec<(usize, Box<dyn Probe>)>,
    context: &ProbeContext<'_>,
    deadline: Option<tokio::time::Instant>,
) -> (Vec<Option<ProbeResult>>, Vec<ProbeMetrics>, bool) {
    let result_len = probes
        .iter()
        .map(|(index, _)| *index)
        .max()
        .map_or(0, |index| index + 1);
    let mut pending = probes
        .iter()
        .map(|(index, probe)| (*index, probe.name().to_string()))
        .collect::<Vec<_>>();
    let mut futures: FuturesUnordered<ProbeFuture<'_>> = FuturesUnordered::new();
    for (index, probe) in probes {
        futures.push(Box::pin(async move {
            let name = probe.name().to_string();
            let started = Instant::now();
            let result = probe.probe(context).await;
            (index, name, result, elapsed_micros(started))
        }));
    }
    let mut results = std::iter::repeat_with(|| None)
        .take(result_len)
        .collect::<Vec<_>>();
    let mut metrics = Vec::new();
    let mut deadline_exceeded = false;
    while !futures.is_empty() {
        let next = if let Some(deadline) = deadline {
            tokio::select! {
                result = futures.next() => result,
                _ = tokio::time::sleep_until(deadline) => {
                    deadline_exceeded = true;
                    None
                }
            }
        } else {
            futures.next().await
        };
        let Some((index, name, result, duration_micros)) = next else {
            break;
        };
        pending.retain(|(pending_index, _)| *pending_index != index);
        metrics.push(ProbeMetrics {
            name,
            duration_micros,
            device_count: result.devices.len(),
            warning_count: result.warnings.len(),
            timed_out: false,
        });
        results[index] = Some(result);
    }
    drop(futures);
    metrics.extend(pending.into_iter().map(|(_, name)| ProbeMetrics {
        name,
        duration_micros: 0,
        device_count: 0,
        warning_count: 1,
        timed_out: true,
    }));
    (results, metrics, deadline_exceeded)
}

async fn run_serial(
    probes: Vec<(usize, Box<dyn Probe>)>,
    context: &ProbeContext<'_>,
    deadline: Option<tokio::time::Instant>,
) -> (Vec<Option<ProbeResult>>, Vec<ProbeMetrics>, bool) {
    let result_len = probes
        .iter()
        .map(|(index, _)| *index)
        .max()
        .map_or(0, |index| index + 1);
    let mut results = std::iter::repeat_with(|| None)
        .take(result_len)
        .collect::<Vec<_>>();
    let mut metrics = Vec::new();
    let mut deadline_exceeded = false;
    let mut probes = probes.into_iter();
    while let Some((index, probe)) = probes.next() {
        if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
            deadline_exceeded = true;
            metrics.push(timeout_metric(probe.name()));
            metrics.extend(probes.map(|(_, probe)| timeout_metric(probe.name())));
            break;
        }
        let name = probe.name().to_string();
        let started = Instant::now();
        let result = probe.probe(context).await;
        metrics.push(ProbeMetrics {
            name,
            duration_micros: elapsed_micros(started),
            device_count: result.devices.len(),
            warning_count: result.warnings.len(),
            timed_out: false,
        });
        results[index] = Some(result);
    }
    (results, metrics, deadline_exceeded)
}

fn timeout_metric(name: &str) -> ProbeMetrics {
    ProbeMetrics {
        name: name.to_string(),
        duration_micros: 0,
        device_count: 0,
        warning_count: 1,
        timed_out: true,
    }
}

fn selected_probes(config: &ScanConfig) -> Vec<(usize, Box<dyn Probe>)> {
    all_probes()
        .into_iter()
        .enumerate()
        .filter(|(_, probe)| {
            config
                .kinds
                .as_ref()
                .is_none_or(|kinds| probe.kinds().iter().any(|kind| kinds.contains(kind)))
        })
        .filter(|(_, probe)| {
            !probe
                .kinds()
                .iter()
                .any(|kind| config.exclude_kinds.contains(kind))
        })
        .collect()
}

fn all_probes() -> Vec<Box<dyn Probe>> {
    vec![
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
    ]
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

fn elapsed_micros(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}
