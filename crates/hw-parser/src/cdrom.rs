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
