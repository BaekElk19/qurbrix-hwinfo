use hw_inventory::{InventoryStore, PageRequest, RetentionPolicy, SNAPSHOT_CLI_SCHEMA_VERSION};
use hw_model::{
    CoreIdentityGroup, Device, DeviceKind, DeviceProperties, IdentityCoverage, QuickProbeReport,
    ScanReport, SystemDeviceInfo, BINDID_V2_ALGORITHM, FINGERPRINT_VERSION,
    SNAPSHOT_SCHEMA_VERSION,
};
use std::{fs, time::Duration};

fn report(device_count: usize) -> ScanReport {
    let mut report = ScanReport::empty();
    report.devices = (0..device_count)
        .map(|index| {
            Device::new(
                format!("system:{index:04}"),
                DeviceKind::System,
                format!("System {index:04}"),
                DeviceProperties::System(SystemDeviceInfo {
                    uuid: Some(format!("uuid-{index:04}")),
                    manufacturer: Some("Example".into()),
                    product_name: Some("Fixture".into()),
                    ..SystemDeviceInfo::default()
                }),
            )
        })
        .collect();
    report
}

fn probe(index: usize, machine: char) -> QuickProbeReport {
    QuickProbeReport {
        schema_version: SNAPSHOT_SCHEMA_VERSION.into(),
        fingerprint_version: FINGERPRINT_VERSION,
        bindid_algorithm: BINDID_V2_ALGORITHM.into(),
        machine_bind_id: machine.to_string().repeat(64),
        configuration_fingerprint: format!("{index:064x}"),
        canonical_payload_sha256: format!("{index:064x}"),
        observed_at: "2026-07-23T00:00:00Z".into(),
        identity_records: vec![format!("fixture:{index}")],
        configuration_records: vec![format!("fixture:{index}")],
        coverage: IdentityCoverage {
            covered: CoreIdentityGroup::REQUIRED.to_vec(),
            missing: Vec::new(),
            trusted_absent: Vec::new(),
        },
        warnings: Vec::new(),
    }
}

#[tokio::test]
async fn retention_protects_current_pinned_unuploaded_and_recent() {
    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    let mut ids = Vec::new();
    for index in 0..6 {
        ids.push(
            store
                .publish_snapshot(report(1), probe(index, 'a'))
                .await
                .unwrap(),
        );
    }
    for (index, id) in ids.iter().enumerate() {
        if index != 1 {
            store.mark_uploaded(*id, None).await.unwrap();
        }
    }
    store.set_pinned(ids[2], true).await.unwrap();
    let policy = RetentionPolicy {
        keep_recent_per_machine: 2,
        uploaded_max_age: Duration::ZERO,
        dry_run: true,
    };
    let dry_run = store.apply_retention(policy).await.unwrap();
    assert_eq!(dry_run.eligible, 2);
    assert_eq!(dry_run.database_deleted, 0);

    let removed = store
        .apply_retention(RetentionPolicy {
            dry_run: false,
            ..policy
        })
        .await
        .unwrap();
    assert_eq!(removed.database_deleted, 2);
    assert_eq!(removed.artifacts_deleted, 2);
    assert_eq!(removed.pending_artifact_deletes, 0);
    assert!(store.load_snapshot(ids[5]).await.unwrap().is_some());
    assert!(store.load_snapshot(ids[2]).await.unwrap().is_some());
    assert!(store.load_snapshot(ids[1]).await.unwrap().is_some());
    assert_eq!(store.metrics().await.unwrap().snapshot_count, 4);
}

#[cfg(unix)]
#[tokio::test]
async fn artifact_delete_failure_is_queued_and_retried() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    let mut ids = Vec::new();
    for index in 0..3 {
        let id = store
            .publish_snapshot(report(1), probe(index, 'a'))
            .await
            .unwrap();
        store.mark_uploaded(id, None).await.unwrap();
        ids.push(id);
    }
    let reports_dir = temp.path().join("reports");
    fs::set_permissions(&reports_dir, fs::Permissions::from_mode(0o500)).unwrap();
    let first = store
        .apply_retention(RetentionPolicy {
            keep_recent_per_machine: 1,
            uploaded_max_age: Duration::ZERO,
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(first.database_deleted, 2);
    assert_eq!(first.artifact_delete_failures, 2);
    assert_eq!(first.pending_artifact_deletes, 2);
    assert!(store.load_snapshot(ids[2]).await.unwrap().is_some());

    fs::set_permissions(&reports_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let retry = store
        .apply_retention(RetentionPolicy {
            keep_recent_per_machine: 1,
            uploaded_max_age: Duration::ZERO,
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(retry.artifacts_deleted, 2);
    assert_eq!(retry.pending_artifact_deletes, 0);
}

#[tokio::test]
async fn health_detects_orphan_and_same_size_corruption() {
    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    let id = store
        .publish_snapshot(report(1), probe(1, 'a'))
        .await
        .unwrap();
    let healthy = store.health_check().await.unwrap();
    assert!(healthy.healthy);
    assert_eq!(healthy.sqlite_integrity, "ok");
    assert_eq!(healthy.schema_version, SNAPSHOT_CLI_SCHEMA_VERSION);

    fs::write(temp.path().join("reports/orphan.json"), b"{}").unwrap();
    let orphan = store.health_check().await.unwrap();
    assert!(!orphan.healthy);
    assert_eq!(orphan.orphan_artifacts, 1);
    store.recover_orphan_artifacts().await.unwrap();

    let artifact = store.load_snapshot(id).await.unwrap().unwrap().artifact;
    let path = temp.path().join(artifact.relative_path);
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(path, bytes).unwrap();
    let corrupt = store.health_check().await.unwrap();
    assert!(!corrupt.healthy);
    assert_eq!(corrupt.corrupt_artifacts, 1);
}

#[tokio::test]
async fn metrics_and_wal_checkpoint_bound_long_running_state() {
    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    for index in 0..20 {
        store
            .publish_snapshot(report(3), probe(index, 'a'))
            .await
            .unwrap();
    }
    let metrics = store.metrics().await.unwrap();
    assert_eq!(metrics.snapshot_count, 20);
    assert_eq!(metrics.device_count, 60);
    assert!(metrics.artifact_bytes > 0);
    let checkpoint = store.wal_checkpoint().await.unwrap();
    assert_eq!(checkpoint.busy, 0);
    assert!(checkpoint.checkpointed_frames <= checkpoint.log_frames);
    eprintln!(
        "inventory_growth snapshots=20 devices=60 db_bytes={} artifact_bytes={} wal_log_frames={} wal_checkpointed_frames={}",
        fs::metadata(store.database_path()).unwrap().len(),
        metrics.artifact_bytes,
        checkpoint.log_frames,
        checkpoint.checkpointed_frames
    );
}

#[tokio::test]
async fn thousand_device_publish_and_paginated_query_are_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    let started = std::time::Instant::now();
    let id = store
        .publish_snapshot(report(1_000), probe(1, 'a'))
        .await
        .unwrap();
    let publish_ms = started.elapsed().as_millis();
    let started = std::time::Instant::now();
    let devices = store
        .list_devices(
            id,
            PageRequest {
                limit: 1_000,
                offset: 0,
            },
        )
        .await
        .unwrap();
    let query_ms = started.elapsed().as_millis();
    assert_eq!(devices.len(), 1_000);
    assert!(publish_ms < 10_000, "publish took {publish_ms} ms");
    assert!(query_ms < 2_000, "query took {query_ms} ms");
    assert!(fs::metadata(store.database_path()).unwrap().len() < 16 * 1024 * 1024);
    eprintln!(
        "inventory_perf publish_ms={publish_ms} query_ms={query_ms} db_bytes={}",
        fs::metadata(store.database_path()).unwrap().len()
    );
}
