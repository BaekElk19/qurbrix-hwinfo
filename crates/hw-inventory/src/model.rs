use hw_model::{Device, SnapshotId, StoredSnapshot};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_recent_per_machine: u32,
    pub uploaded_max_age: Duration,
    pub dry_run: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_recent_per_machine: 30,
            uploaded_max_age: Duration::from_secs(90 * 24 * 60 * 60),
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionReport {
    pub schema_version: String,
    pub examined: u64,
    pub protected_current: u64,
    pub protected_pinned: u64,
    pub protected_unuploaded: u64,
    pub protected_recent: u64,
    pub eligible: u64,
    pub database_deleted: u64,
    pub artifacts_deleted: u64,
    pub artifact_delete_failures: u64,
    pub pending_artifact_deletes: u64,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalCheckpointResult {
    pub busy: u64,
    pub log_frames: u64,
    pub checkpointed_frames: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryMetrics {
    pub schema_version: String,
    pub snapshot_count: u64,
    pub device_count: u64,
    pub artifact_bytes: u64,
    pub probe_count: u64,
    pub failed_probe_count: u64,
    pub running_probe_count: u64,
    pub average_probe_duration_ms: Option<f64>,
    pub pending_artifact_deletes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryHealth {
    pub schema_version: String,
    pub healthy: bool,
    pub sqlite_integrity: String,
    pub foreign_key_violations: u64,
    pub missing_artifacts: u64,
    pub corrupt_artifacts: u64,
    pub orphan_artifacts: u64,
    pub metrics: InventoryMetrics,
    pub wal_checkpoint: WalCheckpointResult,
}
