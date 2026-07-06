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
