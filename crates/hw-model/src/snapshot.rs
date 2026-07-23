use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr, time::Duration};
use uuid::Uuid;

pub const SNAPSHOT_SCHEMA_VERSION: &str = "qurbrix.hw.snapshot.v1";
pub const FINGERPRINT_VERSION: u32 = 2;
pub const BINDID_V2_ALGORITHM: &str = "qurbrix-hw-bindid-sha256-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(Uuid);

impl SnapshotId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

impl FromStr for SnapshotId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PartialPolicy {
    #[default]
    PublishIfCoreComplete,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsureSnapshotOptions {
    pub force_full_scan: bool,
    pub max_snapshot_age: Option<Duration>,
    pub partial_policy: PartialPolicy,
}

impl Default for EnsureSnapshotOptions {
    fn default() -> Self {
        Self {
            force_full_scan: false,
            max_snapshot_age: Some(Duration::from_secs(24 * 60 * 60)),
            partial_policy: PartialPolicy::PublishIfCoreComplete,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreIdentityGroup {
    Platform,
    Cpu,
    Memory,
    Storage,
    PhysicalNetwork,
    Gpu,
}

impl CoreIdentityGroup {
    pub const REQUIRED: [Self; 5] = [
        Self::Platform,
        Self::Cpu,
        Self::Memory,
        Self::Storage,
        Self::PhysicalNetwork,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityCoverage {
    pub covered: Vec<CoreIdentityGroup>,
    pub missing: Vec<CoreIdentityGroup>,
    pub trusted_absent: Vec<CoreIdentityGroup>,
}

impl IdentityCoverage {
    pub fn core_complete(&self) -> bool {
        CoreIdentityGroup::REQUIRED.iter().all(|group| {
            self.covered.contains(group) || self.trusted_absent.contains(group)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickProbeReport {
    pub schema_version: String,
    pub fingerprint_version: u32,
    pub bindid_algorithm: String,
    pub machine_bind_id: String,
    pub configuration_fingerprint: String,
    pub canonical_payload_sha256: String,
    pub observed_at: String,
    pub identity_records: Vec<String>,
    pub configuration_records: Vec<String>,
    pub coverage: IdentityCoverage,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishedScanStatus {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSnapshot {
    pub snapshot_id: SnapshotId,
    pub machine_bind_id: String,
    pub bindid_algorithm: String,
    pub schema_version: String,
    pub scanner_version: Option<String>,
    pub created_at: String,
    pub scan_status: PublishedScanStatus,
    pub configuration_fingerprint: String,
    pub artifact: ArtifactMetadata,
    pub device_count: u64,
    pub warning_count: u64,
    pub duration_ms: Option<u64>,
    pub pinned: bool,
    pub uploaded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureSnapshotOutcome {
    Reused(SnapshotId),
    Published(SnapshotId),
    Failed { previous: Option<SnapshotId> },
}
