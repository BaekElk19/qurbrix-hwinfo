use hw_model::{BusInfo, DeviceKind, DeviceProperties, SourceKind, SourceStatus};
use hw_probe::{PciProbe, Probe, ProbeContext, UsbProbe};
use hw_source::FakeSourceRunner;
use std::path::PathBuf;
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

#[tokio::test]
async fn usb_probe_filters_root_hubs_and_usb_hubs() {
    let runner = FakeSourceRunner::new().with_command(
        "lsusb",
        std::iter::empty::<&str>(),
        "Bus 001 Device 001: ID 1d6b:0002 Linux Foundation 2.0 root hub\n\
         Bus 001 Device 002: ID 05e3:0610 Genesys Logic, Inc. Hub\n\
         Bus 001 Device 003: ID 046d:c534 Logitech, Inc. USB Receiver\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = UsbProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "Logitech, Inc. USB Receiver");
}

#[tokio::test]
async fn usb_probe_keeps_host_controller_text_from_lsusb_success_path() {
    let runner = FakeSourceRunner::new().with_command(
        "lsusb",
        std::iter::empty::<&str>(),
        "Bus 001 Device 002: ID 1234:5678 Example xHCI Host Controller\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = UsbProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "Example xHCI Host Controller");
}

#[tokio::test]
async fn usb_probe_uses_sysfs_when_lsusb_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/usb/devices/*",
            vec![
                PathBuf::from("/sys/bus/usb/devices/1-2"),
                PathBuf::from("/sys/bus/usb/devices/usb1"),
            ],
        )
        .with_file("/sys/bus/usb/devices/1-2/busnum", "001\n")
        .with_file("/sys/bus/usb/devices/1-2/devnum", "004\n")
        .with_file("/sys/bus/usb/devices/1-2/idVendor", "0bda\n")
        .with_file("/sys/bus/usb/devices/1-2/idProduct", "5689\n")
        .with_file("/sys/bus/usb/devices/1-2/bDeviceClass", "ef\n")
        .with_file("/sys/bus/usb/devices/1-2/bDeviceSubClass", "02\n")
        .with_file("/sys/bus/usb/devices/1-2/bDeviceProtocol", "01\n")
        .with_file(
            "/sys/bus/usb/devices/1-2/manufacturer",
            "Realtek Semiconductor Corp.\n",
        )
        .with_file("/sys/bus/usb/devices/1-2/product", "Integrated Camera\n")
        .with_file("/sys/bus/usb/devices/1-2/serial", "ABC123\n")
        .with_file("/sys/bus/usb/devices/1-2/speed", "480\n")
        .with_file(
            "/sys/bus/usb/devices/usb1/product",
            "xHCI Host Controller\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = UsbProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    let device = &result.devices[0];
    assert_eq!(device.id, "usb:0bda:5689:ABC123");
    assert_eq!(device.name, "Integrated Camera");
    assert_eq!(
        device.bus,
        Some(BusInfo::Usb {
            bus: Some("001".to_string()),
            device: Some("004".to_string()),
            vendor_id: Some("0bda".to_string()),
            product_id: Some("5689".to_string()),
            interface: None,
            class: Some("ef".to_string()),
        })
    );
    assert!(matches!(
        &device.properties,
        DeviceProperties::Usb(info)
            if info.bus_number.as_deref() == Some("001")
                && info.device_number.as_deref() == Some("004")
                && info.vendor_id.as_deref() == Some("0bda")
                && info.product_id.as_deref() == Some("5689")
                && info.class.as_deref() == Some("ef")
                && info.subclass.as_deref() == Some("02")
                && info.protocol.as_deref() == Some("01")
                && info.manufacturer.as_deref() == Some("Realtek Semiconductor Corp.")
                && info.product.as_deref() == Some("Integrated Camera")
                && info.serial.as_deref() == Some("ABC123")
                && info.speed.as_deref() == Some("480")
    ));
    assert_eq!(device.sources.len(), 1);
    assert_eq!(device.sources[0].source, "/sys/bus/usb/devices/1-2");
    assert_eq!(device.sources[0].kind, SourceKind::Sysfs);
    assert_eq!(device.sources[0].status, SourceStatus::Success);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("lsusb"));
}

#[tokio::test]
async fn usb_probe_ignores_sysfs_interface_entries() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/usb/devices/*",
            vec![PathBuf::from("/sys/bus/usb/devices/1-2:1.0")],
        )
        .with_file("/sys/bus/usb/devices/1-2:1.0/bInterfaceClass", "ff\n")
        .with_file("/sys/bus/usb/devices/1-2:1.0/bInterfaceSubClass", "00\n")
        .with_file("/sys/bus/usb/devices/1-2:1.0/bInterfaceProtocol", "00\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = UsbProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
}
