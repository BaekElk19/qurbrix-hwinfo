use hw_model::{Device, DeviceKind, DeviceProperties, OtherDeviceInfo, OtherPciInfo};

pub fn other_pci_from(device: &Device) -> Device {
    let mut fallback = Device::new(
        device.id.replace("pci:", "other-pci:"),
        DeviceKind::OtherPci,
        device.name.clone(),
        DeviceProperties::OtherPci(OtherPciInfo {
            original_class: match &device.properties {
                DeviceProperties::Pci(pci) => pci.class_name.clone(),
                _ => None,
            },
            reason: "unclassified-pci-device".to_string(),
        }),
    );
    fallback.bus = device.bus.clone();
    fallback.driver = device.driver.clone();
    fallback.capabilities = device.capabilities.clone();
    fallback.identifiers = device.identifiers.clone();
    fallback.sources = device.sources.clone();
    fallback.warnings = device.warnings.clone();
    fallback
}

pub fn other_device_from(device: &Device) -> Device {
    Device::new(
        device.id.replace("usb:", "other-device:"),
        DeviceKind::OtherDevice,
        device.name.clone(),
        DeviceProperties::OtherDevice(OtherDeviceInfo {
            original_kind: Some(device.kind.to_string()),
            reason: "unclassified-device".to_string(),
        }),
    )
}
