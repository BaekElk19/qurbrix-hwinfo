use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsbRecord {
    pub bus: Option<String>,
    pub device: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
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
