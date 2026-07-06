use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InputRecord {
    pub bus: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub version: Option<String>,
    pub name: Option<String>,
    pub phys: Option<String>,
    pub uniq: Option<String>,
    pub handlers: Vec<String>,
    pub capabilities: InputCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InputCapabilities {
    pub ev: Option<String>,
    pub key: Option<String>,
    pub rel: Option<String>,
    pub abs: Option<String>,
    pub properties: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HwinfoInputRecord {
    pub input_kind: HwinfoInputKind,
    pub event_node: Option<String>,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub device: Option<String>,
    pub driver: Option<String>,
    pub driver_modules: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HwinfoInputKind {
    Keyboard,
    Mouse,
    #[default]
    UnknownInput,
}

pub fn parse_proc_bus_input_devices(input: &str) -> Vec<InputRecord> {
    let id_re = Regex::new(r"Bus=(\S+)\s+Vendor=(\S+)\s+Product=(\S+)\s+Version=(\S+)").unwrap();
    let mut records = Vec::new();
    let mut current = InputRecord::default();
    let mut seen = false;

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if seen {
                records.push(current);
                current = InputRecord::default();
                seen = false;
            }
            continue;
        }
        seen = true;
        if let Some(rest) = line.strip_prefix("I: ") {
            if let Some(caps) = id_re.captures(rest) {
                current.bus = Some(caps[1].to_string());
                current.vendor_id = Some(caps[2].to_ascii_lowercase());
                current.product_id = Some(caps[3].to_ascii_lowercase());
                current.version = Some(caps[4].to_string());
            }
        } else if let Some(rest) = line.strip_prefix("N: Name=") {
            current.name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = line.strip_prefix("P: Phys=") {
            current.phys = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("U: Uniq=") {
            current.uniq = Some(rest.to_string()).filter(|v| !v.is_empty());
        } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
            current.handlers = rest.split_whitespace().map(ToOwned::to_owned).collect();
        } else if let Some(rest) = line.strip_prefix("B: ") {
            let Some((name, value)) = rest.split_once('=') else {
                continue;
            };
            let value = value.trim().to_string();
            match name {
                "EV" => current.capabilities.ev = Some(value),
                "KEY" => current.capabilities.key = Some(value),
                "REL" => current.capabilities.rel = Some(value),
                "ABS" => current.capabilities.abs = Some(value),
                "PROP" => current.capabilities.properties = Some(value),
                _ => {}
            }
        }
    }
    records
}

pub fn parse_hwinfo_input(input: &str) -> Vec<HwinfoInputRecord> {
    let mut records = Vec::new();
    let mut section = Vec::new();

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            push_hwinfo_input_record(&mut records, parse_hwinfo_input_section(&section));
            section.clear();
            continue;
        }
        section.push(line);
    }

    records
}

fn parse_hwinfo_input_section(lines: &[&str]) -> Option<HwinfoInputRecord> {
    let mut record = HwinfoInputRecord::default();

    for line in lines {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Hardware Class" => {
                record.input_kind = match value {
                    "keyboard" => HwinfoInputKind::Keyboard,
                    "mouse" => HwinfoInputKind::Mouse,
                    _ => HwinfoInputKind::UnknownInput,
                };
            }
            "Model" => record.model = clean_hwinfo_input_value(value),
            "Vendor" | "SubVendor" => record.vendor = clean_hwinfo_input_value(value),
            "Device" => record.device = clean_hwinfo_input_value(value),
            "Driver" => record.driver = clean_hwinfo_input_value(value),
            "Driver Modules" => record.driver_modules = clean_hwinfo_input_modules(value),
            "Device File" | "Device Files" => {
                if record.event_node.is_none() {
                    record.event_node = hwinfo_input_event_node(value);
                }
            }
            _ => {}
        }
    }

    (record.input_kind != HwinfoInputKind::UnknownInput).then_some(record)
}

fn push_hwinfo_input_record(
    records: &mut Vec<HwinfoInputRecord>,
    record: Option<HwinfoInputRecord>,
) {
    if let Some(record) = record {
        if record.event_node.is_some()
            || record.model.is_some()
            || record.vendor.is_some()
            || record.device.is_some()
            || record.driver.is_some()
            || !record.driver_modules.is_empty()
        {
            records.push(record);
        }
    }
}

fn hwinfo_input_event_node(value: &str) -> Option<String> {
    value
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .map(str::trim)
        .find(|part| {
            part.strip_prefix("/dev/input/event").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
            })
        })
        .map(ToString::to_string)
}

fn clean_hwinfo_input_value(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.split('"').nth(1).unwrap_or(value).trim();
    if value.is_empty() || value.contains("unknown") {
        None
    } else {
        Some(value.to_string())
    }
}

fn clean_hwinfo_input_modules(value: &str) -> Vec<String> {
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
