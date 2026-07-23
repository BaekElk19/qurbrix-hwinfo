mod artifact;
pub mod canonical;
pub mod diff;
pub mod error;
pub mod maintenance;
pub mod model;
pub mod probe;
pub mod service;
pub mod store;

pub use canonical::canonicalize_devices;
pub use diff::diff_snapshots;
pub use error::{InventoryError, Result};
pub use model::{
    ChangedDevice, ExportMetadata, InventoryHealth, InventoryMetrics, InventoryState, PageRequest,
    ProbeCompletion, ProbeKind, RetentionPolicy, RetentionReport, SnapshotDiff,
    StoredDeviceSummary, UploadSnapshotProjection, WalCheckpointResult,
    SNAPSHOT_CLI_SCHEMA_VERSION,
};
pub use probe::{quick_probe, quick_probe_with_runner, QuickProbeConfig};
pub use service::{
    ensure_snapshot, ensure_snapshot_with_scanner, full_scan, RealSnapshotScanner, SnapshotScanner,
};
pub use store::InventoryStore;
