use hw_model::DeviceKind;
use hw_probe::{BiosProbe, GpuProbe, MemoryProbe, MonitorProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn memory_probe_outputs_dimm_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "memory"],
        "Memory Device\n\tSize: 16 GB\n\tLocator: ChannelA-DIMM0\n\tManufacturer: Samsung\n\tSerial Number: ABCD\n\tPart Number: M471A2K43\n\tType: DDR4\n\tSpeed: 3200 MT/s\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
}

#[tokio::test]
async fn bios_probe_outputs_bios_and_motherboard_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\tVendor: LENOVO\n\tVersion: N2IET98W\n\tRelease Date: 01/01/2026\nBase Board Information\n\tManufacturer: LENOVO\n\tProduct Name: 20XX\n\tSerial Number: BOARD123\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;
    assert!(result.devices.iter().any(|d| d.kind == DeviceKind::Bios));
    assert!(result
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::Motherboard));
}

#[tokio::test]
async fn gpu_and_monitor_probes_output_devices() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n\tKernel driver in use: i915\n")
        .with_command("xrandr", ["--query"], "eDP-1 connected primary 1920x1080+0+0\n   1920x1080     60.00*+\nHDMI-1 disconnected\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    assert_eq!(GpuProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Gpu);
    assert_eq!(
        MonitorProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Monitor
    );
}
