use hw_inventory::{InventoryError, InventoryStore, PageRequest};
use hw_model::{
    CoreIdentityGroup, Device, DeviceKind, DeviceProperties, IdentityCoverage, QuickProbeReport,
    ScanReport, ScanStatus, SnapshotId, SystemDeviceInfo, BINDID_V2_ALGORITHM,
    FINGERPRINT_VERSION, SNAPSHOT_SCHEMA_VERSION,
};
use rusqlite::Connection;
use std::{fs, path::Path};
use tempfile::TempDir;

fn report() -> ScanReport {
    let mut report = ScanReport::empty();
    report.metadata.scanner_version = Some("test-scanner".into());
    report.metadata.duration_ms = Some(42);
    report.status = ScanStatus::Complete;
    let mut system = Device::new(
        "system:fixture",
        DeviceKind::System,
        "Fixture System",
        DeviceProperties::System(SystemDeviceInfo {
            uuid: Some("ABC-123".into()),
            manufacturer: Some("Example".into()),
            product_name: Some("Workstation".into()),
            ..SystemDeviceInfo::default()
        }),
    );
    system.vendor = Some("Example".into());
    system.model = Some("Workstation".into());
    system.identifiers.push(hw_model::DeviceIdentifier {
        kind: "system_uuid".into(),
        value: "abc-123".into(),
    });
    system.capabilities = vec!["fixture".into()];
    report.devices.push(system);
    report
}

fn probe() -> QuickProbeReport {
    QuickProbeReport {
        schema_version: SNAPSHOT_SCHEMA_VERSION.into(),
        fingerprint_version: FINGERPRINT_VERSION,
        bindid_algorithm: BINDID_V2_ALGORITHM.into(),
        machine_bind_id: "a".repeat(64),
        configuration_fingerprint: "b".repeat(64),
        canonical_payload_sha256: "c".repeat(64),
        observed_at: "2026-07-23T01:02:03Z".into(),
        identity_records: vec!["fixture:identity".into()],
        configuration_records: vec!["fixture:configuration".into()],
        coverage: IdentityCoverage {
            covered: CoreIdentityGroup::REQUIRED.to_vec(),
            missing: Vec::new(),
            trusted_absent: vec![CoreIdentityGroup::Gpu],
        },
        warnings: Vec::new(),
    }
}

async fn open_temp() -> (TempDir, InventoryStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = InventoryStore::open(temp.path()).await.unwrap();
    (temp, store)
}

fn db(path: &Path) -> Connection {
    let connection = Connection::open(path.join("qurbrix_hwinfo.db")).unwrap();
    connection.pragma_update(None, "foreign_keys", "ON").unwrap();
    connection
}

#[tokio::test]
async fn migration_is_idempotent_and_has_required_schema() {
    let (temp, store) = open_temp().await;
    InventoryStore::open(temp.path()).await.unwrap();
    let connection = db(temp.path());
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    let migration_count: i64 = connection
        .query_row("SELECT count(*) FROM schema_migration", [], |row| row.get(0))
        .unwrap();
    assert_eq!(migration_count, 1);

    for table in [
        "inventory_state",
        "hardware_snapshot",
        "snapshot_device",
        "snapshot_device_identifier",
        "snapshot_device_property",
        "snapshot_device_relation",
        "snapshot_warning",
        "snapshot_source",
        "probe_history",
        "snapshot_artifact",
        "snapshot_lifecycle",
        "scan_lease",
    ] {
        let exists: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing table {table}");
    }
    assert!(store.database_path().exists());
}

#[tokio::test]
async fn rejects_future_schema_without_rebuilding() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("qurbrix_hwinfo.db");
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
    drop(connection);
    let error = InventoryStore::open(temp.path()).await.unwrap_err();
    assert!(matches!(error, InventoryError::UnsupportedSchema(2)));
    assert!(path.exists());
}

#[tokio::test]
async fn publishes_projection_and_verified_artifact_atomically() {
    let (temp, store) = open_temp().await;
    let expected_report = report();
    let id = store
        .publish_snapshot(expected_report.clone(), probe())
        .await
        .unwrap();
    assert_eq!(store.load_scan_report(id).await.unwrap(), Some(expected_report));
    let stored = store.load_snapshot(id).await.unwrap().unwrap();
    assert_eq!(stored.snapshot_id, id);
    assert_eq!(stored.device_count, 1);
    assert_eq!(stored.artifact.sha256.len(), 64);
    assert!(temp.path().join(&stored.artifact.relative_path).is_file());

    let devices = store
        .list_devices(
            id,
            PageRequest {
                limit: 1,
                offset: 0,
            },
        )
        .await
        .unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_id, "system:fixture");
    assert!(store
        .list_devices(
            id,
            PageRequest {
                limit: 1,
                offset: 1,
            },
        )
        .await
        .unwrap()
        .is_empty());
    let upload = store
        .upload_projection(id, PageRequest::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(upload.schema_version, SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(upload.devices, devices);

    let connection = db(temp.path());
    let properties: i64 = connection
        .query_row(
            "SELECT count(*) FROM snapshot_device_property WHERE snapshot_id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(properties >= 3);
    let json_columns: i64 = connection
        .query_row(
            "SELECT count(*) FROM pragma_table_info('snapshot_device') WHERE lower(name) LIKE '%json%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(json_columns, 0);
}

#[tokio::test]
async fn transaction_failure_preserves_current_and_removes_new_artifact() {
    let (temp, store) = open_temp().await;
    let first = store.publish_snapshot(report(), probe()).await.unwrap();
    let mut invalid = report();
    invalid.devices.push(invalid.devices[0].clone());
    assert!(store.publish_snapshot(invalid, probe()).await.is_err());

    let connection = db(temp.path());
    let current: String = connection
        .query_row(
            "SELECT current_snapshot_id FROM inventory_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current, first.to_string());
    let snapshots: i64 = connection
        .query_row("SELECT count(*) FROM hardware_snapshot", [], |row| row.get(0))
        .unwrap();
    assert_eq!(snapshots, 1);
    let reports = fs::read_dir(temp.path().join("reports"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|value| value == "json"))
        .count();
    assert_eq!(reports, 1);
}

#[tokio::test]
async fn foreign_keys_indexes_and_immutability_are_enforced() {
    let (temp, store) = open_temp().await;
    let id = store.publish_snapshot(report(), probe()).await.unwrap();
    let connection = db(temp.path());
    let orphan = connection.execute(
        "INSERT INTO snapshot_device(snapshot_id, device_id, kind, name, ordinal) VALUES (?1, 'orphan', 'system', 'orphan', 0)",
        [SnapshotId::new_v7().to_string()],
    );
    assert!(orphan.is_err());
    assert!(connection
        .execute(
            "UPDATE hardware_snapshot SET device_count = 9 WHERE snapshot_id = ?1",
            [id.to_string()],
        )
        .is_err());
    for index in [
        "idx_snapshot_machine_created",
        "idx_device_snapshot_kind",
        "idx_device_property_lookup",
        "idx_probe_started",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing index {index}");
    }
}

#[tokio::test]
async fn detects_same_size_artifact_tampering_and_missing_files() {
    let (temp, store) = open_temp().await;
    let first = store.publish_snapshot(report(), probe()).await.unwrap();
    let first_path = temp
        .path()
        .join(store.load_snapshot(first).await.unwrap().unwrap().artifact.relative_path);
    let mut bytes = fs::read(&first_path).unwrap();
    bytes[0] ^= 1;
    fs::write(&first_path, bytes).unwrap();
    assert!(matches!(
        store.load_scan_report(first).await.unwrap_err(),
        InventoryError::ArtifactHashMismatch
    ));

    let second = store.publish_snapshot(report(), probe()).await.unwrap();
    let second_path = temp
        .path()
        .join(store.load_snapshot(second).await.unwrap().unwrap().artifact.relative_path);
    fs::remove_file(second_path).unwrap();
    assert!(matches!(
        store.load_scan_report(second).await.unwrap_err(),
        InventoryError::Io(_)
    ));
}

#[tokio::test]
async fn startup_recovers_temp_and_renamed_orphan_artifacts() {
    let (temp, store) = open_temp().await;
    let reports = temp.path().join("reports");
    fs::write(reports.join(".crashed.snapshot.tmp"), b"partial").unwrap();
    fs::write(reports.join("01900000-0000-7000-8000-000000000000.json"), b"{}").unwrap();
    fs::create_dir(reports.join("ignored-directory.json")).unwrap();
    assert_eq!(store.recover_orphan_artifacts().await.unwrap(), 2);
    assert!(reports.join("ignored-directory.json").is_dir());
}

#[cfg(unix)]
#[tokio::test]
async fn state_permissions_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let (temp, store) = open_temp().await;
    store.publish_snapshot(report(), probe()).await.unwrap();
    assert_eq!(fs::metadata(temp.path()).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(store.database_path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let report_path = fs::read_dir(temp.path().join("reports"))
        .unwrap()
        .find_map(Result::ok)
        .unwrap()
        .path();
    assert_eq!(fs::metadata(report_path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[tokio::test]
async fn snapshot_listing_filters_by_machine_and_pages() {
    let (_temp, store) = open_temp().await;
    let first = store.publish_snapshot(report(), probe()).await.unwrap();
    let mut other_probe = probe();
    other_probe.machine_bind_id = "d".repeat(64);
    let second = store.publish_snapshot(report(), other_probe).await.unwrap();
    let filtered = store
        .list_snapshots(
            Some("a".repeat(64)),
            PageRequest {
                limit: 10,
                offset: 0,
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].snapshot_id, first);
    let all = store.list_snapshots(None, PageRequest::default()).await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|snapshot| snapshot.snapshot_id == second));
}
