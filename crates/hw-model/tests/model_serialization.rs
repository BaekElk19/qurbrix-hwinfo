use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus, PciInfo,
    ScanReport, SourceEvidence, SourceKind, SourceStatus,
};

#[test]
fn device_kind_serializes_as_kebab_case() {
    assert_eq!(
        serde_json::to_string(&DeviceKind::OtherPci).unwrap(),
        "\"other-pci\""
    );
    assert_eq!(
        "other-device".parse::<DeviceKind>().unwrap(),
        DeviceKind::OtherDevice
    );
}

#[test]
fn pci_device_has_stable_flat_fields() {
    let device = Device::new(
        device_id::pci("0000:00:1f.3"),
        DeviceKind::Pci,
        "Intel HD Audio Controller",
        DeviceProperties::Pci(PciInfo {
            address: "0000:00:1f.3".to_string(),
            class_name: Some("Audio device".to_string()),
            class_id: Some("0403".to_string()),
            vendor: Some("Intel Corporation".to_string()),
            vendor_id: Some("8086".to_string()),
            device: Some("HD Audio Controller".to_string()),
            device_id: Some("a348".to_string()),
            subsystem_vendor_id: None,
            subsystem_device_id: None,
        }),
    )
    .with_bus(BusInfo::Pci {
        address: "0000:00:1f.3".to_string(),
        vendor_id: Some("8086".to_string()),
        device_id: Some("a348".to_string()),
        subsystem_vendor_id: None,
        subsystem_device_id: None,
        class: Some("0403".to_string()),
    })
    .with_driver(DriverInfo {
        name: Some("snd_hda_intel".to_string()),
        version: None,
        modules: vec!["snd_hda_intel".to_string()],
        provider: None,
        status: DriverStatus::InUse,
    })
    .with_source(SourceEvidence {
        source: "lspci -nn -k".to_string(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    });

    let json = serde_json::to_value(device).unwrap();
    assert_eq!(json["kind"], "pci");
    assert_eq!(json["driver"]["status"], "in_use");
    assert_eq!(json["sources"][0]["status"], "success");
}

#[test]
fn empty_report_uses_schema_v1() {
    let report = ScanReport::empty();
    assert_eq!(report.schema_version, "qurbrix.hw.scan.v1");
    assert_eq!(report.devices.len(), 0);
    assert_eq!(
        serde_json::to_value(report.status).unwrap(),
        "complete"
    );
}
