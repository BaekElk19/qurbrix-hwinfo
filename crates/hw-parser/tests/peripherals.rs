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
              bus info: pci@0000:00:1f.3\n\
              configuration: driver=snd_hda_intel latency=64\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].product.as_deref(),
        Some("Alder Lake PCH-P High Definition Audio Controller")
    );
    assert_eq!(records[0].vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(records[0].bus_info.as_deref(), Some("pci@0000:00:1f.3"));
    assert_eq!(records[0].driver.as_deref(), Some("snd_hda_intel"));
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
fn parses_bluetooth_and_video() {
    let controllers = parse_hciconfig(&hw_testdata::fixture("bluetooth/hciconfig-a.txt"));
    let paired =
        parse_bluetoothctl_paired_devices(&hw_testdata::fixture("bluetooth/paired-devices.txt"));
    let cameras = parse_v4l2_list_devices(&hw_testdata::fixture("video/v4l2-list-devices.txt"));
    assert_eq!(controllers[0].address.as_deref(), Some("00:11:22:33:44:55"));
    assert_eq!(paired.len(), 2);
    assert_eq!(cameras[0].nodes, vec!["/dev/video0", "/dev/video1"]);
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
