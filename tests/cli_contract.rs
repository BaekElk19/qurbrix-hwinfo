use std::process::Command;

use qurbrix_hw::{
    CoreIdentityGroup, Device, DeviceKind, DeviceProperties, IdentityCoverage, InventoryStore,
    QuickProbeReport, ScanReport, SystemDeviceInfo, BINDID_V2_ALGORITHM, FINGERPRINT_VERSION,
    SNAPSHOT_SCHEMA_VERSION,
};

fn qurbrix_hw() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qurbrix-hw"))
}

fn seed_snapshots(path: &std::path::Path) -> (qurbrix_hw::SnapshotId, qurbrix_hw::SnapshotId) {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = InventoryStore::open(path).await.unwrap();
        let mut first_report = ScanReport::empty();
        first_report.devices.push(Device::new(
            "system:fixture",
            DeviceKind::System,
            "Fixture",
            DeviceProperties::System(SystemDeviceInfo {
                uuid: Some("fixture-uuid".into()),
                ..SystemDeviceInfo::default()
            }),
        ));
        let probe = |fingerprint: char| QuickProbeReport {
            schema_version: SNAPSHOT_SCHEMA_VERSION.into(),
            fingerprint_version: FINGERPRINT_VERSION,
            bindid_algorithm: BINDID_V2_ALGORITHM.into(),
            machine_bind_id: "a".repeat(64),
            configuration_fingerprint: fingerprint.to_string().repeat(64),
            canonical_payload_sha256: fingerprint.to_string().repeat(64),
            observed_at: "2026-07-23T00:00:00Z".into(),
            identity_records: vec!["fixture".into()],
            configuration_records: vec!["fixture".into()],
            coverage: IdentityCoverage {
                covered: CoreIdentityGroup::REQUIRED.to_vec(),
                missing: Vec::new(),
                trusted_absent: Vec::new(),
            },
            warnings: Vec::new(),
        };
        let first = store
            .publish_snapshot(first_report.clone(), probe('b'))
            .await
            .unwrap();
        let mut second_report = first_report;
        second_report.devices[0].name = "Changed".into();
        let second = store
            .publish_snapshot(second_report, probe('c'))
            .await
            .unwrap();
        (first, second)
    })
}

#[test]
fn schema_command_writes_stable_json_to_stdout_only() {
    let output = qurbrix_hw().arg("schema").output().expect("run schema");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr must be reserved for diagnostics"
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("schema stdout should be JSON");
    assert_eq!(
        value.get("schema_version").and_then(|value| value.as_str()),
        Some("qurbrix.hw.scan.v2")
    );
}

#[test]
fn list_kinds_json_is_machine_readable_stdout_only() {
    let output = qurbrix_hw()
        .args(["list-kinds", "--format", "json"])
        .output()
        .expect("run list-kinds");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr must be reserved for diagnostics"
    );

    let kinds: Vec<String> =
        serde_json::from_slice(&output.stdout).expect("list-kinds stdout should be JSON");
    assert!(kinds.contains(&"cpu".to_string()));
    assert!(kinds.contains(&"storage".to_string()));
    assert!(kinds.contains(&"other-device".to_string()));
}

#[test]
fn snapshot_show_list_diff_and_export_have_stable_json_contracts() {
    let state = tempfile::tempdir().unwrap();
    let (first, second) = seed_snapshots(state.path());
    let state_path = state.path().to_str().unwrap();

    let show = qurbrix_hw()
        .args([
            "snapshot",
            "show",
            &second.to_string(),
            "--state-dir",
            state_path,
        ])
        .output()
        .unwrap();
    assert!(show.status.success());
    assert!(show.stderr.is_empty());
    let show_json: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(show_json["schema_version"], "qurbrix.hw.snapshot.cli.v1");
    assert_eq!(show_json["snapshot"]["snapshot_id"], second.to_string());

    let list = qurbrix_hw()
        .args([
            "snapshot",
            "list",
            "--state-dir",
            state_path,
            "--limit",
            "1",
            "--offset",
            "1",
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    assert!(list.stderr.is_empty());
    let list_json: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(list_json["limit"], 1);
    assert_eq!(list_json["offset"], 1);
    assert_eq!(list_json["snapshots"].as_array().unwrap().len(), 1);

    let diff = qurbrix_hw()
        .args([
            "snapshot",
            "diff",
            &first.to_string(),
            &second.to_string(),
            "--state-dir",
            state_path,
        ])
        .output()
        .unwrap();
    assert!(diff.status.success());
    assert!(diff.stderr.is_empty());
    let diff_json: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(diff_json["changed"][0]["device_id"], "system:fixture");

    let export_path = state.path().join("exported.json");
    let export = qurbrix_hw()
        .args([
            "snapshot",
            "export",
            &second.to_string(),
            "--state-dir",
            state_path,
            "--output",
            export_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(export.status.success());
    assert!(export.stderr.is_empty());
    let export_json: serde_json::Value = serde_json::from_slice(&export.stdout).unwrap();
    assert_eq!(export_json["snapshot_id"], second.to_string());
    let exported: ScanReport =
        serde_json::from_slice(&std::fs::read(export_path).unwrap()).unwrap();
    assert_eq!(exported.devices[0].name, "Changed");
}

#[test]
fn snapshot_not_found_has_empty_stdout_stable_stderr_and_exit_five() {
    let state = tempfile::tempdir().unwrap();
    let missing = qurbrix_hw::SnapshotId::new_v7();
    let output = qurbrix_hw()
        .args([
            "snapshot",
            "show",
            &missing.to_string(),
            "--state-dir",
            state.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("snapshot error [inventory.snapshot_not_found]:"));
}
