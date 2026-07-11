use hw_bindid::{collect_bindid_report_with_runner, BindIdStatus};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn collector_returns_failed_report_when_required_sources_are_missing() {
    let runner = FakeSourceRunner::new();
    let report = collect_bindid_report_with_runner(&runner, Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(report.status, BindIdStatus::Failed);
    assert!(report.value.is_none());
    assert!(report.missing_required_kinds.contains(&"system".to_string()));
    assert!(report.missing_required_kinds.contains(&"motherboard".to_string()));
    assert!(report.missing_required_kinds.contains(&"memory".to_string()));
    assert!(report.missing_required_kinds.contains(&"storage".to_string()));
    assert!(report.missing_required_kinds.contains(&"network".to_string()));
    assert!(!report.warnings.is_empty());
}

#[tokio::test]
async fn collector_converts_narrow_probe_devices_into_component_keys() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
    );
    let report = collect_bindid_report_with_runner(&runner, Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(report.status, BindIdStatus::Failed);
    assert!(report.value.is_none());
    assert_eq!(
        report.component_keys,
        vec!["storage:model=Disk|serial=S1".to_string()]
    );
    assert_eq!(report.covered_kinds, vec!["storage".to_string()]);
    assert!(!report.missing_required_kinds.contains(&"storage".to_string()));
    assert!(!report.warnings.is_empty());
}
