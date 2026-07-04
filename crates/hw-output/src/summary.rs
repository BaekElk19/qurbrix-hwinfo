use hw_model::ScanReport;
use std::collections::BTreeMap;

pub fn summary_text(report: &ScanReport) -> String {
    let mut counts = BTreeMap::new();
    for device in &report.devices {
        *counts.entry(device.kind.to_string()).or_insert(0usize) += 1;
    }
    let mut text = format!(
        "Status: {:?}\nDevices: {}\nWarnings: {}\n",
        report.status,
        report.devices.len(),
        report.warnings.len()
    );
    for (kind, count) in counts {
        text.push_str(&format!("{}: {}\n", kind, count));
    }
    text
}
