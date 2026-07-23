mod artifact;
pub mod canonical;
pub mod error;
pub mod model;
pub mod probe;
pub mod service;
pub mod store;

pub use canonical::canonicalize_devices;
pub use error::{InventoryError, Result};
pub use model::{
    InventoryState, PageRequest, ProbeCompletion, ProbeKind, StoredDeviceSummary,
    UploadSnapshotProjection,
};
pub use probe::{quick_probe, quick_probe_with_runner, QuickProbeConfig};
pub use service::{
    ensure_snapshot, ensure_snapshot_with_scanner, full_scan, RealSnapshotScanner, SnapshotScanner,
};
pub use store::InventoryStore;
