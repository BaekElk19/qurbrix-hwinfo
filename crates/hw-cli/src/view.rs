use hw_model::{DeviceKind, ScanConfig, ScanReport};
use std::time::Duration;

/// Build the inventory collector config.
///
/// Iron rule: published snapshots are always full. CLI view filters
/// (`--kind`, `--exclude-kind`, `--no-sources`, `--no-warnings`,
/// `--no-optional-sources`) must not narrow collection or publication.
pub fn inventory_scan_config(timeout: Duration) -> ScanConfig {
    ScanConfig {
        timeout,
        ..ScanConfig::default()
    }
}

/// Optional peripheral kinds hidden by `--no-optional-sources` **display**.
/// Collection still runs these probes so the store stays complete.
pub const OPTIONAL_DISPLAY_KINDS: &[DeviceKind] = &[
    DeviceKind::Monitor,
    DeviceKind::Audio,
    DeviceKind::Bluetooth,
    DeviceKind::Input,
    DeviceKind::Camera,
    DeviceKind::Battery,
    DeviceKind::Printer,
    DeviceKind::Cdrom,
    DeviceKind::Usb,
];

/// Apply stdout-only filters after a complete inventory observation.
///
/// `status` continues to describe the underlying observation. Hiding devices or
/// warnings must not turn a partial scan into a complete one, or make an empty
/// requested view look like a failed hardware scan.
pub fn filtered_report(
    mut report: ScanReport,
    kinds: &[DeviceKind],
    excluded_kinds: &[DeviceKind],
    omit_optional: bool,
    omit_sources: bool,
    omit_warnings: bool,
) -> ScanReport {
    if !kinds.is_empty() {
        report.devices.retain(|device| kinds.contains(&device.kind));
    }
    if !excluded_kinds.is_empty() {
        report
            .devices
            .retain(|device| !excluded_kinds.contains(&device.kind));
    }
    if omit_optional {
        report
            .devices
            .retain(|device| !OPTIONAL_DISPLAY_KINDS.contains(&device.kind));
    }
    if omit_sources {
        for device in &mut report.devices {
            device.sources.clear();
        }
    }
    if omit_warnings {
        report.warnings.clear();
        for device in &mut report.devices {
            device.warnings.clear();
        }
    }
    report
}
