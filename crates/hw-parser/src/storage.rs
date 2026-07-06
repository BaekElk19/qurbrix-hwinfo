use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsblkReport {
    #[serde(default)]
    pub blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsblkDevice {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub size: Option<u64>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub tran: Option<String>,
    pub wwn: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SmartctlInfo {
    pub smart_status: Option<String>,
    pub temperature_celsius: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwDiskRecord {
    pub logical_name: Option<String>,
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlReport {
    smart_status: Option<SmartctlStatus>,
    temperature: Option<SmartctlTemperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlStatus {
    passed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlTemperature {
    current: Option<f32>,
}

pub fn parse_lsblk_json(input: &str) -> Vec<LsblkDevice> {
    parse_lsblk_json_result(input).unwrap_or_default()
}

pub fn parse_lsblk_json_result(input: &str) -> Result<Vec<LsblkDevice>, serde_json::Error> {
    serde_json::from_str::<LsblkReport>(input).map(|report| report.blockdevices)
}

pub fn parse_smartctl_json(input: &str) -> Result<SmartctlInfo, serde_json::Error> {
    serde_json::from_str::<SmartctlReport>(input).map(|report| SmartctlInfo {
        smart_status: report
            .smart_status
            .and_then(|status| status.passed)
            .map(|passed| {
                if passed {
                    "passed".to_string()
                } else {
                    "failed".to_string()
                }
            }),
        temperature_celsius: report
            .temperature
            .and_then(|temperature| temperature.current),
    })
}

pub fn parse_lshw_disk(input: &str) -> Vec<LshwDiskRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwDiskRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-disk") {
            push_lshw_disk_record(&mut records, current.take());
            current = Some(LshwDiskRecord::default());
            continue;
        }
        if trimmed.starts_with("*-") {
            push_lshw_disk_record(&mut records, current.take());
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "logical name" => record.logical_name = clean_lshw_disk_value(value),
            "product" => record.product = clean_lshw_disk_value(value),
            "vendor" => record.vendor = clean_lshw_disk_value(value),
            "serial" => record.serial = clean_lshw_disk_value(value),
            "configuration" => parse_lshw_disk_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_disk_record(&mut records, current.take());
    records
}

fn push_lshw_disk_record(records: &mut Vec<LshwDiskRecord>, record: Option<LshwDiskRecord>) {
    if let Some(record) = record {
        if record.logical_name.is_some()
            || record.product.is_some()
            || record.vendor.is_some()
            || record.serial.is_some()
        {
            records.push(record);
        }
    }
}

fn parse_lshw_disk_configuration(record: &mut LshwDiskRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "firmware" {
            record.firmware = clean_lshw_disk_value(value);
        }
    }
}

fn clean_lshw_disk_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}
