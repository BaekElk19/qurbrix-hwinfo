use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BluetoothControllerRecord {
    pub name: Option<String>,
    pub address: Option<String>,
    pub bus: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothPairedDeviceRecord {
    pub address: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwCommunicationRecord {
    pub logical_name: Option<String>,
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub bus_info: Option<String>,
    pub driver: Option<String>,
}

pub fn parse_hciconfig(input: &str) -> Vec<BluetoothControllerRecord> {
    let address_re = Regex::new(r"BD Address:\s*([0-9A-Fa-f:]{17})").unwrap();
    let name_re = Regex::new(r"Name:\s*'(.+)'").unwrap();
    let mut records = Vec::new();
    let mut current: Option<BluetoothControllerRecord> = None;
    for line in input.lines() {
        if line.starts_with("hci") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let bus = line.split("Bus:").nth(1).map(|v| v.trim().to_string());
            current = Some(BluetoothControllerRecord {
                bus,
                ..Default::default()
            });
        } else if let Some(record) = current.as_mut() {
            if let Some(caps) = address_re.captures(line) {
                record.address = Some(caps[1].to_string());
            } else if let Some(caps) = name_re.captures(line) {
                record.name = Some(caps[1].to_string());
            } else {
                let flags: Vec<String> = line
                    .split_whitespace()
                    .filter(|v| v.chars().all(|c| c.is_ascii_uppercase()))
                    .map(ToOwned::to_owned)
                    .collect();
                if !flags.is_empty() {
                    record.flags = flags;
                }
            }
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}

pub fn parse_bluetoothctl_paired_devices(input: &str) -> Vec<BluetoothPairedDeviceRecord> {
    input
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("Device ")?;
            let (address, name) = rest.split_once(' ')?;
            Some(BluetoothPairedDeviceRecord {
                address: address.to_string(),
                name: name.to_string(),
            })
        })
        .collect()
}

pub fn parse_lshw_communication(input: &str) -> Vec<LshwCommunicationRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwCommunicationRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-communication") {
            push_lshw_communication_record(&mut records, current.take());
            current = Some(LshwCommunicationRecord::default());
            continue;
        }
        if trimmed.starts_with("*-") {
            push_lshw_communication_record(&mut records, current.take());
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
            "logical name" => record.logical_name = clean_lshw_communication_value(value),
            "product" => record.product = clean_lshw_communication_value(value),
            "vendor" => record.vendor = clean_lshw_communication_value(value),
            "bus info" => record.bus_info = clean_lshw_communication_value(value),
            "configuration" => parse_lshw_communication_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_communication_record(&mut records, current.take());
    records
}

fn push_lshw_communication_record(
    records: &mut Vec<LshwCommunicationRecord>,
    record: Option<LshwCommunicationRecord>,
) {
    if let Some(record) = record {
        if record.logical_name.is_some()
            || record.product.is_some()
            || record.vendor.is_some()
            || record.bus_info.is_some()
        {
            records.push(record);
        }
    }
}

fn parse_lshw_communication_configuration(record: &mut LshwCommunicationRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "driver" {
            record.driver = clean_lshw_communication_value(value);
        }
    }
}

fn clean_lshw_communication_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}
