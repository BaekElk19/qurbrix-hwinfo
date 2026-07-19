use hw_parser::*;

#[test]
fn parses_input_devices() {
    let records = parse_proc_bus_input_devices(&hw_testdata::fixture("proc/bus-input-devices.txt"));
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].name.as_deref(),
        Some("AT Translated Set 2 keyboard")
    );
    assert_eq!(records[0].handlers, vec!["sysrq", "kbd", "event0", "leds"]);
    assert_eq!(records[1].vendor_id.as_deref(), Some("046d"));
}

#[test]
fn parses_hwinfo_input_devices() {
    let records = parse_hwinfo_input(
        "18: USB 00.0: 10800 Keyboard\n\
         \tHardware Class: keyboard\n\
         \tModel: \"Lite-On USB Keyboard\"\n\
         \tVendor: usb 0x04ca \"Lite-On Technology Corp.\"\n\
         \tDevice: usb 0x00a1 \"USB Keyboard\"\n\
         \tDriver: \"usbhid\"\n\
         \tDriver Modules: \"usbhid\"\n\
         \tDevice Files: /dev/input/event0, /dev/input/by-id/usb-keyboard-event-kbd\n\
         \n\
         19: USB 00.1: 10503 Mouse\n\
         \tHardware Class: mouse\n\
         \tModel: \"Logitech USB Optical Mouse\"\n\
         \tVendor: usb 0x046d \"Logitech, Inc.\"\n\
         \tDevice File: /dev/input/event5\n\
         \n\
         20: SCSI 0.0: 10600 Disk\n\
         \tHardware Class: disk\n\
         \tDevice File: /dev/sda\n",
    );

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].input_kind, HwinfoInputKind::Keyboard);
    assert_eq!(records[0].event_node.as_deref(), Some("/dev/input/event0"));
    assert_eq!(records[0].model.as_deref(), Some("Lite-On USB Keyboard"));
    assert_eq!(
        records[0].vendor.as_deref(),
        Some("Lite-On Technology Corp.")
    );
    assert_eq!(records[0].device.as_deref(), Some("USB Keyboard"));
    assert_eq!(records[0].driver.as_deref(), Some("usbhid"));
    assert_eq!(records[0].driver_modules, vec!["usbhid"]);
    assert_eq!(records[1].input_kind, HwinfoInputKind::Mouse);
    assert_eq!(records[1].event_node.as_deref(), Some("/dev/input/event5"));
}

#[test]
fn parses_asound_cards() {
    let cards = parse_proc_asound_cards(&hw_testdata::fixture("proc/asound-cards.txt"));
    assert_eq!(cards[0].index, 0);
    assert_eq!(cards[0].id.as_deref(), Some("PCH"));
    assert!(cards[0].name.as_deref().unwrap().contains("HDA Intel PCH"));
}

#[test]
fn parses_lshw_multimedia_audio_devices() {
    let records = parse_lshw_multimedia(
        "  *-multimedia\n\
              description: Audio device\n\
              product: Alder Lake PCH-P High Definition Audio Controller\n\
              vendor: Intel Corporation\n\
              version: 30\n\
              bus info: pci@0000:00:1f.3\n\
              capabilities: pm msi bus_master cap_list\n\
              configuration: driver=snd_hda_intel latency=64 irq=145\n\
              resources: irq:145 memory:a1230000-a1233fff\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].product.as_deref(),
        Some("Alder Lake PCH-P High Definition Audio Controller")
    );
    assert_eq!(records[0].vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(records[0].bus_info.as_deref(), Some("pci@0000:00:1f.3"));
    assert_eq!(records[0].driver.as_deref(), Some("snd_hda_intel"));
    assert_eq!(records[0].version.as_deref(), Some("30"));
    assert_eq!(records[0].irq.as_deref(), Some("145"));
    assert_eq!(records[0].latency.as_deref(), Some("64"));
    assert_eq!(
        records[0].capabilities,
        vec!["pm", "msi", "bus_master", "cap_list"]
    );
    assert_eq!(
        records[0].memory_address.as_deref(),
        Some("a1230000-a1233fff")
    );
}

#[test]
fn parses_hwinfo_sound_devices() {
    let records = parse_hwinfo_sound(
        "12: PCI 1f.3: 0403 Audio device\n\
         \t[Created at pci.386]\n\
         \tUnique ID: nS1_.abc123\n\
         \tHardware Class: sound\n\
         \tModel: \"Intel Cannon Lake PCH cAVS\"\n\
         \tVendor: pci 0x8086 \"Intel Corporation\"\n\
         \tDevice: pci 0xa348 \"Cannon Lake PCH cAVS\"\n\
         \tDriver: \"snd_hda_intel\"\n\
         \tDriver Modules: \"snd_hda_intel\"\n\
         \tRevision: 30\n\
         \tIRQ: 145\n\
         \tMemory Range: 0xa1230000-0xa1233fff (rw,non-prefetchable)\n\
         \tDriver Status: snd_hda_intel is active\n\
         \tSubDevice: pci 0x1234\n\
         \tSubVendor: pci 0x8086 \"Intel Corporation\"\n\
         \tModule Alias: pci:v00008086d0000A348sv00008086sd00001234bc04sc03i00\n\
         \tSysFS BusID: 0000:00:1f.3\n\
         \tSysFS ID: /class/sound/card0\n\
         \n\
         13: PCI 02.0: 0200 Ethernet controller\n\
         \tHardware Class: network\n\
         \tModel: \"Intel Ethernet\"\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].model.as_deref(),
        Some("Intel Cannon Lake PCH cAVS")
    );
    assert_eq!(records[0].vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(records[0].driver.as_deref(), Some("snd_hda_intel"));
    assert_eq!(records[0].driver_modules, vec!["snd_hda_intel"]);
    assert_eq!(records[0].pci_address.as_deref(), Some("0000:00:1f.3"));
    assert_eq!(records[0].card_index, Some(0));
    assert_eq!(records[0].revision.as_deref(), Some("30"));
    assert_eq!(records[0].irq.as_deref(), Some("145"));
    assert_eq!(
        records[0].memory_address.as_deref(),
        Some("0xa1230000-0xa1233fff (rw,non-prefetchable)")
    );
    assert_eq!(
        records[0].driver_status.as_deref(),
        Some("snd_hda_intel is active")
    );
    assert_eq!(records[0].sub_device.as_deref(), Some("pci 0x1234"));
    assert_eq!(records[0].sub_vendor.as_deref(), Some("Intel Corporation"));
    assert!(records[0]
        .modalias
        .as_deref()
        .is_some_and(|value| value.starts_with("pci:v00008086")));
}

#[test]
fn parses_pactl_card_profiles() {
    let records = parse_pactl_card_profiles(
        "Card #0\n\
         \tName: alsa_card.pci-0000_00_1f.3\n\
         \tProperties:\n\
         \t\talsa.card = \"0\"\n\
         \tProfiles:\n\
         \t\toutput:analog-stereo: Analog Stereo Output (sinks: 1, sources: 0, priority: 6500, available: yes)\n\
         \t\toutput:hdmi-stereo: Digital Stereo (HDMI) Output (sinks: 1, sources: 0, priority: 5900, available: no)\n\
         \t\toff: Off (sinks: 0, sources: 0, priority: 0, available: yes)\n\
         \tActive Profile: output:analog-stereo\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].card_index, Some(0));
    assert_eq!(
        records[0].profiles,
        vec!["output:analog-stereo", "output:hdmi-stereo", "off"]
    );
}

#[test]
fn parses_upower_battery() {
    let devices = parse_upower_dump(&hw_testdata::fixture("power/upower-dump.txt"));
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].native_path.as_deref(), Some("BAT0"));
    assert_eq!(devices[0].state.as_deref(), Some("discharging"));
    assert_eq!(devices[0].capacity_percent, Some(88.0702));
}

#[test]
fn parses_lshw_memory_banks() {
    let records = parse_lshw_memory(
        "*-memory\n\
             description: System Memory\n\
           *-bank:0\n\
                description: SODIMM DDR4 Synchronous 3200 MHz (0.3 ns)\n\
                product: M471A2K43CB1-CTD\n\
                vendor: Samsung\n\
                serial: ABCD1234\n\
                slot: ChannelA-DIMM0\n\
                size: 8GiB\n\
                clock: 3200MHz (0.3ns)\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].locator.as_deref(), Some("ChannelA-DIMM0"));
    assert_eq!(records[0].manufacturer.as_deref(), Some("Samsung"));
    assert_eq!(records[0].serial.as_deref(), Some("ABCD1234"));
    assert_eq!(records[0].part_number.as_deref(), Some("M471A2K43CB1-CTD"));
    assert_eq!(records[0].memory_type.as_deref(), Some("DDR4"));
    assert_eq!(
        parse_size_to_bytes(records[0].size.as_deref()),
        Some(8 * 1024 * 1024 * 1024)
    );
    assert_eq!(parse_speed_mtps(records[0].speed.as_deref()), Some(3200));
}

#[test]
fn parses_lshw_system_memory_when_no_banks() {
    let records = parse_lshw_memory(
        "*-memory\n\
             description: System Memory\n\
             physical id: 10\n\
             size: 32GiB\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        parse_size_to_bytes(records[0].size.as_deref()),
        Some(32 * 1024 * 1024 * 1024)
    );
}

#[test]
fn parses_dmidecode_memory_manufacturer_id_fallback() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 16 GB\n\
         \tLocator: ChannelA-DIMM0\n\
         \tManufacturer: Not Specified\n\
         \tManufacturer ID: 80ce00000000\n\
         \tSerial Number: ABCD1234\n\
         \tType: DDR5\n\
         \tSpeed: 4800 MT/s\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].manufacturer.as_deref(), Some("80CE00000000"));
}

#[test]
fn parses_dmidecode_memory_bank_locator_fallback() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 16 GB\n\
         \tLocator: Not Specified\n\
         \tBank Locator: BANK 0\n\
         \tManufacturer: Samsung\n\
         \tType: DDR4\n\
         \tSpeed: 3200 MT/s\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].locator.as_deref(), Some("BANK 0"));
}

#[test]
fn parses_dmidecode_memory_configured_speed_fallback() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 16 GB\n\
         \tLocator: DIMM 0\n\
         \tManufacturer: Samsung\n\
         \tType: DDR5\n\
         \tSpeed: Unknown\n\
         \tConfigured Memory Speed: 5600 MT/s\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].speed.as_deref(), Some("5600 MT/s"));
    assert_eq!(parse_speed_mtps(records[0].speed.as_deref()), Some(5600));
}

#[test]
fn parses_dmidecode_memory_ignores_out_of_spec_type() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 16 GB\n\
         \tLocator: DIMM 0\n\
         \tManufacturer: Samsung\n\
         \tType: <OUT OF SPEC>\n\
         \tSpeed: 3200 MT/s\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].memory_type, None);
}

#[test]
fn parses_dmidecode_memory_ignores_ft1500a_random_size() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 12345678901234567890\n\
         \tLocator: DIMM0\n\
         \tManufacturer ID: 80ce00000000\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].size, None);
    assert_eq!(records[0].locator.as_deref(), Some("DIMM0"));
    assert_eq!(records[0].manufacturer.as_deref(), Some("80CE00000000"));
}

#[test]
fn parses_dmidecode_memory_drops_records_with_only_invalid_size() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 12345678901234567890\n",
    );

    assert!(records.is_empty());
}

#[test]
fn parses_dmidecode_memory_deepin_detail_fields() {
    let records = parse_dmidecode_memory(
        "Memory Device\n\
         \tSize: 32 GB\n\
         \tError Information Handle: 0x0042\n\
         \tForm Factor: DIMM\n\
         \tSet: None\n\
         \tLocator: ChannelA-DIMM0\n\
         \tBank Locator: BANK 0\n\
         \tManufacturer: CXMT\n\
         \tSerial Number: 0\n\
         \tPart Number: ABCD5600\n\
         \tType: DDR5\n\
         \tType Detail: Synchronous Unbuffered (Unregistered)\n\
         \tSpeed: 5600 MT/s\n\
         \tConfigured Memory Speed: 5200 MT/s\n\
         \tAsset Tag: 9876543210\n\
         \tRank: 2\n\
         \tModule Manufacturer ID: 0x8A32\n\
         \tModule Product ID: 0x1234\n\
         \tMemory Subsystem Controller Manufacturer ID: 0x8086\n\
         \tMemory Subsystem Controller Product ID: 0x5678\n\
         \tMemory Technology: DRAM\n\
         \tMemory Operating Mode Capability: Volatile memory\n\
         \tFirmware Version: 1.2.3\n\
         \tNon-Volatile Size: None\n\
         \tVolatile Size: 32 GB\n\
         \tCache Size: None\n\
         \tLogical Size: 32 GB\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].serial, None);
    assert_eq!(
        records[0].error_information_handle.as_deref(),
        Some("0x0042")
    );
    assert_eq!(records[0].form_factor.as_deref(), Some("DIMM"));
    assert_eq!(records[0].set.as_deref(), Some("None"));
    assert_eq!(records[0].bank_locator.as_deref(), Some("BANK 0"));
    assert_eq!(
        records[0].type_detail.as_deref(),
        Some("Synchronous Unbuffered (Unregistered)")
    );
    assert_eq!(records[0].asset_tag.as_deref(), Some("9876543210"));
    assert_eq!(records[0].configured_speed.as_deref(), Some("5200 MT/s"));
    assert_eq!(records[0].rank.as_deref(), Some("2"));
    assert_eq!(records[0].module_manufacturer_id.as_deref(), Some("0x8A32"));
    assert_eq!(records[0].module_product_id.as_deref(), Some("0x1234"));
    assert_eq!(
        records[0]
            .memory_subsystem_controller_manufacturer_id
            .as_deref(),
        Some("0x8086")
    );
    assert_eq!(
        records[0].memory_subsystem_controller_product_id.as_deref(),
        Some("0x5678")
    );
    assert_eq!(records[0].memory_technology.as_deref(), Some("DRAM"));
    assert_eq!(
        records[0].memory_operating_mode_capability.as_deref(),
        Some("Volatile memory")
    );
    assert_eq!(records[0].firmware_version.as_deref(), Some("1.2.3"));
    assert_eq!(records[0].volatile_size.as_deref(), Some("32 GB"));
    assert_eq!(records[0].logical_size.as_deref(), Some("32 GB"));
}

#[test]
fn parses_spd_decode_dimms_records() {
    let records = parse_spd_decode_dimms(
        "Decoding EEPROM: /sys/bus/i2c/drivers/eeprom/0-0050\n\
         Guessing DIMM is in                              bank 1\n\
         ---=== SPD EEPROM Information ===---\n\
         Fundamental Memory type                         DDR4 SDRAM\n\
         ---=== Memory Characteristics ===---\n\
         Maximum module speed                            3200 MT/s (PC4-25600)\n\
         Size                                            8192 MB\n\
         ---=== Manufacturer Data ===---\n\
         Module Manufacturer                             Samsung\n\
         Assembly Serial Number                          12345678\n\
         Part Number                                     M471A1K43DB1-CWE\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].locator.as_deref(), Some("bank 1"));
    assert_eq!(records[0].manufacturer.as_deref(), Some("Samsung"));
    assert_eq!(records[0].serial.as_deref(), Some("12345678"));
    assert_eq!(records[0].part_number.as_deref(), Some("M471A1K43DB1-CWE"));
    assert_eq!(records[0].memory_type.as_deref(), Some("DDR4 SDRAM"));
    assert_eq!(
        parse_size_to_bytes(records[0].size.as_deref()),
        Some(8192 * 1024 * 1024)
    );
    assert_eq!(parse_speed_mtps(records[0].speed.as_deref()), Some(3200));
}

#[test]
fn ignores_truncated_raw_spd_eeprom() {
    assert!(parse_spd_eeprom(&[0x00, 0x00, 0x0c]).is_none());
}

#[test]
fn ignores_raw_spd_eeprom_with_invalid_width_combination() {
    let mut bytes = vec![0; 64];
    bytes[2] = 0x0c;
    bytes[4] = 0x05;
    bytes[12] = 0x03;
    bytes[13] = 0x00;

    assert!(parse_spd_eeprom(&bytes).is_none());
}

#[test]
fn parses_raw_ddr5_spd_identity_fields() {
    let record = parse_spd_eeprom(&ddr5_spd_eeprom()).expect("DDR5 SPD identity record");

    assert_eq!(record.memory_type.as_deref(), Some("DDR5 SDRAM"));
    assert_eq!(record.manufacturer.as_deref(), Some("Crucial"));
    assert_eq!(record.serial.as_deref(), Some("E6FFB785"));
    assert_eq!(record.part_number.as_deref(), Some("CT8G48C40U5.M4A1"));
    assert_eq!(record.size, None);
    assert_eq!(record.speed, None);
}

#[test]
fn parses_common_raw_spd_manufacturer_ids() {
    let cases = [
        (0x01, 0x98, "Kingston"),
        (0x04, 0x43, "Ramaxel"),
        (0x04, 0xcb, "ADATA"),
        (0x89, 0xcd, "Longsys"),
        (0x89, 0x68, "Kimtigo"),
        (0x83, 0x0b, "Nanya"),
        (0x80, 0xda, "Winbond"),
        (0x04, 0xc8, "Powerchip"),
        (0x89, 0x9b, "YMTC"),
        (0x8a, 0x91, "CXMT"),
        (0x8a, 0x8f, "UNIC"),
        (0x86, 0xc8, "GigaDevice"),
        (0x07, 0x46, "Gloway"),
        (0x08, 0x13, "Gloway"),
        (0x08, 0x1a, "UniIC"),
        (0x8a, 0x02, "KingSpec"),
        (0x89, 0xf7, "Netac"),
        (0x8a, 0xb1, "Biwin"),
    ];

    for (count, code, vendor) in cases {
        let mut bytes = ddr5_spd_eeprom();
        bytes[512] = count;
        bytes[513] = code;

        let record = parse_spd_eeprom(&bytes).expect("DDR5 SPD identity record");

        assert_eq!(record.manufacturer.as_deref(), Some(vendor), "{vendor}");
    }
}

#[test]
fn preserves_unknown_raw_spd_manufacturer_id() {
    let mut bytes = ddr5_spd_eeprom();
    bytes[512] = 0x12;
    bytes[513] = 0x34;

    let record = parse_spd_eeprom(&bytes).expect("DDR5 SPD identity record");

    assert_eq!(record.manufacturer.as_deref(), Some("JEP106 0x1234"));
}

fn ddr5_spd_eeprom() -> Vec<u8> {
    let mut bytes = vec![0; 1024];
    bytes[2] = 0x12;
    bytes[512] = 0x85;
    bytes[513] = 0x9b;
    bytes[517] = 0xe6;
    bytes[518] = 0xff;
    bytes[519] = 0xb7;
    bytes[520] = 0x85;
    bytes[521..537].copy_from_slice(b"CT8G48C40U5.M4A1");
    bytes
}

#[test]
fn parses_printer_status_and_uri() {
    let statuses = parse_lpstat_a(&hw_testdata::fixture("printer/lpstat-a.txt"));
    let uris = parse_lpstat_v(&hw_testdata::fixture("printer/lpstat-v.txt"));
    assert_eq!(statuses[0].queue, "Office_Printer");
    assert!(statuses[0].accepting);
    assert_eq!(
        uris[0].device_uri.as_deref(),
        Some("ipp://printer.local/ipp/print")
    );
}

#[test]
fn parses_cdrom_capabilities() {
    let info = parse_proc_cdrom_info(&hw_testdata::fixture("proc/cdrom-info.txt"));
    assert_eq!(info.drive_names, vec!["sr0"]);
    assert!(info.capabilities.contains(&"read-dvd".to_string()));
}

#[test]
fn parses_all_cdrom_capabilities_per_drive() {
    let info = parse_proc_cdrom_info(
        "drive name:\t\tsr0\tsr1\n\
         Can close tray:\t\t1\t0\n\
         Can open tray:\t\t1\t1\n\
         Can lock tray:\t\t0\t1\n\
         Can read multisession:\t1\t0\n\
         Can play audio:\t\t1\t1\n\
         Can write DVD-RAM:\t0\t1\n",
    );

    assert_eq!(
        info.capabilities_by_drive.get("sr0").unwrap(),
        &vec!["close-tray", "open-tray", "read-multisession", "play-audio"]
    );
    assert_eq!(
        info.capabilities_by_drive.get("sr1").unwrap(),
        &vec!["open-tray", "lock-tray", "play-audio", "write-dvd-ram"]
    );
}

#[test]
fn parses_lshw_cdrom_records() {
    let records = parse_lshw_cdrom(
        "  *-cdrom\n\
              description: DVD-RAM writer\n\
              product: DVDRAM GP60\n\
              vendor: HL-DT-ST\n\
              logical name: /dev/sr0\n\
              serial: ABC123\n\
              configuration: ansiversion=5 status=nodisc firmware=1.00\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_name.as_deref(), Some("/dev/sr0"));
    assert_eq!(records[0].product.as_deref(), Some("DVDRAM GP60"));
    assert_eq!(records[0].vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(records[0].serial.as_deref(), Some("ABC123"));
    assert_eq!(records[0].firmware.as_deref(), Some("1.00"));
}

#[test]
fn parses_hwinfo_cdrom_records() {
    let records = parse_hwinfo_cdrom(
        "24: SCSI 200.0: 10602 CD-ROM (DVD)\n\
         \tHardware Class: cdrom\n\
         \tModel: \"HL-DT-ST DVDRAM GP60\"\n\
         \tVendor: \"HL-DT-ST\"\n\
         \tDevice: \"DVDRAM GP60\"\n\
         \tRevision: \"1.00\"\n\
         \tDriver: \"sr\"\n\
         \tDriver Modules: \"sr\"\n\
         \tDevice File: /dev/sr0\n\
         \tSysFS ID: /class/block/sr0\n\
         \tSerial ID: \"ABC123\"\n\
         \n\
         25: SCSI 0.0: 10600 Disk\n\
         \tHardware Class: disk\n\
         \tDevice File: /dev/sda\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].device_node.as_deref(), Some("/dev/sr0"));
    assert_eq!(records[0].model.as_deref(), Some("HL-DT-ST DVDRAM GP60"));
    assert_eq!(records[0].vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(records[0].device.as_deref(), Some("DVDRAM GP60"));
    assert_eq!(records[0].revision.as_deref(), Some("1.00"));
    assert_eq!(records[0].driver.as_deref(), Some("sr"));
    assert_eq!(records[0].driver_modules, vec!["sr"]);
    assert_eq!(records[0].serial.as_deref(), Some("ABC123"));
}

#[test]
fn parses_hwinfo_cdrom_device_file_alias_as_primary_node() {
    let records = parse_hwinfo_cdrom(
        "24: SCSI 200.0: 10602 CD-ROM (DVD)\n\
         \tHardware Class: cdrom\n\
         \tModel: \"NECVMWar VMware SATA CD01\"\n\
         \tDevice File: /dev/sr0 (/dev/sg0)\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].device_node.as_deref(), Some("/dev/sr0"));
}

#[test]
fn parses_bluetooth_and_video() {
    let controllers = parse_hciconfig(&hw_testdata::fixture("bluetooth/hciconfig-a.txt"));
    let paired =
        parse_bluetoothctl_paired_devices(&hw_testdata::fixture("bluetooth/paired-devices.txt"));
    let cameras = parse_v4l2_list_devices(&hw_testdata::fixture("video/v4l2-list-devices.txt"));
    assert_eq!(controllers[0].address.as_deref(), Some("3C:F0:11:80:9E:19"));
    assert_eq!(paired.len(), 2);
    assert_eq!(cameras[0].nodes, vec!["/dev/video0", "/dev/video1"]);
}

#[test]
fn parses_lshw_video_camera_records() {
    let records = parse_lshw_video(
        "  *-multimedia\n\
              description: Video\n\
              product: Integrated Camera\n\
              vendor: Chicony Electronics Co., Ltd\n\
              logical name: /dev/video0\n\
              bus info: usb@1:4\n\
              configuration: driver=uvcvideo maxpower=500mA speed=480Mbit/s\n\
         \n\
         *-multimedia\n\
              description: Audio device\n\
              product: HDA Intel PCH\n\
              vendor: Intel Corporation\n\
              bus info: pci@0000:00:1f.3\n\
              configuration: driver=snd_hda_intel\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_name.as_deref(), Some("/dev/video0"));
    assert_eq!(records[0].product.as_deref(), Some("Integrated Camera"));
    assert_eq!(
        records[0].vendor.as_deref(),
        Some("Chicony Electronics Co., Ltd")
    );
    assert_eq!(records[0].bus_info.as_deref(), Some("usb@1:4"));
    assert_eq!(records[0].driver.as_deref(), Some("uvcvideo"));
}

#[test]
fn parses_lshw_bluetooth_communication_records() {
    let records = parse_lshw_communication(
        "  *-communication\n\
              description: Bluetooth wireless interface\n\
              product: Bluetooth 9460/9560 Jefferson Peak (JfP)\n\
              vendor: Intel Corporation\n\
              logical name: hci0\n\
              bus info: usb@1:4\n\
              configuration: driver=btusb maxpower=100mA speed=12Mbit/s\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_name.as_deref(), Some("hci0"));
    assert_eq!(
        records[0].product.as_deref(),
        Some("Bluetooth 9460/9560 Jefferson Peak (JfP)")
    );
    assert_eq!(records[0].vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(records[0].bus_info.as_deref(), Some("usb@1:4"));
    assert_eq!(records[0].driver.as_deref(), Some("btusb"));
}

#[test]
fn parses_v4l2_format_capabilities() {
    let capabilities = parse_v4l2_list_formats_ext(
        "ioctl: VIDIOC_ENUM_FMT\n\
         \tType: Video Capture\n\
         \n\
         \t[0]: 'MJPG' (Motion-JPEG, compressed)\n\
         \t\tSize: Discrete 1280x720\n\
         \t\tSize: Discrete 640x480\n\
         \t[1]: 'YUYV' (YUYV 4:2:2)\n\
         \t\tSize: Discrete 640x480\n",
    );

    assert_eq!(
        capabilities,
        vec!["MJPG 1280x720", "MJPG 640x480", "YUYV 640x480"]
    );
}
