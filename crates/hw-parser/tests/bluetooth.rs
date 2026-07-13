use hw_parser::parse_hciconfig;
use hw_testdata::fixture;

#[test]
fn hciconfig_extended_fields_are_populated() {
    let input = fixture("bluetooth/hciconfig-a.txt");
    let records = parse_hciconfig(&input);

    let record = records.first().expect("one controller parsed");
    assert_eq!(
        record.hci_version.as_deref(),
        Some("5.3 (0xc)  Revision: 0x1234")
    );
    assert_eq!(
        record.lmp_version.as_deref(),
        Some("5.3 (0xc)  Subversion: 0x100")
    );
    assert_eq!(record.manufacturer.as_deref(), Some("Intel Corp. (2)"));
    assert_eq!(record.device_class.as_deref(), Some("0x7c010c"));
    assert_eq!(
        record.features,
        vec!["0xff", "0xff", "0xff", "0xfe", "0xdb", "0xff", "0x7b", "0x87",]
    );
    assert!(record.flags.contains(&"UP".to_string()));
    assert!(record.flags.contains(&"RUNNING".to_string()));
}
