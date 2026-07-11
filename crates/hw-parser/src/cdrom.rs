use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CdromProcInfo {
    pub drive_names: Vec<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwCdromRecord {
    pub logical_name: Option<String>,
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HwinfoCdromRecord {
    pub device_node: Option<String>,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub device: Option<String>,
    pub revision: Option<String>,
    pub driver: Option<String>,
    pub driver_modules: Vec<String>,
    pub serial: Option<String>,
}

pub fn parse_proc_cdrom_info(input: &str) -> CdromProcInfo {
    let mut info = CdromProcInfo::default();
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("drive name:") {
            info.drive_names = rest.split_whitespace().map(ToOwned::to_owned).collect();
        } else if line.starts_with("Can read DVD:") && line.ends_with('1') {
            info.capabilities.push("read-dvd".to_string());
        } else if line.starts_with("Can write CD-R:") && line.ends_with('1') {
            info.capabilities.push("write-cd-r".to_string());
        } else if line.starts_with("Can open tray:") && line.ends_with('1') {
            info.capabilities.push("open-tray".to_string());
        }
    }
    info
}

pub fn parse_lshw_cdrom(input: &str) -> Vec<LshwCdromRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwCdromRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-cdrom") {
            push_lshw_cdrom_record(&mut records, current.take());
            current = Some(LshwCdromRecord::default());
            continue;
        }
        if trimmed.starts_with("*-") {
            push_lshw_cdrom_record(&mut records, current.take());
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
            "logical name" => record.logical_name = clean_lshw_cdrom_value(value),
            "product" => record.product = clean_lshw_cdrom_value(value),
            "vendor" => record.vendor = clean_lshw_cdrom_value(value),
            "serial" => record.serial = clean_lshw_cdrom_value(value),
            "configuration" => parse_lshw_cdrom_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_cdrom_record(&mut records, current.take());
    records
}

pub fn parse_hwinfo_cdrom(input: &str) -> Vec<HwinfoCdromRecord> {
    let mut records = Vec::new();
    let mut section = Vec::new();

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            push_hwinfo_cdrom_record(&mut records, parse_hwinfo_cdrom_section(&section));
            section.clear();
            continue;
        }
        section.push(line);
    }

    records
}

fn parse_hwinfo_cdrom_section(lines: &[&str]) -> Option<HwinfoCdromRecord> {
    let mut record = HwinfoCdromRecord::default();
    let mut is_cdrom = false;

    for line in lines {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Hardware Class" => is_cdrom = value == "cdrom",
            "Model" => record.model = clean_hwinfo_cdrom_value(value),
            "Vendor" | "SubVendor" => record.vendor = clean_hwinfo_cdrom_value(value),
            "Device" => record.device = clean_hwinfo_cdrom_value(value),
            "Revision" => record.revision = clean_hwinfo_cdrom_value(value),
            "Driver" => record.driver = clean_hwinfo_cdrom_value(value),
            "Driver Modules" => record.driver_modules = clean_hwinfo_cdrom_modules(value),
            "Device File" => record.device_node = clean_hwinfo_cdrom_device_node(value),
            "SysFS ID" => {
                if record.device_node.is_none() {
                    record.device_node = hwinfo_cdrom_node_from_sysfs_id(value);
                }
            }
            "Serial ID" => record.serial = clean_hwinfo_cdrom_value(value),
            _ => {}
        }
    }

    is_cdrom.then_some(record)
}

fn push_hwinfo_cdrom_record(
    records: &mut Vec<HwinfoCdromRecord>,
    record: Option<HwinfoCdromRecord>,
) {
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

fn push_lshw_cdrom_record(records: &mut Vec<LshwCdromRecord>, record: Option<LshwCdromRecord>) {
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

fn parse_lshw_cdrom_configuration(record: &mut LshwCdromRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "firmware" {
            record.firmware = clean_lshw_cdrom_value(value);
        }
    }
}

fn clean_lshw_cdrom_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}

fn clean_hwinfo_cdrom_value(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.split('"').nth(1).unwrap_or(value).trim();
    if value.is_empty() || value.contains("unknown") {
        None
    } else {
        Some(value.to_string())
    }
}

fn clean_hwinfo_cdrom_device_node(value: &str) -> Option<String> {
    let value = clean_hwinfo_cdrom_value(value)?;
    value
        .split_whitespace()
        .find(|part| part.starts_with("/dev/"))
        .map(|part| part.trim_matches(|ch| ch == '(' || ch == ')').to_string())
}

fn clean_hwinfo_cdrom_modules(value: &str) -> Vec<String> {
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

fn hwinfo_cdrom_node_from_sysfs_id(value: &str) -> Option<String> {
    let value = clean_hwinfo_cdrom_value(value)?;
    let name = value.strip_prefix("/class/block/")?;
    Some(format!("/dev/{name}"))
}
