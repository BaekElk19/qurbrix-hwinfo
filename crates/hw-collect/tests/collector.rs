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

    let report = collect_scan_report_with_runner(&runner, ScanConfig::default())
        .await
        .unwrap();
    assert_eq!(report.status, ScanStatus::Complete);
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Pci));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Usb));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Audio));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Input));
}
