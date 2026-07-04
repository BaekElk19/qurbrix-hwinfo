use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub modules: Vec<String>,
    pub provider: Option<String>,
    pub status: DriverStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriverStatus {
    InUse,
    Available,
    Missing,
    Unknown,
}
