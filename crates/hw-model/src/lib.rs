use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
}

impl ScanReport {
    pub fn empty() -> Self {
        Self {
            schema_version: "qurbrix.hw.scan.v1".to_string(),
        }
    }
}
