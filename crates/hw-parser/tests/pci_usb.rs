use hw_parser::{
    parse_hwinfo_disk, parse_lshw_disk, parse_lspci_nn_k, parse_lsusb, parse_lsusb_verbose,
    parse_smartctl_json,
};

#[test]
fn parses_lspci_driver_and_modules() {
    let input = hw_testdata::fixture("pci/lspci-nn-k.txt");
    let records = parse_lspci_nn_k(&input);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].address, "0000:00:1f.3");
    assert_eq!(records[0].class_name.as_deref(), Some("Audio device"));
    assert_eq!(records[0].class_id.as_deref(), Some("0403"));
    assert_eq!(records[0].vendor, None);
    assert_eq!(
        records[0].device.as_deref(),
        Some("Intel Corporation Cannon Lake PCH cAVS")
    );
    assert_eq!(records[0].vendor_id.as_deref(), Some("8086"));
    assert_eq!(records[0].device_id.as_deref(), Some("a348"));
    assert_eq!(records[0].kernel_driver.as_deref(), Some("snd_hda_intel"));
    assert_eq!(
        records[0].kernel_modules,
        vec!["snd_hda_intel", "snd_soc_avs"]
    );
}

#[test]
fn parses_lsusb_basic_records() {
    let input = hw_testdata::fixture("usb/lsusb.txt");
    let records = parse_lsusb(&input);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].bus.as_deref(), Some("001"));
    assert_eq!(records[0].device.as_deref(), Some("004"));
    assert_eq!(records[0].vendor_id.as_deref(), Some("0bda"));
    assert_eq!(records[0].product_id.as_deref(), Some("5689"));
    assert!(records[0]
        .product
        .as_deref()
        .unwrap()
        .contains("Integrated Camera"));
}

#[test]
fn parses_lsusb_verbose_interface_descriptors() {
    let records = parse_lsusb_verbose(
        "Bus 001 Device 004: ID 0bda:5689 Realtek Semiconductor Corp. Integrated Camera\n\
         Interface Descriptor:\n\
           bInterfaceNumber        0\n\
           bInterfaceClass        14 Video\n\
           bInterfaceSubClass      2 Video Streaming\n\
           bInterfaceProtocol      0\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].interface.as_deref(), Some("0"));
    assert_eq!(records[0].class.as_deref(), Some("0e"));
    assert_eq!(records[0].subclass.as_deref(), Some("02"));
    assert_eq!(records[0].protocol.as_deref(), Some("00"));
}

#[test]
fn parses_lshw_disk_records() {
    let records = parse_lshw_disk(
        "  *-disk\n\
              description: ATA Disk\n\
              product: Samsung SSD 980\n\
              vendor: Samsung\n\
              logical name: /dev/sda\n\
              serial: S12345\n\
              size: 953GiB\n\
              configuration: ansiversion=5 firmware=3B2QGXA7 sectorsize=512\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_name.as_deref(), Some("/dev/sda"));
    assert_eq!(records[0].product.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(records[0].vendor.as_deref(), Some("Samsung"));
    assert_eq!(records[0].serial.as_deref(), Some("S12345"));
    assert_eq!(records[0].firmware.as_deref(), Some("3B2QGXA7"));
}

#[test]
fn lshw_disk_parser_ignores_child_volume_sections() {
    let records = parse_lshw_disk(
        "  *-disk\n\
              product: Samsung SSD 980\n\
              logical name: /dev/sda\n\
              serial: S12345\n\
              configuration: firmware=3B2QGXA7\n\
           *-volume:0\n\
              description: EXT4 volume\n\
              logical name: /dev/sda1\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_name.as_deref(), Some("/dev/sda"));
    assert_eq!(records[0].product.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(records[0].serial.as_deref(), Some("S12345"));
    assert_eq!(records[0].firmware.as_deref(), Some("3B2QGXA7"));
}

#[test]
fn parses_hwinfo_disk_records() {
    let records = parse_hwinfo_disk(
        "30: IDE 00.0: 10600 Disk\n\
             Hardware Class: disk\n\
             Model: \"Samsung SSD 980\"\n\
             Vendor: \"Samsung\"\n\
             Device: \"SSD 980\"\n\
             Revision: \"3B2QGXA7\"\n\
             Driver: \"nvme\"\n\
             Driver Modules: \"nvme\"\n\
             Device File: /dev/nvme0n1\n\
             Serial ID: \"S12345\"\n\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].device_node.as_deref(), Some("/dev/nvme0n1"));
    assert_eq!(records[0].model.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(records[0].vendor.as_deref(), Some("Samsung"));
    assert_eq!(records[0].device.as_deref(), Some("SSD 980"));
    assert_eq!(records[0].revision.as_deref(), Some("3B2QGXA7"));
    assert_eq!(records[0].driver.as_deref(), Some("nvme"));
    assert_eq!(records[0].driver_modules, vec!["nvme"]);
    assert_eq!(records[0].serial.as_deref(), Some("S12345"));
}

#[test]
fn parsers_ignore_empty_and_malformed_lines() {
    assert!(parse_lspci_nn_k("").is_empty());
    assert!(parse_lspci_nn_k("not pci\n\tKernel driver in use: nope").is_empty());
    assert!(parse_lsusb("").is_empty());
    assert!(parse_lsusb("Bus xxx Device: malformed").is_empty());
}

#[test]
fn parses_smartctl_health_and_temperature() {
    let info = parse_smartctl_json(
        r#"{
          "smart_status": {"passed": true},
          "temperature": {"current": 37}
        }"#,
    )
    .expect("expected smartctl JSON to parse");

    assert_eq!(info.smart_status.as_deref(), Some("passed"));
    assert_eq!(info.temperature_celsius, Some(37.0));
}

#[test]
fn parses_nvme_smartctl_health_details() {
    let info = parse_smartctl_json(
        r#"{
          "smart_status": {"passed": true},
          "temperature": {"current": 37},
          "power_on_time": {"hours": 1234},
          "power_cycle_count": 56,
          "nvme_smart_health_information_log": {
            "available_spare": 99,
            "available_spare_threshold": 10,
            "percentage_used": 3,
            "data_units_read": 123456,
            "data_units_written": 654321,
            "media_errors": 2,
            "num_err_log_entries": 4
          }
        }"#,
    )
    .expect("expected smartctl JSON to parse");

    assert_eq!(info.power_on_hours, Some(1234));
    assert_eq!(info.power_cycle_count, Some(56));
    assert_eq!(info.available_spare_percent, Some(99));
    assert_eq!(info.available_spare_threshold_percent, Some(10));
    assert_eq!(info.percentage_used, Some(3));
    assert_eq!(info.data_units_read, Some(123456));
    assert_eq!(info.data_units_written, Some(654321));
    assert_eq!(info.media_errors, Some(2));
    assert_eq!(info.error_log_entries, Some(4));
}
