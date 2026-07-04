use hw_model::DeviceKind;
use hw_probe::{
    AudioProbe, BatteryProbe, CameraProbe, CdromProbe, InputProbe, PrinterProbe, Probe,
    ProbeContext,
};
use hw_source::FakeSourceRunner;
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
