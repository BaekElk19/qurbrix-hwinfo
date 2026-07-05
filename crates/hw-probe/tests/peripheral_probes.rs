use hw_model::{DeviceKind, DeviceProperties, SourceKind, SourceStatus};
use hw_probe::{
    AudioProbe, BatteryProbe, BluetoothProbe, CameraProbe, CdromProbe, InputProbe, PrinterProbe,
    Probe, ProbeContext,
};
use hw_source::FakeSourceRunner;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn audio_probe_reads_proc_asound() {
    let runner = FakeSourceRunner::new().with_file(
        "/proc/asound/cards",
        " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Audio);
}

#[tokio::test]
async fn battery_probe_reads_upower() {
    let runner = FakeSourceRunner::new().with_command(
        "upower",
        ["--dump"],
        "Device: /org/freedesktop/UPower/devices/battery_BAT0\n  native-path: BAT0\n  battery\n    state: discharging\n    capacity: 88%\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BatteryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Battery);
}

#[tokio::test]
async fn battery_probe_filters_line_power_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "upower",
        ["--dump"],
        "Device: /org/freedesktop/UPower/devices/line_power_AC\n  native-path: AC\n  line-power\n    online: yes\n\
         Device: /org/freedesktop/UPower/devices/battery_BAT0\n  native-path: BAT0\n  battery\n    state: discharging\n    capacity: 88%\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BatteryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Battery);
    assert_eq!(result.devices[0].name, "BAT0");
}

#[tokio::test]
async fn battery_probe_uses_sysfs_when_upower_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/power_supply/BAT*",
            vec![
                PathBuf::from("/sys/class/power_supply/BAT0"),
                PathBuf::from("/sys/class/power_supply/BAT1"),
            ],
        )
        .with_file("/sys/class/power_supply/BAT0/type", "Battery\n")
        .with_file("/sys/class/power_supply/BAT0/manufacturer", "LGC\n")
        .with_file("/sys/class/power_supply/BAT0/model_name", "L20M4P73\n")
        .with_file("/sys/class/power_supply/BAT0/serial_number", "ABC123\n")
        .with_file("/sys/class/power_supply/BAT0/technology", "Li-ion\n")
        .with_file("/sys/class/power_supply/BAT0/status", "Discharging\n")
        .with_file("/sys/class/power_supply/BAT0/capacity", "88\n")
        .with_file("/sys/class/power_supply/BAT0/energy_full", "52000000\n")
        .with_file(
            "/sys/class/power_supply/BAT0/energy_full_design",
            "57000000\n",
        )
        .with_file("/sys/class/power_supply/BAT0/energy_now", "46000000\n")
        .with_file("/sys/class/power_supply/BAT0/voltage_now", "11500000\n")
        .with_file("/sys/class/power_supply/BAT0/cycle_count", "321\n")
        .with_file("/sys/class/power_supply/BAT0/present", "1\n")
        .with_file("/sys/class/power_supply/BAT1/type", "Mains\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BatteryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Battery);
    assert_eq!(result.devices[0].name, "BAT0");
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(
        result.devices[0].sources[0].source,
        "/sys/class/power_supply/BAT0"
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);

    let DeviceProperties::Battery(info) = &result.devices[0].properties else {
        panic!("expected battery properties");
    };
    assert_eq!(info.vendor.as_deref(), Some("LGC"));
    assert_eq!(info.model.as_deref(), Some("L20M4P73"));
    assert_eq!(info.serial.as_deref(), Some("ABC123"));
    assert_eq!(info.technology.as_deref(), Some("Li-ion"));
    assert_eq!(info.state.as_deref(), Some("Discharging"));
    assert_eq!(info.capacity_percent, Some(88.0));
    assert_eq!(info.energy_full_wh, Some(52.0));
    assert_eq!(info.energy_design_wh, Some(57.0));
    assert_eq!(info.energy_now_wh, Some(46.0));
    assert_eq!(info.voltage_v, Some(11.5));
    assert_eq!(info.cycle_count, Some(321));
    assert_eq!(info.present, Some(true));

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("upower --dump"));
}

#[tokio::test]
async fn camera_probe_emits_one_device_per_physical_camera() {
    let runner = FakeSourceRunner::new().with_command(
        "v4l2-ctl",
        ["--list-devices"],
        "Integrated Camera:\n\t/dev/video0\n\t/dev/video1\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "Integrated Camera");
}

#[tokio::test]
async fn camera_probe_uses_sysfs_when_v4l2_ctl_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/video4linux/video*",
            vec![
                PathBuf::from("/sys/class/video4linux/video1"),
                PathBuf::from("/sys/class/video4linux/video0"),
            ],
        )
        .with_file("/sys/class/video4linux/video0/name", "Integrated Camera\n")
        .with_file("/sys/class/video4linux/video1/name", "Integrated Camera\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    assert_eq!(result.devices[0].kind, DeviceKind::Camera);
    assert_eq!(result.devices[0].name, "Integrated Camera");
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(
        result.devices[0].sources[0].source,
        "/sys/class/video4linux/video0"
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);

    let DeviceProperties::Camera(info) = &result.devices[0].properties else {
        panic!("expected camera properties");
    };
    assert_eq!(info.video_node.as_deref(), Some("/dev/video0"));
    assert!(info.capabilities.is_empty());
    let DeviceProperties::Camera(info) = &result.devices[1].properties else {
        panic!("expected camera properties");
    };
    assert_eq!(result.devices[1].name, "Integrated Camera");
    assert_eq!(info.video_node.as_deref(), Some("/dev/video1"));

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("v4l2-ctl --list-devices")
    );
}

#[tokio::test]
async fn camera_probe_uses_video_node_name_when_sysfs_name_is_missing() {
    let runner = FakeSourceRunner::new().with_glob(
        "/sys/class/video4linux/video*",
        vec![PathBuf::from("/sys/class/video4linux/video2")],
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "/dev/video2");
    let DeviceProperties::Camera(info) = &result.devices[0].properties else {
        panic!("expected camera properties");
    };
    assert_eq!(info.video_node.as_deref(), Some("/dev/video2"));
}

#[tokio::test]
async fn bluetooth_probe_warns_when_paired_devices_source_fails() {
    let runner = FakeSourceRunner::new().with_command(
        "hciconfig",
        ["-a"],
        "hci0:   Type: Primary  Bus: USB\n        BD Address: AA:BB:CC:DD:EE:FF\n        UP RUNNING PSCAN\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BluetoothProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("bluetoothctl paired-devices")
    );
}

#[tokio::test]
async fn bluetooth_probe_uses_sysfs_when_hciconfig_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/bluetooth/hci*",
            vec![PathBuf::from("/sys/class/bluetooth/hci0")],
        )
        .with_glob(
            "/sys/class/bluetooth/hci0/rfkill*",
            vec![PathBuf::from("/sys/class/bluetooth/hci0/rfkill0")],
        )
        .with_file("/sys/class/bluetooth/hci0/rfkill0/name", "hci0\n")
        .with_file("/sys/class/bluetooth/hci0/rfkill0/state", "1\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BluetoothProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Bluetooth);
    assert_eq!(result.devices[0].name, "hci0");
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(
        result.devices[0].sources[0].source,
        "/sys/class/bluetooth/hci0"
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);

    let DeviceProperties::Bluetooth(info) = &result.devices[0].properties else {
        panic!("expected bluetooth properties");
    };
    assert_eq!(info.controller_name.as_deref(), Some("hci0"));
    assert_eq!(info.powered, Some(true));
    assert_eq!(info.discoverable, None);
    assert_eq!(info.paired_device_count, None);
    assert!(info.paired_devices.is_empty());

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("hciconfig -a"));
}

#[tokio::test]
async fn bluetooth_probe_maps_blocked_rfkill_state_from_sysfs() {
    for state in ["0\n", "2\n"] {
        let runner = FakeSourceRunner::new()
            .with_glob(
                "/sys/class/bluetooth/hci*",
                vec![PathBuf::from("/sys/class/bluetooth/hci0")],
            )
            .with_glob(
                "/sys/class/bluetooth/hci0/rfkill*",
                vec![PathBuf::from("/sys/class/bluetooth/hci0/rfkill0")],
            )
            .with_file("/sys/class/bluetooth/hci0/rfkill0/state", state);
        let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
        let result = BluetoothProbe.probe(&ctx).await;

        let DeviceProperties::Bluetooth(info) = &result.devices[0].properties else {
            panic!("expected bluetooth properties");
        };
        assert_eq!(result.devices[0].name, "hci0");
        assert_eq!(info.powered, Some(false));
    }
}

#[tokio::test]
async fn bluetooth_probe_ignores_unknown_rfkill_state_from_sysfs() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/bluetooth/hci*",
            vec![PathBuf::from("/sys/class/bluetooth/hci0")],
        )
        .with_glob(
            "/sys/class/bluetooth/hci0/rfkill*",
            vec![PathBuf::from("/sys/class/bluetooth/hci0/rfkill0")],
        )
        .with_file("/sys/class/bluetooth/hci0/rfkill0/state", "unknown\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BluetoothProbe.probe(&ctx).await;

    let DeviceProperties::Bluetooth(info) = &result.devices[0].properties else {
        panic!("expected bluetooth properties");
    };
    assert_eq!(result.devices[0].name, "hci0");
    assert_eq!(info.powered, None);
}

#[tokio::test]
async fn printer_probe_warns_when_uri_source_fails() {
    let runner = FakeSourceRunner::new().with_command(
        "lpstat",
        ["-a"],
        "Office accepting requests since now\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("lpstat -v"));
}

#[tokio::test]
async fn input_camera_printer_and_cdrom_probes_create_devices() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/bus/input/devices",
            "N: Name=\"AT Keyboard\"\nH: Handlers=sysrq kbd event0 leds\n\n",
        )
        .with_command(
            "v4l2-ctl",
            ["--list-devices"],
            "Integrated Camera:\n\t/dev/video0\n",
        )
        .with_command("lpstat", ["-a"], "Office accepting requests since now\n")
        .with_command(
            "lpstat",
            ["-v"],
            "device for Office: ipp://printer.local/ipp/print\n",
        )
        .with_file(
            "/proc/sys/dev/cdrom/info",
            "drive name:\t\tsr0\nCan read DVD:\t\t1\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    assert_eq!(
        InputProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Input
    );
    assert_eq!(
        CameraProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Camera
    );
    assert_eq!(
        PrinterProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Printer
    );
    assert_eq!(
        CdromProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Cdrom
    );
}
