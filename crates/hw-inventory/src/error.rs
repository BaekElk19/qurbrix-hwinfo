use hw_model::SnapshotId;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum InventoryError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("blocking inventory worker failed: {0}")]
    Worker(#[from] tokio::task::JoinError),
    #[error("unsupported database schema version {0}")]
    UnsupportedSchema(i64),
    #[error("invalid artifact path: {0}")]
    InvalidArtifactPath(PathBuf),
    #[error("artifact size mismatch")]
    ArtifactSizeMismatch,
    #[error("artifact SHA-256 mismatch")]
    ArtifactHashMismatch,
    #[error("artifact schema mismatch")]
    ArtifactSchemaMismatch,
    #[error("invalid published report: {0}")]
    InvalidReport(&'static str),
    #[error("stored snapshot identifier is invalid: {0}")]
    InvalidSnapshotId(String),
    #[error("full hardware scan failed")]
    FullScanFailed,
    #[error("partial hardware scan was rejected by policy")]
    PartialRejected,
    #[error("core hardware identity is incomplete")]
    CoreIdentityIncomplete,
    #[error("timed out waiting for the snapshot scan lease")]
    LeaseTimeout,
    #[error("snapshot not found: {0}")]
    SnapshotNotFound(SnapshotId),
    #[error("destination already exists: {0}")]
    DestinationExists(PathBuf),
}

impl InventoryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(_) => "inventory.database",
            Self::Io(_) => "inventory.io",
            Self::Serialization(_) => "inventory.serialization",
            Self::Worker(_) => "inventory.worker",
            Self::UnsupportedSchema(_) => "inventory.schema_unsupported",
            Self::InvalidArtifactPath(_) => "inventory.artifact_path",
            Self::ArtifactSizeMismatch => "inventory.artifact_size",
            Self::ArtifactHashMismatch => "inventory.artifact_hash",
            Self::ArtifactSchemaMismatch => "inventory.artifact_schema",
            Self::InvalidReport(_) => "inventory.report_invalid",
            Self::InvalidSnapshotId(_) => "inventory.snapshot_id",
            Self::FullScanFailed => "inventory.full_scan_failed",
            Self::PartialRejected => "inventory.partial_rejected",
            Self::CoreIdentityIncomplete => "inventory.core_identity_incomplete",
            Self::LeaseTimeout => "inventory.lease_timeout",
            Self::SnapshotNotFound(_) => "inventory.snapshot_not_found",
            Self::DestinationExists(_) => "inventory.destination_exists",
        }
    }
}

pub type Result<T> = std::result::Result<T, InventoryError>;
