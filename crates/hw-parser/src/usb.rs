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
    pub max_power_ma: Option<u32>,
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

        let Some(record) = current.as_mut() else {
            continue;
        };
        if !in_interface {
            if let Some(value) = descriptor_string_field(trimmed, "iManufacturer") {
                record.manufacturer = Some(value);
            } else if let Some(value) = descriptor_string_field(trimmed, "iProduct") {
                record.product = Some(value);
            } else if let Some(value) = descriptor_string_field(trimmed, "iSerial") {
                record.serial = Some(value);
            } else if let Some(value) = descriptor_field(trimmed, "MaxPower") {
                record.max_power_ma = parse_max_power_ma(value);
            } else if let Some(value) = trimmed.strip_prefix("Negotiated speed:") {
                record.speed = parse_negotiated_speed_mbps(value);
            }
        }
        if !in_interface {
            continue;
        }

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

fn descriptor_string_field(line: &str, key: &str) -> Option<String> {
    let mut parts = line.strip_prefix(key)?.split_whitespace();
    let index = parts.next()?;
    if index == "0" {
        return None;
    }
    let value = parts.collect::<Vec<_>>().join(" ");
    (!value.is_empty()).then_some(value)
}

fn parse_max_power_ma(value: &str) -> Option<u32> {
    value.trim().trim_end_matches("mA").trim().parse().ok()
}

fn parse_negotiated_speed_mbps(value: &str) -> Option<String> {
    let speed_re = Regex::new(r"(?i)([0-9]+(?:\.[0-9]+)?)\s*([gmk]?)bps").unwrap();
    let captures = speed_re.captures(value)?;
    let number = captures.get(1)?.as_str().parse::<f64>().ok()?;
    let multiplier = match captures
        .get(2)
        .map(|unit| unit.as_str().to_ascii_lowercase())
        .as_deref()
    {
        Some("g") => 1000.0,
        Some("k") => 0.001,
        _ => 1.0,
    };
    let mbps = number * multiplier;
    if mbps.fract() == 0.0 {
        Some(format!("{mbps:.0}"))
    } else {
        Some(mbps.to_string())
    }
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
