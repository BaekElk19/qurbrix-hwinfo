use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpLinkRecord {
    pub ifname: String,
    pub address: Option<String>,
    pub operstate: Option<String>,
    pub mtu: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpAddrRecord {
    pub ifname: String,
    #[serde(default)]
    pub addr_info: Vec<IpAddrInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpAddrInfo {
    pub family: Option<String>,
    pub local: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwNetworkRecord {
    pub logical_name: Option<String>,
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub serial: Option<String>,
    pub bus_info: Option<String>,
    pub capacity_mbps: Option<u32>,
    pub driver: Option<String>,
    pub driver_version: Option<String>,
    pub firmware: Option<String>,
}

pub fn parse_ip_j_link(input: &str) -> Vec<IpLinkRecord> {
    parse_ip_j_link_result(input).unwrap_or_default()
}

pub fn parse_ip_j_link_result(input: &str) -> Result<Vec<IpLinkRecord>, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn parse_ip_j_addr_result(input: &str) -> Result<Vec<IpAddrRecord>, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn parse_lshw_network(input: &str) -> Vec<LshwNetworkRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwNetworkRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-network") {
            push_lshw_network_record(&mut records, current.take());
            current = Some(LshwNetworkRecord::default());
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
            "product" => record.product = clean_lshw_network_value(value),
            "vendor" => record.vendor = clean_lshw_network_value(value),
            "serial" => record.serial = clean_lshw_network_value(value),
            "bus info" => record.bus_info = clean_lshw_network_value(value),
            "logical name" => record.logical_name = clean_lshw_network_value(value),
            "capacity" => record.capacity_mbps = parse_network_capacity_mbps(value),
            "configuration" => parse_lshw_network_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_network_record(&mut records, current.take());
    records
}

fn push_lshw_network_record(
    records: &mut Vec<LshwNetworkRecord>,
    record: Option<LshwNetworkRecord>,
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

fn parse_lshw_network_configuration(record: &mut LshwNetworkRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "driver" => record.driver = clean_lshw_network_value(value),
            "driverversion" => record.driver_version = clean_lshw_network_value(value),
            "firmware" => record.firmware = clean_lshw_network_value(value),
            _ => {}
        }
    }
}

fn clean_lshw_network_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_network_capacity_mbps(value: &str) -> Option<u32> {
    let value = value.trim();
    let unit_start = value.find(|ch: char| !ch.is_ascii_digit() && ch != '.')?;
    let number = value[..unit_start].parse::<f64>().ok()?;
    let unit = value[unit_start..].to_ascii_lowercase();
    let mbps = if unit.starts_with("gbit") {
        number * 1000.0
    } else if unit.starts_with("mbit") {
        number
    } else if unit.starts_with("kbit") {
        number / 1000.0
    } else {
        return None;
    };
    (mbps > 0.0).then(|| mbps.round() as u32)
}
