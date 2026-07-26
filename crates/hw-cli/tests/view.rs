use hw_cli::view::{filtered_report, inventory_scan_config};
use hw_model::{
    Device, DeviceKind, DeviceProperties, OtherDeviceInfo, ScanReport, ScanStatus, ScanWarning,
    SourceEvidence, SourceKind, SourceStatus,
};
use std::time::Duration;

fn sample_report() -> ScanReport {
    let mut report = ScanReport::empty();
    report.status = ScanStatus::Complete;
    report.devices = vec![
        device(DeviceKind::Cpu, "cpu:0"),
        device(DeviceKind::Usb, "usb:1"),
        device(DeviceKind::Storage, "storage:0"),
    ];
    report.devices[0].sources.push(SourceEvidence {
        source: "lscpu".into(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    });
    report
        .warnings
        .push(ScanWarning::new("test.warning", "optional warning"));
    report
}

fn device(kind: DeviceKind, id: &str) -> Device {
    Device::new(
        id,
        kind,
        id,
        DeviceProperties::OtherDevice(OtherDeviceInfo {
            original_kind: None,
            reason: "test".into(),
        }),
    )
}

#[test]
fn inventory_scan_config_is_always_full() {
    let config = inventory_scan_config(Duration::from_secs(7));
    assert_eq!(config.timeout, Duration::from_secs(7));
    assert!(config.kinds.is_none());
    assert!(config.exclude_kinds.is_empty());
    assert!(config.optional_sources);
    assert!(config.include_sources);
    assert!(config.include_warnings);
}

#[test]
fn kind_filter_is_display_only_and_preserves_observation_status() {
    let report = filtered_report(
        sample_report(),
        &[DeviceKind::Cpu],
        &[],
        false,
        false,
        false,
    );
    assert_eq!(report.devices.len(), 1);
    assert_eq!(report.devices[0].kind, DeviceKind::Cpu);
    assert_eq!(report.status, ScanStatus::Complete);
}

#[test]
fn empty_view_and_hidden_warnings_preserve_observation_status() {
    let mut input = sample_report();
    input.status = ScanStatus::Partial;
    let report = filtered_report(input, &[DeviceKind::Camera], &[], false, false, true);
    assert!(report.devices.is_empty());
    assert!(report.warnings.is_empty());
    assert_eq!(report.status, ScanStatus::Partial);
}

#[test]
fn no_optional_display_drops_peripheral_kinds_only() {
    let report = filtered_report(sample_report(), &[], &[], true, false, false);
    assert!(report
        .devices
        .iter()
        .all(|device| device.kind != DeviceKind::Usb));
    assert!(report
        .devices
        .iter()
        .any(|device| device.kind == DeviceKind::Cpu));
    assert!(report
        .devices
        .iter()
        .any(|device| device.kind == DeviceKind::Storage));
}

#[test]
fn no_sources_and_no_warnings_are_display_only() {
    let report = filtered_report(sample_report(), &[], &[], false, true, true);
    assert!(report.warnings.is_empty());
    assert!(report
        .devices
        .iter()
        .all(|device| device.sources.is_empty()));
    assert_eq!(report.devices.len(), 3);
}
