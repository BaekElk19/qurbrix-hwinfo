use hw_model::{SnapshotId, StoredSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredDeviceSummary {
    pub snapshot_id: SnapshotId,
    pub device_id: String,
    pub kind: String,
    pub name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub bus_kind: Option<String>,
    pub bus_address: Option<String>,
    pub driver_name: Option<String>,
    pub driver_status: Option<String>,
    pub parent_device_id: Option<String>,
    pub ordinal: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadSnapshotProjection {
    pub schema_version: String,
    pub snapshot: StoredSnapshot,
    pub devices: Vec<StoredDeviceSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u64,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

impl PageRequest {
    pub(crate) fn bounded_limit(self) -> u32 {
        self.limit.clamp(1, 1_000)
    }
}
