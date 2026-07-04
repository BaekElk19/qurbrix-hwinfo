use hw_model::{Device, ScanMetadata, ScanReport, ScanStatus, ScanWarning};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatScanReportView {
    pub schema_version: String,
    pub status: ScanStatus,
    pub metadata: ScanMetadata,
    pub summary: FlatSummary,
    pub devices: Vec<FlatDeviceView>,
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatSummary {
    pub device_count: usize,
    pub counts_by_kind: BTreeMap<String, usize>,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatDeviceView {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub bus: Option<Value>,
    pub driver: Option<Value>,
    pub capabilities: Vec<String>,
    pub identifiers: Vec<Value>,
    pub properties: Value,
    pub sources: Vec<Value>,
    pub warnings: Vec<ScanWarning>,
}

pub fn to_flat_report(report: &ScanReport) -> FlatScanReportView {
    let mut counts_by_kind = BTreeMap::new();
    for device in &report.devices {
        *counts_by_kind.entry(device.kind.to_string()).or_insert(0) += 1;
    }
    FlatScanReportView {
        schema_version: report.schema_version.clone(),
        status: report.status,
        metadata: report.metadata.clone(),
        summary: FlatSummary {
            device_count: report.devices.len(),
            counts_by_kind,
            warning_count: report.warnings.len(),
        },
        devices: report.devices.iter().map(to_flat_device).collect(),
        warnings: report.warnings.clone(),
    }
}

pub fn to_flat_device(device: &Device) -> FlatDeviceView {
    FlatDeviceView {
        id: device.id.clone(),
        kind: device.kind.to_string(),
        name: device.name.clone(),
        vendor: device.vendor.clone(),
        model: device.model.clone(),
        serial: device.serial.clone(),
        bus: device
            .bus
            .as_ref()
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
        driver: device
            .driver
            .as_ref()
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
        capabilities: device.capabilities.clone(),
        identifiers: device
            .identifiers
            .iter()
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
            .collect(),
        properties: serde_json::to_value(&device.properties).unwrap_or(Value::Null),
        sources: device
            .sources
            .iter()
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
            .collect(),
        warnings: device.warnings.clone(),
    }
}
