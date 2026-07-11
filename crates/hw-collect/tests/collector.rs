use hw_collect::collect_scan_report_with_runner;
use hw_model::{DeviceKind, ScanConfig, ScanStatus};
use hw_source::FakeSourceRunner;
use std::path::PathBuf;

#[tokio::test]
async fn collector_runs_base_and_peripheral_probes() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:1f.3 Audio device [0403]: Intel Corporation HD Audio [8086:a348]\n\tKernel driver in use: snd_hda_intel\n")
        .with_command("lsusb", std::iter::empty::<&str>(), "Bus 001 Device 004: ID 0bda:5689 Realtek Integrated Camera\n")
        .with_file("/proc/asound/cards", " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n")
        .with_file("/proc/bus/input/devices", "N: Name=\"AT Keyboard\"\nH: Handlers=sysrq kbd event0 leds\n\n");

    let config = ScanConfig {
        kinds: Some(vec![
            DeviceKind::Pci,
            DeviceKind::Usb,
            DeviceKind::Audio,
            DeviceKind::Input,
        ]),
        ..Default::default()
    };

    let report = collect_scan_report_with_runner(&runner, config)
        .await
        .unwrap();
    assert_eq!(report.status, ScanStatus::Complete);
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Pci));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Usb));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Audio));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Input));
}

#[tokio::test]
async fn collector_reports_partial_when_requested_probe_source_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:1f.3 Audio device [0403]: Intel Corporation HD Audio [8086:a348]\n",
    );
    let config = ScanConfig {
        kinds: Some(vec![DeviceKind::Pci, DeviceKind::Usb]),
        ..Default::default()
    };

    let report = collect_scan_report_with_runner(&runner, config)
        .await
        .unwrap();

    assert_eq!(report.status, ScanStatus::Partial);
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Pci));
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].code, "source_missing");
    assert_eq!(report.warnings[0].source.as_deref(), Some("lsusb"));
}

#[tokio::test]
async fn collector_can_omit_sources_and_warnings_from_report() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:1f.3 Audio device [0403]: Intel Corporation HD Audio [8086:a348]\n",
    );
    let config = ScanConfig {
        kinds: Some(vec![DeviceKind::Pci, DeviceKind::Usb]),
        include_sources: false,
        include_warnings: false,
        ..Default::default()
    };

    let report = collect_scan_report_with_runner(&runner, config)
        .await
        .unwrap();

    assert_eq!(report.status, ScanStatus::Partial);
    assert!(report.warnings.is_empty());
    assert!(report
        .devices
        .iter()
        .all(|device| device.sources.is_empty()));
}

#[tokio::test]
async fn collector_populates_top_level_system_metadata_from_system_device() {
    let runner = FakeSourceRunner::new()
        .with_file("/proc/sys/kernel/hostname", "test-host\n")
        .with_file("/etc/os-release", "PRETTY_NAME=\"Test OS\"\n")
        .with_command("uname", ["-r"], "1.2.3-test\n")
        .with_command("uname", ["-m"], "test64\n");
    let config = ScanConfig {
        kinds: Some(vec![DeviceKind::System]),
        ..Default::default()
    };

    let report = collect_scan_report_with_runner(&runner, config)
        .await
        .unwrap();

    assert_eq!(report.metadata.hostname.as_deref(), Some("test-host"));
    assert_eq!(report.metadata.os.as_deref(), Some("Test OS"));
    assert_eq!(report.metadata.kernel.as_deref(), Some("1.2.3-test"));
    assert_eq!(report.metadata.architecture.as_deref(), Some("test64"));
    assert_eq!(
        report.metadata.scanner_version.as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert!(report.metadata.duration_ms.is_some());
    assert_eq!(report.system.hostname, report.metadata.hostname);
    assert_eq!(report.system.os, report.metadata.os);
    assert_eq!(report.system.kernel, report.metadata.kernel);
    assert_eq!(report.system.architecture, report.metadata.architecture);
}

#[tokio::test]
async fn default_scan_reports_unconsumed_pci_as_other_pci() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:02.0 VGA compatible controller [0300]: AMD Radeon Graphics [1002:1638]\n\tKernel driver in use: amdgpu\n\
         00:1f.3 Audio device [0403]: Intel Corporation HD Audio [8086:a348]\n\tKernel driver in use: snd_hda_intel\n",
    );

    let report = collect_scan_report_with_runner(&runner, ScanConfig::default())
        .await
        .unwrap();

    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Gpu));
    assert!(report
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::OtherPci));
    assert!(!report
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::Pci && d.id == "pci:0000:00:1f.3"));
}

#[tokio::test]
async fn default_scan_filters_pci_devices_already_exposed_by_typed_devices() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "00:1f.3 Audio device [0403]: Intel Corporation HD Audio [8086:a348]\n\tKernel driver in use: snd_hda_intel\n\
             02:00.0 Ethernet controller [0200]: Intel Corporation I219-LM [8086:15f3]\n\tKernel driver in use: e1000e\n\
             03:00.0 ISA bridge [0601]: Intel Corporation LPC Controller [8086:7a8c]\n",
        )
        .with_file(
            "/proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n",
        )
        .with_file(
            "/sys/class/sound/card0/device/uevent",
            "DRIVER=snd_hda_intel\nPCI_CLASS=40300\nPCI_ID=8086:A348\nPCI_SLOT_NAME=0000:00:1f.3\n",
        )
        .with_command(
            "ip",
            ["-j", "link"],
            r#"[{"ifname":"enp2s0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
        )
        .with_command("ip", ["-j", "addr"], "[]")
        .with_file(
            "/sys/class/net/enp2s0/device/uevent",
            "DRIVER=e1000e\nPCI_CLASS=20000\nPCI_ID=8086:15F3\nPCI_SLOT_NAME=0000:02:00.0\n",
        );

    let report = collect_scan_report_with_runner(&runner, ScanConfig::default())
        .await
        .unwrap();

    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Audio));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Network));
    assert!(!report
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::OtherPci && d.id == "other-pci:0000:00:1f.3"));
    assert!(!report
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::OtherPci && d.id == "other-pci:0000:02:00.0"));
    assert!(report
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::OtherPci && d.id == "other-pci:0000:03:00.0"));
}

#[tokio::test]
async fn default_scan_consumes_sysfs_display_pci_as_gpu_when_lspci_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![PathBuf::from("/sys/bus/pci/devices/0000:00:02.0")],
        )
        .with_file("/sys/bus/pci/devices/0000:00:02.0/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:00:02.0/device", "0x9a49\n")
        .with_file("/sys/bus/pci/devices/0000:00:02.0/class", "0x030000\n");

    let report = collect_scan_report_with_runner(&runner, ScanConfig::default())
        .await
        .unwrap();

    assert!(report
        .devices
        .iter()
        .any(|device| { device.kind == DeviceKind::Gpu && device.id == "gpu:pci:0000:00:02.0" }));
    assert!(!report
        .devices
        .iter()
        .any(|device| device.kind == DeviceKind::OtherPci));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.source.as_deref() == Some("lspci -nn -k")));
}
