use async_trait::async_trait;
use hw_inventory::{
    canonicalize_devices, ensure_snapshot_with_scanner, InventoryError, InventoryStore,
    SnapshotScanner,
};
use hw_model::{
    CoreIdentityGroup, CpuInfo, Device, DeviceKind, DeviceProperties, EnsureSnapshotOptions,
    MemoryInfo, NetworkInfo, PartialPolicy, QuickProbeReport, ScanReport, ScanStatus, StorageInfo,
    SystemDeviceInfo,
};
use rusqlite::Connection;
use std::{
    collections::BTreeSet,
    fs,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tempfile::TempDir;
use tokio::sync::Notify;

fn complete_report(kernel: &str, network_mac: &str) -> ScanReport {
    let mut report = ScanReport::empty();
    report.devices = vec![
        Device::new(
            "system:fixture",
            DeviceKind::System,
            "Fixture",
            DeviceProperties::System(SystemDeviceInfo {
                uuid: Some("system-uuid".into()),
                manufacturer: Some("Example".into()),
                product_name: Some("Machine".into()),
                kernel: Some(kernel.into()),
                architecture: Some("x86_64".into()),
                ..SystemDeviceInfo::default()
            }),
        ),
        Device::new(
            "cpu:0",
            DeviceKind::Cpu,
            "CPU",
            DeviceProperties::Cpu(Box::new(CpuInfo {
                vendor: Some("Example".into()),
                name: Some("C1".into()),
                architecture: Some("x86_64".into()),
                cores: Some(4),
                threads: Some(8),
                ..CpuInfo::default()
            })),
        ),
        Device::new(
            "memory:0",
            DeviceKind::Memory,
            "DIMM",
            DeviceProperties::Memory(MemoryInfo {
                serial: Some("mem-1".into()),
                locator: Some("A0".into()),
                size_bytes: Some(8 * 1024 * 1024 * 1024),
                ..MemoryInfo::default()
            }),
        ),
        {
            let mut device = Device::new(
                "storage:0",
                DeviceKind::Storage,
                "SSD",
                DeviceProperties::Storage(StorageInfo {
                    wwn: Some("wwn-1".into()),
                    firmware: Some("fw-1".into()),
                    ..StorageInfo::default()
                }),
            );
            device.serial = Some("disk-1".into());
            device.model = Some("Disk".into());
            device
        },
        Device::new(
            "network:eth0",
            DeviceKind::Network,
            "Ethernet",
            DeviceProperties::Network(NetworkInfo {
                interface: Some("eth0".into()),
                mac: Some(network_mac.into()),
                ..NetworkInfo::default()
            }),
        ),
    ];
    report
}

fn quick_for(report: &ScanReport) -> QuickProbeReport {
    canonicalize_devices(
        &report.devices,
        Vec::new(),
        BTreeSet::from([CoreIdentityGroup::Gpu]),
        "2026-07-23T00:00:00Z".into(),
    )
    .unwrap()
}

struct FakeScanner {
    quick: QuickProbeReport,
    full: ScanReport,
    quick_fails: AtomicBool,
    full_fails: AtomicBool,
    quick_calls: AtomicUsize,
    full_calls: AtomicUsize,
    full_delay: Duration,
    entered_full: Option<Arc<Notify>>,
    release_full: Option<Arc<Notify>>,
}

impl FakeScanner {
    fn new(report: ScanReport) -> Self {
        Self {
            quick: quick_for(&report),
            full: report,
            quick_fails: AtomicBool::new(false),
            full_fails: AtomicBool::new(false),
            quick_calls: AtomicUsize::new(0),
            full_calls: AtomicUsize::new(0),
            full_delay: Duration::ZERO,
            entered_full: None,
            release_full: None,
        }
    }
}

#[async_trait]
impl SnapshotScanner for FakeScanner {
    async fn quick_probe(&self) -> hw_inventory::Result<QuickProbeReport> {
        self.quick_calls.fetch_add(1, Ordering::SeqCst);
        if self.quick_fails.load(Ordering::SeqCst) {
            Err(InventoryError::InvalidReport("injected quick failure"))
        } else {
            Ok(self.quick.clone())
        }
    }

    async fn full_scan(&self) -> hw_inventory::Result<ScanReport> {
        self.full_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(notify) = &self.entered_full {
            notify.notify_one();
        }
        if let Some(notify) = &self.release_full {
            notify.notified().await;
        }
        if !self.full_delay.is_zero() {
            tokio::time::sleep(self.full_delay).await;
        }
        if self.full_fails.load(Ordering::SeqCst) {
            Err(InventoryError::FullScanFailed)
        } else {
            Ok(self.full.clone())
        }
    }
}

async fn store() -> (TempDir, InventoryStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    (temp, store)
}

#[tokio::test]
async fn first_run_publishes_and_second_run_reuses() {
    let (temp, store) = store().await;
    let scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    let first = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    let second = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(scanner.quick_calls.load(Ordering::SeqCst), 2);
    assert_eq!(scanner.full_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        store.current_state().await.unwrap().current_snapshot_id,
        Some(first)
    );
    let connection = Connection::open(temp.path().join("qurbrix_hwinfo.db")).unwrap();
    let running: i64 = connection
        .query_row(
            "SELECT count(*) FROM probe_history WHERE status = 'running'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let linked: i64 = connection
        .query_row(
            "SELECT count(*) FROM probe_history WHERE snapshot_id = ?1",
            [first.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(running, 0);
    assert_eq!(linked, 3);
}

#[tokio::test]
async fn force_and_zero_ttl_each_publish_new_snapshot() {
    let (_temp, store) = store().await;
    let scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    let first = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    let forced = ensure_snapshot_with_scanner(
        &store,
        EnsureSnapshotOptions {
            force_full_scan: true,
            ..EnsureSnapshotOptions::default()
        },
        &scanner,
    )
    .await
    .unwrap();
    let expired = ensure_snapshot_with_scanner(
        &store,
        EnsureSnapshotOptions {
            max_snapshot_age: Some(Duration::ZERO),
            ..EnsureSnapshotOptions::default()
        },
        &scanner,
    )
    .await
    .unwrap();
    assert_ne!(first, forced);
    assert_ne!(forced, expired);
    assert_eq!(scanner.full_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn physical_and_configuration_changes_have_distinct_identity_semantics() {
    let (_temp, store) = store().await;
    let original = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    let first = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &original)
        .await
        .unwrap();
    let first_stored = store.load_snapshot(first).await.unwrap().unwrap();

    let config_change = FakeScanner::new(complete_report("6.7", "00:11:22:33:44:55"));
    let second =
        ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &config_change)
            .await
            .unwrap();
    let second_stored = store.load_snapshot(second).await.unwrap().unwrap();
    assert_eq!(first_stored.machine_bind_id, second_stored.machine_bind_id);
    assert_ne!(
        first_stored.configuration_fingerprint,
        second_stored.configuration_fingerprint
    );

    let physical_change = FakeScanner::new(complete_report("6.7", "00:11:22:33:44:66"));
    let third =
        ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &physical_change)
            .await
            .unwrap();
    let third_stored = store.load_snapshot(third).await.unwrap().unwrap();
    assert_ne!(second_stored.machine_bind_id, third_stored.machine_bind_id);
}

#[tokio::test]
async fn quick_failure_falls_back_to_full_scan() {
    let (_temp, store) = store().await;
    let scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    scanner.quick_fails.store(true, Ordering::SeqCst);
    let id = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    assert!(store.load_snapshot(id).await.unwrap().is_some());
    assert_eq!(scanner.full_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn full_failure_retains_previous_snapshot_and_returns_error() {
    let (_temp, store) = store().await;
    let scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    let first = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    let changed = FakeScanner::new(complete_report("6.7", "00:11:22:33:44:55"));
    changed.full_fails.store(true, Ordering::SeqCst);
    assert!(matches!(
        ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &changed)
            .await
            .unwrap_err(),
        InventoryError::FullScanFailed
    ));
    assert_eq!(
        store.current_state().await.unwrap().current_snapshot_id,
        Some(first)
    );
    assert!(store.load_scan_report(first).await.unwrap().is_some());
}

#[tokio::test]
async fn partial_policy_publishes_core_complete_and_rejects_when_requested() {
    let (_temp, store) = store().await;
    let mut partial = complete_report("6.6", "00:11:22:33:44:55");
    partial.status = ScanStatus::Partial;
    partial.warnings.push(hw_model::ScanWarning::new(
        "optional_missing",
        "optional source",
    ));
    let scanner = FakeScanner::new(partial);
    let published =
        ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
            .await
            .unwrap();
    assert_eq!(
        store
            .load_snapshot(published)
            .await
            .unwrap()
            .unwrap()
            .scan_status,
        hw_model::PublishedScanStatus::Partial
    );

    let changed = FakeScanner::new({
        let mut report = complete_report("6.7", "00:11:22:33:44:55");
        report.status = ScanStatus::Partial;
        report.warnings.push(hw_model::ScanWarning::new(
            "optional_missing",
            "optional source",
        ));
        report
    });
    assert!(matches!(
        ensure_snapshot_with_scanner(
            &store,
            EnsureSnapshotOptions {
                partial_policy: PartialPolicy::Reject,
                ..EnsureSnapshotOptions::default()
            },
            &changed,
        )
        .await
        .unwrap_err(),
        InventoryError::PartialRejected
    ));
    assert_eq!(
        store.current_state().await.unwrap().current_snapshot_id,
        Some(published)
    );
}

#[tokio::test]
async fn incomplete_core_is_not_published() {
    let (_temp, store) = store().await;
    let mut report = complete_report("6.6", "00:11:22:33:44:55");
    report
        .devices
        .retain(|device| device.kind != DeviceKind::Storage);
    let scanner = FakeScanner::new(report);
    assert!(matches!(
        ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
            .await
            .unwrap_err(),
        InventoryError::CoreIdentityIncomplete
    ));
    assert!(store
        .current_state()
        .await
        .unwrap()
        .current_snapshot_id
        .is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_callers_publish_once_and_share_id() {
    let (_temp, store) = store().await;
    let mut scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    scanner.full_delay = Duration::from_millis(80);
    let scanner = Arc::new(scanner);
    let store = Arc::new(store);
    let tasks = (0..8)
        .map(|_| {
            let store = store.clone();
            let scanner = scanner.clone();
            tokio::spawn(async move {
                ensure_snapshot_with_scanner(
                    &store,
                    EnsureSnapshotOptions::default(),
                    scanner.as_ref(),
                )
                .await
                .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let mut ids = Vec::new();
    for task in tasks {
        ids.push(task.await.unwrap());
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    assert_eq!(scanner.full_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn expired_lease_is_recovered_without_waiting() {
    let (_temp, store) = store().await;
    assert!(store
        .try_acquire_lease("dead-owner".into(), Duration::ZERO)
        .await
        .unwrap());
    let scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    let started = std::time::Instant::now();
    ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_scan_does_not_hold_sqlite_write_transaction() {
    let (temp, store) = store().await;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    scanner.entered_full = Some(entered.clone());
    scanner.release_full = Some(release.clone());
    let store_for_task = store.clone();
    let task = tokio::spawn(async move {
        ensure_snapshot_with_scanner(&store_for_task, EnsureSnapshotOptions::default(), &scanner)
            .await
    });
    entered.notified().await;
    let db_path = temp.path().join("qurbrix_hwinfo.db");
    tokio::task::spawn_blocking(move || {
        let connection = Connection::open(db_path).unwrap();
        connection
            .execute(
                "INSERT INTO probe_history(probe_type, started_at, finished_at, status) VALUES ('quick', '2026-07-23T00:00:00Z', '2026-07-23T00:00:00Z', 'succeeded')",
                [],
            )
            .unwrap();
    })
    .await
    .unwrap();
    release.notify_one();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn tampered_current_artifact_triggers_replacement() {
    let (temp, store) = store().await;
    let scanner = FakeScanner::new(complete_report("6.6", "00:11:22:33:44:55"));
    let first = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    let artifact = store.load_snapshot(first).await.unwrap().unwrap().artifact;
    let path = temp.path().join(artifact.relative_path);
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(path, bytes).unwrap();
    let second = ensure_snapshot_with_scanner(&store, EnsureSnapshotOptions::default(), &scanner)
        .await
        .unwrap();
    assert_ne!(first, second);
}

#[tokio::test]
async fn startup_marks_old_running_probe_failed() {
    let (temp, store) = store().await;
    let id = store
        .start_probe(hw_inventory::ProbeKind::Full, None)
        .await
        .unwrap();
    let connection = Connection::open(store.database_path()).unwrap();
    connection
        .execute(
            "UPDATE probe_history SET started_at = '1970-01-01T00:00:00Z' WHERE probe_id = ?1",
            [id],
        )
        .unwrap();
    drop(connection);
    InventoryStore::open(temp.path()).await.unwrap();
    let connection = Connection::open(store.database_path()).unwrap();
    let (status, code): (String, Option<String>) = connection
        .query_row(
            "SELECT status, error_code FROM probe_history WHERE probe_id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(code.as_deref(), Some("inventory.process_interrupted"));
}
