use hw_model::{DeviceKind, DeviceProperties};
use hw_probe::{PciProbe, Probe, ProbeContext, UsbProbe};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn pci_probe_builds_devices_with_driver_info() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:1f.3 Audio device [0403]: Intel Corporation Cannon Lake PCH cAVS [8086:a348]\n\tKernel driver in use: snd_hda_intel\n\tKernel modules: snd_hda_intel\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PciProbe.probe(&ctx).await;
    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Pci);
    assert_eq!(
        result.devices[0].driver.as_ref().unwrap().name.as_deref(),
        Some("snd_hda_intel")
    );
    assert!(matches!(
        result.devices[0].properties,
        DeviceProperties::Pci(_)
    ));
}

#[tokio::test]
async fn usb_probe_builds_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "lsusb",
        std::iter::empty::<&str>(),
        "Bus 001 Device 004: ID 0bda:5689 Realtek Semiconductor Corp. Integrated Camera\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = UsbProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].id, "usb:001:004");
    assert_eq!(result.devices[0].kind, DeviceKind::Usb);
}
