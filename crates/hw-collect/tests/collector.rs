use hw_collect::collect_scan_report_with_runner;
use hw_model::{DeviceKind, ScanConfig, ScanStatus};
use hw_source::FakeSourceRunner;

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
