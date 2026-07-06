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
    pub power_on_hours: Option<u64>,
    pub power_cycle_count: Option<u64>,
    pub available_spare_percent: Option<u8>,
    pub available_spare_threshold_percent: Option<u8>,
    pub percentage_used: Option<u8>,
    pub data_units_read: Option<u64>,
    pub data_units_written: Option<u64>,
    pub media_errors: Option<u64>,
    pub error_log_entries: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwDiskRecord {
    pub logical_name: Option<String>,
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HwinfoDiskRecord {
    pub device_node: Option<String>,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub device: Option<String>,
    pub revision: Option<String>,
    pub driver: Option<String>,
    pub driver_modules: Vec<String>,
    pub serial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlReport {
    smart_status: Option<SmartctlStatus>,
    temperature: Option<SmartctlTemperature>,
    power_on_time: Option<SmartctlPowerOnTime>,
    power_cycle_count: Option<u64>,
    nvme_smart_health_information_log: Option<NvmeSmartHealthInformationLog>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlStatus {
    passed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlTemperature {
    current: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlPowerOnTime {
    hours: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct NvmeSmartHealthInformationLog {
    available_spare: Option<u8>,
    available_spare_threshold: Option<u8>,
    percentage_used: Option<u8>,
    data_units_read: Option<u64>,
    data_units_written: Option<u64>,
    media_errors: Option<u64>,
    num_err_log_entries: Option<u64>,
}

pub fn parse_lsblk_json(input: &str) -> Vec<LsblkDevice> {
    parse_lsblk_json_result(input).unwrap_or_default()
}

pub fn parse_lsblk_json_result(input: &str) -> Result<Vec<LsblkDevice>, serde_json::Error> {
    serde_json::from_str::<LsblkReport>(input).map(|report| report.blockdevices)
}

pub fn parse_smartctl_json(input: &str) -> Result<SmartctlInfo, serde_json::Error> {
    serde_json::from_str::<SmartctlReport>(input).map(|report| {
        let nvme = report.nvme_smart_health_information_log;
        SmartctlInfo {
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
            power_on_hours: report.power_on_time.and_then(|value| value.hours),
            power_cycle_count: report.power_cycle_count,
            available_spare_percent: nvme.as_ref().and_then(|value| value.available_spare),
            available_spare_threshold_percent: nvme
                .as_ref()
                .and_then(|value| value.available_spare_threshold),
            percentage_used: nvme.as_ref().and_then(|value| value.percentage_used),
            data_units_read: nvme.as_ref().and_then(|value| value.data_units_read),
            data_units_written: nvme.as_ref().and_then(|value| value.data_units_written),
            media_errors: nvme.as_ref().and_then(|value| value.media_errors),
            error_log_entries: nvme.and_then(|value| value.num_err_log_entries),
        }
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

pub fn parse_hwinfo_disk(input: &str) -> Vec<HwinfoDiskRecord> {
    let mut records = Vec::new();
    let mut section = Vec::new();

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            push_hwinfo_disk_record(&mut records, parse_hwinfo_disk_section(&section));
            section.clear();
            continue;
        }
        section.push(line);
    }

    records
}

fn parse_hwinfo_disk_section(lines: &[&str]) -> Option<HwinfoDiskRecord> {
    let mut record = HwinfoDiskRecord::default();
    let mut is_disk = false;

    for line in lines {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Hardware Class" => is_disk = value == "disk",
            "Model" => record.model = clean_hwinfo_disk_value(value),
            "Vendor" | "SubVendor" => record.vendor = clean_hwinfo_disk_value(value),
            "Device" => record.device = clean_hwinfo_disk_value(value),
            "Revision" => record.revision = clean_hwinfo_disk_value(value),
            "Driver" => record.driver = clean_hwinfo_disk_value(value),
            "Driver Modules" => record.driver_modules = clean_hwinfo_disk_modules(value),
            "Device File" => record.device_node = clean_hwinfo_disk_value(value),
            "SysFS ID" => {
                if record.device_node.is_none() {
                    record.device_node = hwinfo_disk_node_from_sysfs_id(value);
                }
            }
            "Serial ID" => record.serial = clean_hwinfo_disk_value(value),
            _ => {}
        }
    }

    is_disk.then_some(record)
}

fn push_hwinfo_disk_record(records: &mut Vec<HwinfoDiskRecord>, record: Option<HwinfoDiskRecord>) {
    if let Some(record) = record {
        if record.device_node.is_some()
            || record.model.is_some()
            || record.vendor.is_some()
            || record.device.is_some()
            || record.revision.is_some()
            || record.driver.is_some()
            || !record.driver_modules.is_empty()
            || record.serial.is_some()
        {
            records.push(record);
        }
    }
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

fn clean_hwinfo_disk_value(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.split('"').nth(1).unwrap_or(value).trim();
    if value.is_empty() || value.contains("unknown") {
        None
    } else {
        Some(value.to_string())
    }
}

fn clean_hwinfo_disk_modules(value: &str) -> Vec<String> {
    let quoted = value
        .split('"')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part.trim()))
        .filter(|part| !part.is_empty() && !part.contains("unknown"))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if !quoted.is_empty() {
        return quoted;
    }

    value
        .split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.contains("unknown"))
        .map(ToString::to_string)
        .collect()
}

fn hwinfo_disk_node_from_sysfs_id(value: &str) -> Option<String> {
    let value = clean_hwinfo_disk_value(value)?;
    let name = value.strip_prefix("/class/block/")?;
    Some(format!("/dev/{name}"))
}
