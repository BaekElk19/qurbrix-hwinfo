use hw_model::{DeviceKind, ScanReport};

pub fn table_text(report: &ScanReport, filter: Option<DeviceKind>) -> String {
    let mut out = String::from("KIND       ID                           NAME\n");
    for device in &report.devices {
        if filter.is_some_and(|kind| device.kind != kind) {
            continue;
        }
        out.push_str(&format!(
            "{:<10} {:<28} {}\n",
            device.kind, device.id, device.name
        ));
    }
    out
}
