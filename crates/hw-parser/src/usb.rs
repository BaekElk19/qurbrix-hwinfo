use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsbRecord {
    pub bus: Option<String>,
    pub device: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub interface: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub class: Option<String>,
    pub subclass: Option<String>,
    pub protocol: Option<String>,
    pub speed: Option<String>,
}

pub fn parse_lsusb(input: &str) -> Vec<UsbRecord> {
    let re = Regex::new(r"^Bus\s+(?P<bus>\d+)\s+Device\s+(?P<device>\d+):\s+ID\s+(?P<vid>[0-9a-fA-F]{4}):(?P<pid>[0-9a-fA-F]{4})\s*(?P<product>.*)$").unwrap();
    input
        .lines()
        .filter_map(|line| {
            let caps = re.captures(line)?;
            Some(UsbRecord {
                bus: Some(caps["bus"].to_string()),
                device: Some(caps["device"].to_string()),
                vendor_id: Some(caps["vid"].to_ascii_lowercase()),
                product_id: Some(caps["pid"].to_ascii_lowercase()),
                product: Some(caps["product"].trim().to_string()).filter(|v| !v.is_empty()),
                ..Default::default()
            })
        })
        .collect()
}

pub fn parse_lsusb_verbose(input: &str) -> Vec<UsbRecord> {
    let mut records = Vec::new();
    let mut current: Option<UsbRecord> = None;
    let mut in_interface = false;

    for line in input.lines() {
        if let Some(mut record) = parse_lsusb(line).into_iter().next() {
            if let Some(current) = current.take() {
                records.push(current);
            }
            record.product = record.product.filter(|value| !value.is_empty());
            current = Some(record);
            in_interface = false;
            continue;
        }

        let trimmed = line.trim();
        if trimmed == "Interface Descriptor:" {
            in_interface = true;
            continue;
        }
        if !in_interface {
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        if record.class.is_some() && record.subclass.is_some() && record.protocol.is_some() {
            continue;
        }
        if let Some(value) = descriptor_field(trimmed, "bInterfaceNumber") {
            if record.interface.is_none() {
                record.interface = Some(value.to_string());
            }
        } else if let Some(value) = descriptor_byte_field(trimmed, "bInterfaceClass") {
            if record.class.is_none() {
                record.class = Some(value);
            }
        } else if let Some(value) = descriptor_byte_field(trimmed, "bInterfaceSubClass") {
            if record.subclass.is_none() {
                record.subclass = Some(value);
            }
        } else if let Some(value) = descriptor_byte_field(trimmed, "bInterfaceProtocol") {
            if record.protocol.is_none() {
                record.protocol = Some(value);
            }
        }
    }

    if let Some(current) = current {
        records.push(current);
    }
    records
}

fn descriptor_byte_field(line: &str, key: &str) -> Option<String> {
    let value = descriptor_field(line, key)?;
    let byte = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        .or_else(|| value.parse::<u8>().ok())?;
    Some(format!("{byte:02x}"))
}

fn descriptor_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(key)?.split_whitespace().next()
}
