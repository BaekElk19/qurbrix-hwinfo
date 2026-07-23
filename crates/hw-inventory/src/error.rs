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
}

pub type Result<T> = std::result::Result<T, InventoryError>;
