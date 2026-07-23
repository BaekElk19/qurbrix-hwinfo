#[test]
fn facade_exports_schema_version() {
    assert_eq!(qurbrix_hw::schema_version(), "qurbrix.hw.scan.v2");
}

#[test]
fn facade_exports_snapshot_contract_types() {
    let id = qurbrix_hw::SnapshotId::new_v7();
    let options = qurbrix_hw::EnsureSnapshotOptions::default();
    let page = qurbrix_hw::PageRequest::default();
    assert_eq!(id.as_uuid().get_version_num(), 7);
    assert_eq!(options.max_snapshot_age.unwrap().as_secs(), 86_400);
    assert_eq!(page.limit, 100);
}
