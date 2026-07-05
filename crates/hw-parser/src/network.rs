use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpLinkRecord {
    pub ifname: String,
    pub address: Option<String>,
    pub operstate: Option<String>,
    pub mtu: Option<u32>,
}

pub fn parse_ip_j_link(input: &str) -> Vec<IpLinkRecord> {
    serde_json::from_str(input).unwrap_or_default()
}
