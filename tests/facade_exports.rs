#[test]
fn facade_exports_schema_version() {
    assert_eq!(qurbrix_hw::schema_version(), "qurbrix.hw.scan.v2");
}
