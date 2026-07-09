use hw_parser::{parse_hwinfo_disk, parse_lsblk_json_result, parse_lshw_disk, parse_smartctl_json};

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

#[test]
fn storage_fixtures_cover_lsblk_smartctl_hwinfo_and_lshw_sources() {
    let lsblk = parse_lsblk_json_result(&hw_testdata::fixture("storage/lsblk.json"))
        .expect("lsblk fixture parses");
    assert_eq!(lsblk.len(), 1);
    assert_eq!(lsblk[0].tran.as_deref(), Some("nvme"));

    let smart = parse_smartctl_json(&hw_testdata::fixture("storage/smartctl-nvme.json"))
        .expect("smartctl fixture parses");
    assert_eq!(smart.smart_status.as_deref(), Some("passed"));
    assert_eq!(smart.temperature_celsius, Some(37.0));

    let hwinfo = parse_hwinfo_disk(&hw_testdata::fixture("storage/hwinfo-disk.txt"));
    assert_eq!(hwinfo.len(), 1);
    assert_eq!(hwinfo[0].device_node.as_deref(), Some("/dev/nvme0n1"));

    let lshw = parse_lshw_disk(&hw_testdata::fixture("storage/lshw-disk.txt"));
    assert_eq!(lshw.len(), 1);
    assert_eq!(lshw[0].logical_name.as_deref(), Some("/dev/nvme0n1"));
}
