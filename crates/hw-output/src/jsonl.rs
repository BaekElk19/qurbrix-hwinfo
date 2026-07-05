use crate::flat::to_flat_device;
use anyhow::Result;
use hw_model::ScanReport;

pub fn to_jsonl(report: &ScanReport) -> Result<String> {
    let mut lines = Vec::new();
    for device in &report.devices {
        lines.push(serde_json::to_string(&to_flat_device(device))?);
    }
    Ok(lines.join("\n"))
}
