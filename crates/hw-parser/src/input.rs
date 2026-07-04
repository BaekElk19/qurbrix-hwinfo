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
        }
    }
    records
}
