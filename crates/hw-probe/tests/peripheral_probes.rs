use hw_model::{
    BusInfo, DeviceKind, DeviceProperties, DriverStatus, InputKind, SourceKind, SourceStatus,
};
use hw_probe::{
    AudioProbe, BatteryProbe, BluetoothProbe, CameraProbe, CdromProbe, InputProbe, PrinterProbe,
    Probe, ProbeContext,
};
use hw_source::FakeSourceRunner;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn audio_probe_reads_proc_asound() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
        )
        .with_glob(
            "/proc/asound/card0/codec#*",
            vec![PathBuf::from("/proc/asound/card0/codec#0")],
        )
        .with_file("/proc/asound/card0/codec#0", "Codec: Realtek ALC256\n")
        .with_file(
            "/sys/class/sound/card0/device/uevent",
            "DRIVER=snd_hda_intel\n",
        )
        .with_file("/sys/class/sound/card0/device/vendor", "0x8086\n")
        .with_file("/sys/class/sound/card0/device/subsystem_vendor", "0x1028\n")
        .with_file("/sys/class/sound/card0/device/subsystem_device", "0x087c\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Audio);
    assert_eq!(result.devices[0].vendor.as_deref(), Some("Intel"));
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("snd_hda_intel")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );

    let DeviceProperties::Audio(info) = &result.devices[0].properties else {
        panic!("expected audio properties");
    };
    assert_eq!(info.codec.as_deref(), Some("Realtek ALC256"));
    assert_eq!(info.subsystem.as_deref(), Some("1028:087c"));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.source == "/proc/asound/card0/codec#0"
            && source.kind == SourceKind::Procfs));
    assert!(result.devices[0].sources.iter().any(|source| source.source
        == "/sys/class/sound/card0"
        && source.kind == SourceKind::Sysfs));
}

#[tokio::test]
async fn audio_probe_preserves_pci_identity_and_modules_from_sysfs() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
        )
        .with_file(
            "/sys/class/sound/card0/device/uevent",
            "DRIVER=snd_hda_intel\nPCI_CLASS=40300\nPCI_ID=8086:A0C8\nPCI_SUBSYS_ID=1028:087C\nPCI_SLOT_NAME=0000:00:1f.3\n",
        )
        .with_glob(
            "/sys/class/sound/card0/device/driver/module/drivers/*",
            vec![PathBuf::from(
                "/sys/class/sound/card0/device/driver/module/drivers/pci:snd_hda_intel",
            )],
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:00:1f.3".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("a0c8".to_string()),
            subsystem_vendor_id: Some("1028".to_string()),
            subsystem_device_id: Some("087c".to_string()),
            class: Some("040300".to_string()),
        })
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .map(|driver| driver.modules.as_slice()),
        Some(&["snd_hda_intel".to_string()][..])
    );
}

#[tokio::test]
async fn audio_probe_enriches_human_readable_lshw_fields() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
        )
        .with_file(
            "/sys/class/sound/card0/device/uevent",
            "DRIVER=snd_hda_intel\nPCI_CLASS=40300\nPCI_ID=8086:A0C8\nPCI_SUBSYS_ID=1028:087C\nPCI_SLOT_NAME=0000:00:1f.3\n",
        )
        .with_command(
            "lshw",
            ["-class", "multimedia"],
            "  *-multimedia\n\
                  description: Audio device\n\
                  product: Alder Lake PCH-P High Definition Audio Controller\n\
                  vendor: Intel Corporation\n\
                  bus info: pci@0000:00:1f.3\n\
                  configuration: driver=snd_hda_intel latency=64\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(
        device.model.as_deref(),
        Some("Alder Lake PCH-P High Definition Audio Controller")
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command
            && source.source == "lshw -class multimedia"));
}

#[tokio::test]
async fn audio_probe_enriches_human_readable_hwinfo_fields() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
        )
        .with_file(
            "/sys/class/sound/card0/device/uevent",
            "PCI_CLASS=40300\nPCI_ID=8086:A348\nPCI_SLOT_NAME=0000:00:1f.3\n",
        )
        .with_command(
            "hwinfo",
            ["--sound"],
            "12: PCI 1f.3: 0403 Audio device\n\
             \tHardware Class: sound\n\
             \tModel: \"Intel Cannon Lake PCH cAVS\"\n\
             \tVendor: pci 0x8086 \"Intel Corporation\"\n\
             \tDevice: pci 0xa348 \"Cannon Lake PCH cAVS\"\n\
             \tDriver: \"snd_hda_intel\"\n\
             \tDriver Modules: \"snd_hda_intel\"\n\
             \tSysFS BusID: 0000:00:1f.3\n\
             \tSysFS ID: /class/sound/card0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(device.model.as_deref(), Some("Intel Cannon Lake PCH cAVS"));
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("snd_hda_intel")
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .map(|driver| driver.modules.as_slice()),
        Some(&["snd_hda_intel".to_string()][..])
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "hwinfo --sound"));
}

#[tokio::test]
async fn audio_probe_reads_pactl_card_profiles() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
        )
        .with_command(
            "pactl",
            ["list", "cards"],
            "Card #0\n\
             \tName: alsa_card.pci-0000_00_1f.3\n\
             \tProperties:\n\
             \t\talsa.card = \"0\"\n\
             \tProfiles:\n\
             \t\toutput:analog-stereo: Analog Stereo Output (sinks: 1, sources: 0, priority: 6500, available: yes)\n\
             \t\toff: Off (sinks: 0, sources: 0, priority: 0, available: yes)\n\
             \tActive Profile: output:analog-stereo\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;

    let DeviceProperties::Audio(info) = &result.devices[0].properties else {
        panic!("expected audio properties");
    };
    assert_eq!(info.profiles, vec!["output:analog-stereo", "off"]);
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "pactl list cards"));
}

#[tokio::test]
async fn audio_probe_uses_sysfs_when_proc_asound_cards_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/sound/card*",
            vec![
                PathBuf::from("/sys/class/sound/card1"),
                PathBuf::from("/sys/class/sound/card10"),
                PathBuf::from("/sys/class/sound/card2"),
                PathBuf::from("/sys/class/sound/controlC0"),
                PathBuf::from("/sys/class/sound/pcmC0D0p"),
                PathBuf::from("/sys/class/sound/card-test"),
                PathBuf::from("/sys/class/sound/card0"),
            ],
        )
        .with_file("/sys/class/sound/card0/id", "PCH\n")
        .with_file(
            "/sys/class/sound/card0/device/uevent",
            "DRIVER=snd_hda_intel\n",
        )
        .with_file("/sys/class/sound/card0/device/subsystem_vendor", "0x1028\n")
        .with_file("/sys/class/sound/card0/device/subsystem_device", "0x087c\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 4);
    assert_eq!(result.devices[0].kind, DeviceKind::Audio);
    assert_eq!(result.devices[0].name, "PCH");
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(
        result.devices[0].sources[0].source,
        "/sys/class/sound/card0"
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);
    assert_eq!(
        result.devices[0]
            .sources
            .iter()
            .filter(|source| source.source == "/sys/class/sound/card0")
            .count(),
        1
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("snd_hda_intel")
    );

    let DeviceProperties::Audio(info) = &result.devices[0].properties else {
        panic!("expected audio properties");
    };
    assert_eq!(info.card_index, Some(0));
    assert_eq!(info.card_name.as_deref(), Some("PCH"));
    assert_eq!(info.subsystem.as_deref(), Some("1028:087c"));

    assert_eq!(result.devices[1].name, "Audio card 1");
    let DeviceProperties::Audio(info) = &result.devices[1].properties else {
        panic!("expected audio properties");
    };
    assert_eq!(info.card_index, Some(1));
    assert_eq!(info.card_name.as_deref(), Some("Audio card 1"));
    assert_eq!(result.devices[2].name, "Audio card 2");
    assert_eq!(result.devices[3].name, "Audio card 10");

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("/proc/asound/cards")
    );
}

#[tokio::test]
async fn battery_probe_reads_upower() {
    let runner = FakeSourceRunner::new().with_command(
        "upower",
        ["--dump"],
        "Device: /org/freedesktop/UPower/devices/battery_BAT0\n  native-path: BAT0\n  vendor: LGC\n  battery\n    state: discharging\n    capacity: 88%\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BatteryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Battery);
    let DeviceProperties::Battery(info) = &result.devices[0].properties else {
        panic!("expected battery properties");
    };
    assert_eq!(info.vendor.as_deref(), Some("LG Chem"));
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
        .with_file("/sys/class/power_supply/BAT0/temp", "298\n")
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
    assert_eq!(info.vendor.as_deref(), Some("LG Chem"));
    assert_eq!(info.model.as_deref(), Some("L20M4P73"));
    assert_eq!(info.serial.as_deref(), Some("ABC123"));
    assert_eq!(info.technology.as_deref(), Some("Li-ion"));
    assert_eq!(info.state.as_deref(), Some("Discharging"));
    assert_eq!(info.capacity_percent, Some(88.0));
    assert_eq!(info.energy_full_wh, Some(52.0));
    assert_eq!(info.energy_design_wh, Some(57.0));
    assert_eq!(info.energy_now_wh, Some(46.0));
    assert_eq!(info.voltage_v, Some(11.5));
    assert_eq!(info.temperature_celsius, Some(29.8));
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
async fn camera_probe_reads_driver_from_sysfs_for_v4l2_node() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "v4l2-ctl",
            ["--list-devices"],
            "Integrated Camera:\n\t/dev/video0\n",
        )
        .with_file(
            "/sys/class/video4linux/video0/device/uevent",
            "DRIVER=uvcvideo\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("uvcvideo")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.source == "/sys/class/video4linux/video0"
            && source.kind == SourceKind::Sysfs));
}

#[tokio::test]
async fn camera_probe_reads_usb_identity_from_sysfs_for_v4l2_node() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "v4l2-ctl",
            ["--list-devices"],
            "Integrated Camera:\n\t/dev/video0\n",
        )
        .with_file("/sys/class/video4linux/video0/device/../idVendor", "0bda\n")
        .with_file(
            "/sys/class/video4linux/video0/device/../idProduct",
            "5689\n",
        )
        .with_file(
            "/sys/class/video4linux/video0/device/../manufacturer",
            "Realtek Semiconductor Corp.\n",
        )
        .with_file(
            "/sys/class/video4linux/video0/device/../product",
            "Integrated Camera\n",
        )
        .with_file("/sys/class/video4linux/video0/device/../serial", "ABC123\n")
        .with_file("/sys/class/video4linux/video0/device/../busnum", "001\n")
        .with_file("/sys/class/video4linux/video0/device/../devnum", "004\n")
        .with_file("/sys/class/video4linux/video0/device/../speed", "480\n")
        .with_file(
            "/sys/class/video4linux/video0/device/bInterfaceNumber",
            "00\n",
        )
        .with_file(
            "/sys/class/video4linux/video0/device/bInterfaceClass",
            "0e\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.vendor.as_deref(),
        Some("Realtek Semiconductor Corp.")
    );
    assert_eq!(device.model.as_deref(), Some("Integrated Camera"));
    assert_eq!(device.serial.as_deref(), Some("ABC123"));
    assert_eq!(
        device.bus,
        Some(BusInfo::Usb {
            bus: Some("001".to_string()),
            device: Some("004".to_string()),
            vendor_id: Some("0bda".to_string()),
            product_id: Some("5689".to_string()),
            speed: Some("480".to_string()),
            interface: Some("00".to_string()),
            class: Some("0e".to_string()),
        })
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.source == "/sys/class/video4linux/video0"
            && source.kind == SourceKind::Sysfs));
}

#[tokio::test]
async fn camera_probe_enriches_human_readable_lshw_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "v4l2-ctl",
            ["--list-devices"],
            "Integrated Camera:\n\t/dev/video0\n",
        )
        .with_command(
            "lshw",
            ["-class", "multimedia"],
            "  *-multimedia\n\
                  description: Video\n\
                  product: ThinkPad Integrated Camera\n\
                  vendor: Chicony Electronics Co., Ltd\n\
                  logical name: /dev/video0\n\
                  bus info: usb@1:4\n\
                  configuration: driver=uvcvideo maxpower=500mA speed=480Mbit/s\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.vendor.as_deref(),
        Some("Chicony Electronics Co., Ltd")
    );
    assert_eq!(device.model.as_deref(), Some("ThinkPad Integrated Camera"));
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("uvcvideo")
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command
            && source.source == "lshw -class multimedia"));
}

#[tokio::test]
async fn camera_probe_reads_v4l2_format_capabilities() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "v4l2-ctl",
            ["--list-devices"],
            "Integrated Camera:\n\t/dev/video0\n",
        )
        .with_command(
            "v4l2-ctl",
            ["--device", "/dev/video0", "--list-formats-ext"],
            "ioctl: VIDIOC_ENUM_FMT\n\
             \t[0]: 'MJPG' (Motion-JPEG, compressed)\n\
             \t\tSize: Discrete 1280x720\n\
             \t\tSize: Discrete 640x480\n\
             \t[1]: 'YUYV' (YUYV 4:2:2)\n\
             \t\tSize: Discrete 640x480\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CameraProbe.probe(&ctx).await;

    let DeviceProperties::Camera(info) = &result.devices[0].properties else {
        panic!("expected camera properties");
    };
    assert_eq!(
        info.capabilities,
        vec!["MJPG 1280x720", "MJPG 640x480", "YUYV 640x480"]
    );
    assert!(result.devices[0].sources.iter().any(|source| {
        source.kind == SourceKind::Command
            && source.source == "v4l2-ctl --device /dev/video0 --list-formats-ext"
    }));
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
        .with_file(
            "/sys/class/video4linux/video0/device/uevent",
            "DRIVER=uvcvideo\n",
        )
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
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("uvcvideo")
    );
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
async fn bluetooth_probe_warns_when_hciconfig_parses_no_controllers() {
    let runner = FakeSourceRunner::new()
        .with_command("hciconfig", ["-a"], "no bluetooth controllers here\n")
        .with_command("bluetoothctl", ["paired-devices"], "");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BluetoothProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_empty");
    assert_eq!(result.warnings[0].source.as_deref(), Some("hciconfig -a"));
}

#[tokio::test]
async fn bluetooth_probe_enriches_from_lshw_communication_identity() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "hciconfig",
            ["-a"],
            "hci0:   Type: Primary  Bus: USB\n        BD Address: AA:BB:CC:DD:EE:FF\n        UP RUNNING PSCAN\n",
        )
        .with_command("bluetoothctl", ["paired-devices"], "")
        .with_command(
            "lshw",
            ["-class", "communication"],
            "  *-communication\n\
                  description: Bluetooth wireless interface\n\
                  product: Bluetooth 9460/9560 Jefferson Peak (JfP)\n\
                  vendor: Intel Corporation\n\
                  logical name: hci0\n\
                  configuration: driver=btusb latency=0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BluetoothProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(
        device.model.as_deref(),
        Some("Bluetooth 9460/9560 Jefferson Peak (JfP)")
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("btusb")
    );
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "lshw -class communication"
    }));
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
        .with_file("/sys/class/bluetooth/hci0/rfkill0/state", "1\n")
        .with_file("/sys/class/bluetooth/hci0/address", "AA:BB:CC:DD:EE:FF\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BluetoothProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "bluetooth:AA:BB:CC:DD:EE:FF");
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
    assert_eq!(info.address.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
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
async fn printer_probe_reads_default_destination() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lpstat",
            ["-a"],
            "Office accepting requests since now\nBackup accepting requests since now\n",
        )
        .with_command(
            "lpstat",
            ["-v"],
            "device for Office: ipp://printer.local/ipp/print\ndevice for Backup: ipp://backup.local/ipp/print\n",
        )
        .with_command("lpstat", ["-d"], "system default destination: Office\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    let DeviceProperties::Printer(office) = &result.devices[0].properties else {
        panic!("expected printer properties");
    };
    assert_eq!(office.queue_name.as_deref(), Some("Office"));
    assert_eq!(office.is_default, Some(true));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "lpstat -d"));

    let DeviceProperties::Printer(backup) = &result.devices[1].properties else {
        panic!("expected printer properties");
    };
    assert_eq!(backup.queue_name.as_deref(), Some("Backup"));
    assert_eq!(backup.is_default, Some(false));
}

#[tokio::test]
async fn printer_probe_reads_make_model_from_long_status() {
    let runner = FakeSourceRunner::new()
        .with_command("lpstat", ["-a"], "Office accepting requests since now\n")
        .with_command(
            "lpstat",
            ["-v"],
            "device for Office: ipp://printer.local/ipp/print\n",
        )
        .with_command(
            "lpstat",
            ["-l", "-p"],
            "printer Office is idle. enabled since now\n\tDescription: HP LaserJet Pro M404dn\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    let DeviceProperties::Printer(info) = &result.devices[0].properties else {
        panic!("expected printer properties");
    };
    assert_eq!(info.make_model.as_deref(), Some("HP LaserJet Pro M404dn"));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "lpstat -l -p"));
}

#[tokio::test]
async fn printer_probe_uses_uri_source_when_status_source_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "lpstat",
        ["-v"],
        "device for Office: ipp://printer.local/ipp/print\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Printer);
    assert_eq!(result.devices[0].name, "Office");
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(result.devices[0].sources[0].source, "lpstat -v");
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Command);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);

    let DeviceProperties::Printer(info) = &result.devices[0].properties else {
        panic!("expected printer properties");
    };
    assert_eq!(info.queue_name.as_deref(), Some("Office"));
    assert_eq!(
        info.device_uri.as_deref(),
        Some("ipp://printer.local/ipp/print")
    );
    assert_eq!(info.accepting, None);
    assert_eq!(info.make_model, None);
    assert_eq!(info.is_default, None);

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("lpstat -a"));
}

#[tokio::test]
async fn printer_probe_uses_uri_source_when_status_source_parses_empty() {
    let runner = FakeSourceRunner::new()
        .with_command("lpstat", ["-a"], "\n")
        .with_command(
            "lpstat",
            ["-v"],
            "device for Office: ipp://printer.local/ipp/print\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Printer);
    assert_eq!(result.devices[0].name, "Office");
    assert_eq!(result.devices[0].sources[0].source, "lpstat -v");
    let DeviceProperties::Printer(info) = &result.devices[0].properties else {
        panic!("expected printer properties");
    };
    assert_eq!(info.queue_name.as_deref(), Some("Office"));
    assert_eq!(
        info.device_uri.as_deref(),
        Some("ipp://printer.local/ipp/print")
    );
    assert_eq!(info.accepting, None);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_empty");
    assert_eq!(result.warnings[0].source.as_deref(), Some("lpstat -a"));
}

#[tokio::test]
async fn printer_probe_does_not_preserve_empty_uri_from_fallback_source() {
    let runner = FakeSourceRunner::new().with_command("lpstat", ["-v"], "device for Office:\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    let DeviceProperties::Printer(info) = &result.devices[0].properties else {
        panic!("expected printer properties");
    };
    assert_eq!(info.queue_name.as_deref(), Some("Office"));
    assert_eq!(info.device_uri, None);
}

#[tokio::test]
async fn printer_probe_reports_both_warnings_when_status_and_uri_sources_fail() {
    let runner = FakeSourceRunner::new();
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PrinterProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 2);
    assert_eq!(result.warnings[0].source.as_deref(), Some("lpstat -a"));
    assert_eq!(result.warnings[1].source.as_deref(), Some("lpstat -v"));
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

#[tokio::test]
async fn input_probe_uses_sysfs_when_proc_bus_input_devices_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/input/event*",
            vec![
                PathBuf::from("/sys/class/input/event10"),
                PathBuf::from("/sys/class/input/event2"),
                PathBuf::from("/sys/class/input/event-test"),
                PathBuf::from("/sys/class/input/event0"),
            ],
        )
        .with_file("/sys/class/input/event0/device/name", "AT Keyboard\n")
        .with_file(
            "/sys/class/input/event0/device/phys",
            "isa0060/serio0/input0\n",
        )
        .with_file("/sys/class/input/event0/device/uniq", "\n")
        .with_file("/sys/class/input/event0/device/id/bustype", "0011\n")
        .with_file("/sys/class/input/event0/device/id/vendor", "0001\n")
        .with_file("/sys/class/input/event0/device/id/product", "0001\n")
        .with_file("/sys/class/input/event0/device/id/version", "ab41\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = InputProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 3);
    assert_eq!(result.devices[0].kind, DeviceKind::Input);
    assert_eq!(result.devices[0].name, "AT Keyboard");
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(
        result.devices[0].sources[0].source,
        "/sys/class/input/event0"
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);

    let DeviceProperties::Input(info) = &result.devices[0].properties else {
        panic!("expected input properties");
    };
    assert_eq!(info.input_kind, InputKind::Keyboard);
    assert_eq!(info.event_node.as_deref(), Some("/dev/input/event0"));
    assert_eq!(info.phys.as_deref(), Some("isa0060/serio0/input0"));
    assert_eq!(info.uniq, None);
    assert!(info.handlers.is_empty());
    assert_eq!(info.bus_type.as_deref(), Some("0011"));
    assert_eq!(info.vendor_id.as_deref(), Some("0001"));
    assert_eq!(info.product_id.as_deref(), Some("0001"));
    assert_eq!(info.version.as_deref(), Some("ab41"));

    assert_eq!(result.devices[1].name, "/dev/input/event2");
    assert_eq!(result.devices[2].name, "/dev/input/event10");
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("/proc/bus/input/devices")
    );
}

#[tokio::test]
async fn input_probe_classifies_sysfs_events_from_capabilities() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/input/event*",
            vec![
                PathBuf::from("/sys/class/input/event0"),
                PathBuf::from("/sys/class/input/event1"),
                PathBuf::from("/sys/class/input/event2"),
            ],
        )
        .with_file(
            "/sys/class/input/event0/device/name",
            "Goodix Capacitive Device\n",
        )
        .with_file("/sys/class/input/event0/device/capabilities/ev", "b\n")
        .with_file("/sys/class/input/event0/device/capabilities/abs", "3\n")
        .with_file(
            "/sys/class/input/event0/device/capabilities/key",
            "400 0 0 0 0 0\n",
        )
        .with_file("/sys/class/input/event0/device/properties", "2\n")
        .with_file("/sys/class/input/event1/device/name", "ELAN Input Device\n")
        .with_file("/sys/class/input/event1/device/capabilities/ev", "b\n")
        .with_file("/sys/class/input/event1/device/capabilities/abs", "3\n")
        .with_file(
            "/sys/class/input/event1/device/capabilities/key",
            "420 0 0 0 0 0\n",
        )
        .with_file("/sys/class/input/event1/device/properties", "1\n")
        .with_file(
            "/sys/class/input/event2/device/name",
            "Wacom HID 52FD Pen\n",
        )
        .with_file("/sys/class/input/event2/device/capabilities/ev", "b\n")
        .with_file("/sys/class/input/event2/device/capabilities/abs", "3\n")
        .with_file(
            "/sys/class/input/event2/device/capabilities/key",
            "1 0 0 0 0 0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = InputProbe.probe(&ctx).await;

    let kinds = result
        .devices
        .iter()
        .map(|device| {
            let DeviceProperties::Input(info) = &device.properties else {
                panic!("expected input properties");
            };
            info.input_kind
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            InputKind::Touchscreen,
            InputKind::Touchpad,
            InputKind::Tablet
        ]
    );
}

#[tokio::test]
async fn input_probe_classifies_proc_events_from_capabilities() {
    let runner = FakeSourceRunner::new().with_file(
        "/proc/bus/input/devices",
        "I: Bus=0018 Vendor=27c6 Product=0113 Version=0100\n\
         N: Name=\"Goodix Capacitive Device\"\n\
         H: Handlers=event0\n\
         B: PROP=2\n\
         B: EV=b\n\
         B: KEY=400 0 0 0 0 0\n\
         B: ABS=3\n\n\
         I: Bus=0003 Vendor=056a Product=52fd Version=0111\n\
         N: Name=\"Wacom HID 52FD Pen\"\n\
         H: Handlers=event1\n\
         B: EV=b\n\
         B: KEY=1 0 0 0 0 0\n\
         B: ABS=3\n\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = InputProbe.probe(&ctx).await;

    let kinds = result
        .devices
        .iter()
        .map(|device| {
            let DeviceProperties::Input(info) = &device.properties else {
                panic!("expected input properties");
            };
            info.input_kind
        })
        .collect::<Vec<_>>();
    assert_eq!(kinds, vec![InputKind::Touchscreen, InputKind::Tablet]);
}

#[tokio::test]
async fn input_probe_uses_sysfs_when_proc_bus_input_devices_parses_empty() {
    let runner = FakeSourceRunner::new()
        .with_file("/proc/bus/input/devices", "\n")
        .with_glob(
            "/sys/class/input/event*",
            vec![PathBuf::from("/sys/class/input/event0")],
        )
        .with_file("/sys/class/input/event0/device/name", "AT Keyboard\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = InputProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "AT Keyboard");
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_empty");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("/proc/bus/input/devices")
    );
}

#[tokio::test]
async fn cdrom_probe_uses_sysfs_when_proc_cdrom_info_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/block/sr*",
            vec![PathBuf::from("/sys/class/block/sr0")],
        )
        .with_file("/sys/class/block/sr0/device/vendor", "HL-DT-ST\n")
        .with_file("/sys/class/block/sr0/device/model", "DVDRAM GP60\n")
        .with_file("/sys/class/block/sr0/device/serial", "ABC123\n")
        .with_file("/sys/class/block/sr0/device/rev", "1.00\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Cdrom);
    assert_eq!(result.devices[0].name, "sr0");
    assert_eq!(result.devices[0].vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(result.devices[0].model.as_deref(), Some("DVDRAM GP60"));
    assert_eq!(result.devices[0].serial.as_deref(), Some("ABC123"));
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_eq!(result.devices[0].sources[0].source, "/sys/class/block/sr0");
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.devices[0].sources[0].status, SourceStatus::Success);

    let DeviceProperties::Cdrom(info) = &result.devices[0].properties else {
        panic!("expected cdrom properties");
    };
    assert_eq!(info.device_node.as_deref(), Some("/dev/sr0"));
    assert_eq!(info.media_present, None);
    assert_eq!(info.firmware.as_deref(), Some("1.00"));
    assert!(info.capabilities.is_empty());

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("/proc/sys/dev/cdrom/info")
    );
}

#[tokio::test]
async fn cdrom_probe_enriches_proc_drives_from_sysfs_identity() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/sys/dev/cdrom/info",
            "drive name:\t\tsr0\nCan read DVD:\t\t1\n",
        )
        .with_file("/sys/class/block/sr0/device/vendor", "HL-DT-ST\n")
        .with_file("/sys/class/block/sr0/device/model", "DVDRAM GP60\n")
        .with_file("/sys/class/block/sr0/device/serial", "ABC123\n")
        .with_file("/sys/class/block/sr0/device/rev", "1.00\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(result.devices[0].model.as_deref(), Some("DVDRAM GP60"));
    assert_eq!(result.devices[0].serial.as_deref(), Some("ABC123"));
    let DeviceProperties::Cdrom(info) = &result.devices[0].properties else {
        panic!("expected cdrom properties");
    };
    assert_eq!(info.firmware.as_deref(), Some("1.00"));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs && source.source == "/sys/class/block/sr0"));
}

#[tokio::test]
async fn cdrom_probe_enriches_from_lshw_cdrom_identity() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/sys/dev/cdrom/info",
            "drive name:\t\tsr0\nCan read DVD:\t\t1\n",
        )
        .with_command(
            "lshw",
            ["-class", "disk"],
            "  *-cdrom\n\
                  description: DVD-RAM writer\n\
                  product: DVDRAM GP60\n\
                  vendor: HL-DT-ST\n\
                  logical name: /dev/sr0\n\
                  serial: ABC123\n\
                  configuration: ansiversion=5 firmware=1.00 status=nodisc\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "DVDRAM GP60");
    assert_eq!(device.vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(device.model.as_deref(), Some("DVDRAM GP60"));
    assert_eq!(device.serial.as_deref(), Some("ABC123"));
    let DeviceProperties::Cdrom(info) = &device.properties else {
        panic!("expected cdrom properties");
    };
    assert_eq!(info.firmware.as_deref(), Some("1.00"));
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "lshw -class disk"));
}

#[tokio::test]
async fn cdrom_probe_enriches_from_hwinfo_cdrom_identity() {
    let runner = FakeSourceRunner::new()
        .with_file(
            "/proc/sys/dev/cdrom/info",
            "drive name:\t\tsr0\nCan read DVD:\t\t1\n",
        )
        .with_command(
            "hwinfo",
            ["--cdrom"],
            "24: SCSI 200.0: 10602 CD-ROM (DVD)\n\
             \tHardware Class: cdrom\n\
             \tModel: \"HL-DT-ST DVDRAM GP60\"\n\
             \tVendor: \"HL-DT-ST\"\n\
             \tDevice: \"DVDRAM GP60\"\n\
             \tRevision: \"1.00\"\n\
             \tDriver: \"sr\"\n\
             \tDriver Modules: \"sr\"\n\
             \tDevice File: /dev/sr0\n\
             \tSerial ID: \"ABC123\"\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "HL-DT-ST DVDRAM GP60");
    assert_eq!(device.vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(device.model.as_deref(), Some("HL-DT-ST DVDRAM GP60"));
    assert_eq!(device.serial.as_deref(), Some("ABC123"));
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("sr")
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .map(|driver| driver.modules.as_slice()),
        Some(&["sr".to_string()][..])
    );
    assert_eq!(
        device.driver.as_ref().map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    let DeviceProperties::Cdrom(info) = &device.properties else {
        panic!("expected cdrom properties");
    };
    assert_eq!(info.firmware.as_deref(), Some("1.00"));
    assert_eq!(info.capabilities, vec!["read-dvd"]);
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "hwinfo --cdrom"));
}

#[tokio::test]
async fn cdrom_probe_uses_hwinfo_when_proc_and_sysfs_are_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "hwinfo",
        ["--cdrom"],
        "24: SCSI 200.0: 10602 CD-ROM (DVD)\n\
         \tHardware Class: cdrom\n\
         \tModel: \"HL-DT-ST DVDRAM GP60\"\n\
         \tVendor: \"HL-DT-ST\"\n\
         \tDevice File: /dev/sr0\n\
         \tRevision: \"1.00\"\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    let device = &result.devices[0];
    assert_eq!(device.name, "HL-DT-ST DVDRAM GP60");
    assert_eq!(device.vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(device.model.as_deref(), Some("HL-DT-ST DVDRAM GP60"));
    let DeviceProperties::Cdrom(info) = &device.properties else {
        panic!("expected cdrom properties");
    };
    assert_eq!(info.device_node.as_deref(), Some("/dev/sr0"));
    assert_eq!(info.firmware.as_deref(), Some("1.00"));
    assert!(info.capabilities.is_empty());
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "hwinfo --cdrom"));
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
}

#[tokio::test]
async fn cdrom_sysfs_fallback_enriches_from_lshw_cdrom_identity() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/block/sr*",
            vec![PathBuf::from("/sys/class/block/sr0")],
        )
        .with_command(
            "lshw",
            ["-class", "disk"],
            "  *-cdrom\n\
                  product: DVDRAM GP60\n\
                  vendor: HL-DT-ST\n\
                  logical name: /dev/sr0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "DVDRAM GP60");
    assert_eq!(device.vendor.as_deref(), Some("HL-DT-ST"));
    assert_eq!(device.model.as_deref(), Some("DVDRAM GP60"));
}

#[tokio::test]
async fn cdrom_probe_uses_sysfs_when_proc_cdrom_info_parses_empty() {
    let runner = FakeSourceRunner::new()
        .with_file("/proc/sys/dev/cdrom/info", "CD-ROM information\n")
        .with_glob(
            "/sys/class/block/sr*",
            vec![PathBuf::from("/sys/class/block/sr0")],
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "sr0");
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_empty");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("/proc/sys/dev/cdrom/info")
    );
}

#[tokio::test]
async fn cdrom_probe_sysfs_fallback_sorts_and_filters_sr_numbered_nodes() {
    let runner = FakeSourceRunner::new().with_glob(
        "/sys/class/block/sr*",
        vec![
            PathBuf::from("/sys/class/block/sr1"),
            PathBuf::from("/sys/class/block/sr-test"),
            PathBuf::from("/sys/class/block/sr0"),
        ],
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CdromProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    assert_eq!(result.devices[0].name, "sr0");
    assert_eq!(result.devices[1].name, "sr1");
}
