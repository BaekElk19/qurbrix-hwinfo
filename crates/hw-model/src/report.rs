use crate::{Device, DeviceKind, ScanWarning};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const SCHEMA_VERSION: &str = "qurbrix.hw.scan.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
    pub metadata: ScanMetadata,
    pub system: SystemInfo,
    pub devices: Vec<Device>,
    pub warnings: Vec<ScanWarning>,
    pub status: ScanStatus,
}

impl ScanReport {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            metadata: ScanMetadata::default(),
            system: SystemInfo::default(),
            devices: Vec::new(),
            warnings: Vec::new(),
            status: ScanStatus::Complete,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScanMetadata {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
    pub scanner_version: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Complete,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    pub kinds: Option<Vec<DeviceKind>>,
    pub exclude_kinds: Vec<DeviceKind>,
    pub timeout: Duration,
    pub optional_sources: bool,
    pub include_sources: bool,
    pub include_warnings: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            kinds: None,
            exclude_kinds: Vec::new(),
            timeout: Duration::from_secs(30),
            optional_sources: true,
            include_sources: true,
            include_warnings: true,
        }
    }
}
