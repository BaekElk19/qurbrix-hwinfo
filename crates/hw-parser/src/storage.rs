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
    pub wwn: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SmartctlInfo {
    pub smart_status: Option<String>,
    pub temperature_celsius: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlReport {
    smart_status: Option<SmartctlStatus>,
    temperature: Option<SmartctlTemperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlStatus {
    passed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct SmartctlTemperature {
    current: Option<f32>,
}

pub fn parse_lsblk_json(input: &str) -> Vec<LsblkDevice> {
    parse_lsblk_json_result(input).unwrap_or_default()
}

pub fn parse_lsblk_json_result(input: &str) -> Result<Vec<LsblkDevice>, serde_json::Error> {
    serde_json::from_str::<LsblkReport>(input).map(|report| report.blockdevices)
}

pub fn parse_smartctl_json(input: &str) -> Result<SmartctlInfo, serde_json::Error> {
    serde_json::from_str::<SmartctlReport>(input).map(|report| SmartctlInfo {
        smart_status: report
            .smart_status
            .and_then(|status| status.passed)
            .map(|passed| {
                if passed {
                    "passed".to_string()
                } else {
                    "failed".to_string()
                }
            }),
        temperature_celsius: report
            .temperature
            .and_then(|temperature| temperature.current),
    })
}
