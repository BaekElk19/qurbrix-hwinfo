use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpLinkRecord {
    pub ifname: String,
    pub address: Option<String>,
    pub operstate: Option<String>,
    pub mtu: Option<u32>,
}

pub fn parse_ip_j_link(input: &str) -> Vec<IpLinkRecord> {
    parse_ip_j_link_result(input).unwrap_or_default()
}

pub fn parse_ip_j_link_result(input: &str) -> Result<Vec<IpLinkRecord>, serde_json::Error> {
    serde_json::from_str(input)
}
