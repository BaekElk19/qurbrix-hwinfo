use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsblkReport {
    #[serde(default)]
    pub blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsblkDevice {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub size: Option<u64>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub tran: Option<String>,
}

pub fn parse_lsblk_json(input: &str) -> Vec<LsblkDevice> {
    serde_json::from_str::<LsblkReport>(input)
        .map(|report| report.blockdevices)
        .unwrap_or_default()
}
