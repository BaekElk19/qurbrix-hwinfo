use async_trait::async_trait;
use hw_collect::{collect_scan_report_with_runner_and_options, ScanExecutionOptions};
use hw_model::{DeviceKind, ScanConfig};
use hw_source::{
    CommandSpec, FakeSourceRunner, GlobResult, SourceBytesResult, SourceResult, SourceRunner,
};
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

struct DelayedRunner {
    inner: FakeSourceRunner,
    delay: Duration,
    active: AtomicUsize,
    active_commands: AtomicUsize,
    peak_commands: AtomicUsize,
}

struct ActivityGuard<'a> {
    counter: &'a AtomicUsize,
}

impl Drop for ActivityGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

impl DelayedRunner {
    fn fixture(delay: Duration) -> Self {
        Self {
            inner: FakeSourceRunner::new()
                .with_command(
                    "lspci",
                    ["-nn", "-k"],
                    "00:1f.3 Audio device [0403]: Intel HD Audio [8086:a348]\n\tKernel driver in use: snd_hda_intel\n",
                )
                .with_command(
                    "lsusb",
                    std::iter::empty::<&str>(),
                    "Bus 001 Device 004: ID 0bda:5689 Realtek Camera\n",
                )
                .with_file(
                    "/proc/asound/cards",
                    " 0 [PCH]: HDA-Intel - HDA Intel PCH\n",
                )
                .with_file(
                    "/proc/bus/input/devices",
                    "N: Name=\"AT Keyboard\"\nH: Handlers=kbd event0\n\n",
                ),
            delay,
            active: AtomicUsize::new(0),
            active_commands: AtomicUsize::new(0),
            peak_commands: AtomicUsize::new(0),
        }
    }

    async fn delay(&self) -> ActivityGuard<'_> {
        self.active.fetch_add(1, Ordering::SeqCst);
        let guard = ActivityGuard {
            counter: &self.active,
        };
        tokio::time::sleep(self.delay).await;
        guard
    }
}

#[async_trait]
impl SourceRunner for DelayedRunner {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        let active = self.active_commands.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_commands.fetch_max(active, Ordering::SeqCst);
        let command_guard = ActivityGuard {
            counter: &self.active_commands,
        };
        let activity_guard = self.delay().await;
        let result = self.inner.run_command(command, timeout).await;
        drop(activity_guard);
        drop(command_guard);
        result
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        let guard = self.delay().await;
        let result = self.inner.read_file(path).await;
        drop(guard);
        result
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        let guard = self.delay().await;
        let result = self.inner.read_file_bytes(path).await;
        drop(guard);
        result
    }

    async fn canonicalize_path(&self, path: &Path) -> SourceResult {
        let guard = self.delay().await;
        let result = self.inner.canonicalize_path(path).await;
        drop(guard);
        result
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        let guard = self.delay().await;
        let result = self.inner.glob(pattern).await;
        drop(guard);
        result
    }
}

fn fixture_config() -> ScanConfig {
    ScanConfig {
        kinds: Some(vec![
            DeviceKind::Pci,
            DeviceKind::Usb,
            DeviceKind::Audio,
            DeviceKind::Input,
        ]),
        timeout: Duration::from_secs(1),
        ..ScanConfig::default()
    }
}

#[tokio::test]
async fn serial_and_parallel_reports_are_semantically_identical() {
    let runner = DelayedRunner::fixture(Duration::from_millis(2));
    let mut serial = collect_scan_report_with_runner_and_options(
        &runner,
        fixture_config(),
        ScanExecutionOptions::serial_baseline(),
    )
    .await
    .unwrap()
    .report;
    let mut parallel = collect_scan_report_with_runner_and_options(
        &runner,
        fixture_config(),
        ScanExecutionOptions::default(),
    )
    .await
    .unwrap()
    .report;
    serial.metadata.duration_ms = None;
    parallel.metadata.duration_ms = None;
    assert_eq!(parallel, serial);
}

#[tokio::test]
async fn delayed_fixture_improves_by_at_least_twenty_five_percent() {
    let runner = DelayedRunner::fixture(Duration::from_millis(10));
    let started = Instant::now();
    collect_scan_report_with_runner_and_options(
        &runner,
        fixture_config(),
        ScanExecutionOptions::serial_baseline(),
    )
    .await
    .unwrap();
    let serial = started.elapsed();
    let started = Instant::now();
    collect_scan_report_with_runner_and_options(
        &runner,
        fixture_config(),
        ScanExecutionOptions::default(),
    )
    .await
    .unwrap();
    let parallel_duration = started.elapsed();
    assert!(parallel_duration.as_nanos() * 4 < serial.as_nanos() * 3);
}

#[tokio::test]
async fn shared_lspci_source_is_executed_once_per_scan() {
    let runner = DelayedRunner::fixture(Duration::from_millis(10));
    let collection = collect_scan_report_with_runner_and_options(
        &runner,
        ScanConfig {
            kinds: Some(vec![DeviceKind::Pci, DeviceKind::Gpu]),
            timeout: Duration::from_secs(1),
            ..ScanConfig::default()
        },
        ScanExecutionOptions::default(),
    )
    .await
    .unwrap();
    assert!(collection.source_metrics.cache_hits >= 1);
    assert_eq!(
        collection
            .source_metrics
            .observations
            .iter()
            .filter(|observation| observation.source == "lspci -nn -k")
            .count(),
        2
    );
}

#[tokio::test]
async fn configured_external_command_peak_is_enforced() {
    let runner = DelayedRunner::fixture(Duration::from_millis(20));
    let collection = collect_scan_report_with_runner_and_options(
        &runner,
        fixture_config(),
        ScanExecutionOptions {
            max_external_commands: 2,
            ..ScanExecutionOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(runner.peak_commands.load(Ordering::SeqCst) <= 2);
    assert!(collection.source_metrics.peak_external_commands <= 2);
}

#[tokio::test]
async fn deadline_cancels_probes_without_residual_source_activity() {
    let runner = DelayedRunner::fixture(Duration::from_millis(250));
    let started = Instant::now();
    let collection = collect_scan_report_with_runner_and_options(
        &runner,
        fixture_config(),
        ScanExecutionOptions {
            global_deadline: Some(Duration::from_millis(30)),
            ..ScanExecutionOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(200));
    assert!(collection.deadline_exceeded);
    assert!(collection
        .probe_metrics
        .iter()
        .any(|metric| metric.timed_out));
    assert_eq!(runner.active.load(Ordering::SeqCst), 0);
    assert_eq!(runner.active_commands.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn kind_filters_are_preserved_by_parallel_graph() {
    let runner = DelayedRunner::fixture(Duration::from_millis(1));
    let collection = collect_scan_report_with_runner_and_options(
        &runner,
        ScanConfig {
            kinds: Some(vec![DeviceKind::Usb, DeviceKind::Pci]),
            exclude_kinds: vec![DeviceKind::Usb],
            ..ScanConfig::default()
        },
        ScanExecutionOptions::default(),
    )
    .await
    .unwrap();
    assert!(collection
        .report
        .devices
        .iter()
        .all(|device| device.kind != DeviceKind::Usb));
    assert_eq!(collection.probe_metrics.len(), 1);
    assert_eq!(collection.probe_metrics[0].name, "pci");
}
