use hw_model::{Device, SnapshotId, StoredSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryState {
    pub current_snapshot_id: Option<SnapshotId>,
    pub current_machine_bind_id: Option<String>,
    pub bindid_algorithm: Option<String>,
    pub last_configuration_fingerprint: Option<String>,
    pub fingerprint_version: Option<u32>,
    pub last_quick_probe_at: Option<String>,
    pub current_snapshot_created_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    Quick,
    Full,
}

impl ProbeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeCompletion {
    Succeeded,
    Partial,
    Failed,
}

impl ProbeCompletion {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

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

pub const SNAPSHOT_CLI_SCHEMA_VERSION: &str = "qurbrix.hw.snapshot.cli.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangedDevice {
    pub device_id: String,
    pub before: Device,
    pub after: Device,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub schema_version: String,
    pub from_snapshot_id: SnapshotId,
    pub to_snapshot_id: SnapshotId,
    pub machine_identity_changed: bool,
    pub configuration_changed: bool,
    pub added: Vec<Device>,
    pub removed: Vec<Device>,
    pub changed: Vec<ChangedDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub schema_version: String,
    pub snapshot_id: SnapshotId,
    pub output_path: String,
    pub sha256: String,
    pub size_bytes: u64,
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
