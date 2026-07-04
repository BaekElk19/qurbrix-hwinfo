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
