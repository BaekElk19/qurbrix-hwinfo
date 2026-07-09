use hw_parser::parse_lshw_disk;

#[test]
fn lshw_disk_parses_deepin_storage_configuration_fields() {
    let records = parse_lshw_disk(
        "  *-disk\n\
             description: ATA Disk\n\
             product: Samsung SSD 870 EVO\n\
             vendor: Samsung\n\
             logical name: /dev/sda\n\
             serial: S12345\n\
             capabilities: gpt-1.00 partitioned partitioned:gpt\n\
             configuration: ansiversion=5 firmware=SVT02B6Q sectorsize=512 speed=6Gbit/s\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].firmware.as_deref(), Some("SVT02B6Q"));
    assert_eq!(records[0].speed.as_deref(), Some("6Gbit/s"));
    assert_eq!(
        records[0].capabilities,
        vec!["gpt-1.00", "partitioned", "partitioned:gpt"]
    );
}
