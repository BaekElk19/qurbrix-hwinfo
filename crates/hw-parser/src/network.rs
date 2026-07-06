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

pub fn parse_ip_j_link(input: &str) -> Vec<IpLinkRecord> {
    parse_ip_j_link_result(input).unwrap_or_default()
}

pub fn parse_ip_j_link_result(input: &str) -> Result<Vec<IpLinkRecord>, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn parse_ip_j_addr_result(input: &str) -> Result<Vec<IpAddrRecord>, serde_json::Error> {
    serde_json::from_str(input)
}
