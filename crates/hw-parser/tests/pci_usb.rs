use hw_parser::{
    parse_dmidecode_bios_board, parse_hdparm_identify, parse_hwinfo_disk, parse_lshw_disk,
    parse_lshw_storage, parse_lspci_host_bridge_chipset, parse_lspci_nn_k, parse_lspci_vmm_nn_k,
    parse_lsusb, parse_lsusb_verbose, parse_smartctl_json,
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
fn parses_machine_readable_lspci_vendor_and_device_separately() {
    let records = parse_lspci_vmm_nn_k(
        "Slot:\t0000:00:10.0\n\
         Class:\tSCSI storage controller [0100]\n\
         Vendor:\tBroadcom / LSI [1000]\n\
         Device:\t53c1030 PCI-X Fusion-MPT Dual Ultra320 SCSI [0030]\n\
         Driver:\tmptspi\n\
         Module:\tmptspi\n\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].address, "0000:00:10.0");
    assert_eq!(records[0].vendor.as_deref(), Some("Broadcom / LSI"));
    assert_eq!(records[0].vendor_id.as_deref(), Some("1000"));
    assert_eq!(
        records[0].device.as_deref(),
        Some("53c1030 PCI-X Fusion-MPT Dual Ultra320 SCSI")
    );
    assert_eq!(records[0].device_id.as_deref(), Some("0030"));
    assert_eq!(records[0].kernel_driver.as_deref(), Some("mptspi"));
}

#[test]
fn parses_deepin_bios_board_detail_fields() {
    let record = parse_dmidecode_bios_board(
        "# dmidecode 3.5\n\
         SMBIOS 3.4.0 present.\n\
         BIOS Information\n\
         \tVendor: LENOVO\n\
         \tVersion: N2IET98W\n\
         \tRelease Date: 01/01/2026\n\
         \tAddress: 0xE0000\n\
         \tRuntime Size: 128 kB\n\
         \tROM Size: 16 MB\n\
         \tCharacteristics:\n\
         \t\tPCI is supported\n\
         \t\tBIOS is upgradeable\n\
         \tBIOS Revision: 1.23\n\
         \tFirmware Revision: 4.56\n\
         Base Board Information\n\
         \tManufacturer: LENOVO\n\
         \tProduct Name: 20XX\n\
         \tFeatures:\n\
         \t\tBoard is a hosting board\n\
         \t\tBoard is replaceable\n\
         \tType: Motherboard\n",
    );

    assert_eq!(record.smbios_version.as_deref(), Some("3.4.0"));
    assert_eq!(record.bios_version.as_deref(), Some("N2IET98W"));
    assert_eq!(record.bios_address.as_deref(), Some("0xE0000"));
    assert_eq!(record.bios_runtime_size.as_deref(), Some("128 kB"));
    assert_eq!(record.bios_rom_size.as_deref(), Some("16 MB"));
    assert_eq!(
        record.bios_characteristics,
        ["PCI is supported", "BIOS is upgradeable"]
    );
    assert_eq!(record.bios_revision.as_deref(), Some("1.23"));
    assert_eq!(record.firmware_revision.as_deref(), Some("4.56"));
    assert_eq!(
        record.board_features,
        ["Board is a hosting board", "Board is replaceable"]
    );
    assert_eq!(record.board_type.as_deref(), Some("Motherboard"));
}

#[test]
fn bios_version_not_overwritten_by_system_information_version() {
    let record = parse_dmidecode_bios_board(
        "# dmidecode 3.6\n\
         SMBIOS 2.7 present.\n\
         \n\
         Handle 0x0000, DMI type 0, 26 bytes\n\
         BIOS Information\n\
         \tVendor: VMware, Inc.\n\
         \tVersion: VMW201.00V.21805430.B64.2305221830\n\
         \tRelease Date: 05/22/2023\n\
         \tROM Size: 2 MB\n\
         \n\
         Handle 0x0001, DMI type 1, 27 bytes\n\
         System Information\n\
         \tManufacturer: VMware, Inc.\n\
         \tProduct Name: VMware20,1\n\
         \tVersion: None\n\
         \tSerial Number: VMware-56 4d 92\n\
         \n\
         Handle 0x0002, DMI type 2, 15 bytes\n\
         Base Board Information\n\
         \tManufacturer: Intel Corporation\n\
         \tProduct Name: 440BX Desktop Reference Platform\n\
         \tVersion: None\n",
    );

    assert_eq!(
        record.bios_version.as_deref(),
        Some("VMW201.00V.21805430.B64.2305221830")
    );
    assert_eq!(record.bios_vendor.as_deref(), Some("VMware, Inc."));
    assert_eq!(record.bios_release_date.as_deref(), Some("05/22/2023"));
    assert_eq!(
        record.board_product_name.as_deref(),
        Some("440BX Desktop Reference Platform")
    );
    assert_eq!(record.board_version, None);
}

#[test]
fn memory_array_fields_not_overwritten_by_memory_device_keys() {
    let record = parse_dmidecode_bios_board(
        "# dmidecode 3.6\n\
         Handle 0x0028, DMI type 16, 23 bytes\n\
         Physical Memory Array\n\
         \tLocation: System Board Or Motherboard\n\
         \tUse: System Memory\n\
         \tError Correction Type: None\n\
         \tMaximum Capacity: 17 GB\n\
         \tError Information Handle: Not Provided\n\
         \tNumber Of Devices: 64\n\
         \n\
         Handle 0x0029, DMI type 17, 40 bytes\n\
         Memory Device\n\
         \tArray Handle: 0x0028\n\
         \tError Information Handle: No Error\n\
         \tTotal Width: 64 bits\n",
    );

    assert_eq!(
        record.memory_array_error_information_handle.as_deref(),
        Some("Not Provided")
    );
    assert_eq!(
        record.memory_array_location.as_deref(),
        Some("System Board Or Motherboard")
    );
    assert_eq!(record.memory_array_number_of_devices.as_deref(), Some("64"));
}

#[test]
fn parses_host_bridge_chipset_family_from_lspci() {
    let chipset = parse_lspci_host_bridge_chipset(
        "00:00.0 Host bridge [0600]: Intel Corporation Device [8086:9a14] (rev 01)\n\
         00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n",
    );

    assert_eq!(
        chipset.as_deref(),
        Some("Intel Corporation Device [8086:9a14]")
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
fn parses_lsusb_verbose_device_descriptor_fields() {
    let records = parse_lsusb_verbose(
        "Bus 002 Device 003: ID 0e0f:0003 VMware, Inc. Virtual Mouse\n\
         Device Descriptor:\n\
           iManufacturer           1 VMware\n\
           iProduct                2 VMware Virtual USB Mouse\n\
           iSerial                 3 VMOUSE123\n\
           Negotiated speed: Full Speed (12Mbps)\n\
         Configuration Descriptor:\n\
           MaxPower                100mA\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].manufacturer.as_deref(), Some("VMware"));
    assert_eq!(
        records[0].product.as_deref(),
        Some("VMware Virtual USB Mouse")
    );
    assert_eq!(records[0].serial.as_deref(), Some("VMOUSE123"));
    assert_eq!(records[0].speed.as_deref(), Some("12"));
    assert_eq!(records[0].max_power_ma, Some(100));
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
fn parses_lshw_storage_controller_records() {
    let records = parse_lshw_storage(
        "  *-storage\n\
              description: Non-Volatile memory controller\n\
              product: NVMe SSD Controller PM9A1/PM9A3/980PRO\n\
              vendor: Samsung Electronics Co Ltd\n\
              bus info: pci@0000:0d:00.0\n\
              configuration: driver=nvme latency=0\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].bus_info.as_deref(), Some("pci@0000:0d:00.0"));
    assert_eq!(
        records[0].product.as_deref(),
        Some("NVMe SSD Controller PM9A1/PM9A3/980PRO")
    );
    assert_eq!(
        records[0].vendor.as_deref(),
        Some("Samsung Electronics Co Ltd")
    );
    assert_eq!(records[0].driver.as_deref(), Some("nvme"));
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
fn parses_hdparm_identify_fields() {
    let record = parse_hdparm_identify(
        "/dev/sda:\n\
         \n\
         Model=Samsung SSD 870 EVO 500GB, FwRev=SVT02B6Q, SerialNo=S6P012345678\n",
    );

    assert_eq!(record.model.as_deref(), Some("Samsung SSD 870 EVO 500GB"));
    assert_eq!(record.firmware.as_deref(), Some("SVT02B6Q"));
    assert_eq!(record.serial.as_deref(), Some("S6P012345678"));
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
