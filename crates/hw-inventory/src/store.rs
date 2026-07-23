use crate::{
    artifact,
    error::{InventoryError, Result},
    model::{PageRequest, StoredDeviceSummary, UploadSnapshotProjection},
};
use hw_model::{
    ArtifactMetadata, BusInfo, Device, PublishedScanStatus, QuickProbeReport, ScanReport,
    ScanStatus, SnapshotId, StoredSnapshot, SNAPSHOT_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");

#[derive(Debug, Clone)]
pub struct InventoryStore {
    state_dir: Arc<PathBuf>,
    db_path: Arc<PathBuf>,
}

impl InventoryStore {
    pub async fn open(state_dir: impl Into<PathBuf>) -> Result<Self> {
        let state_dir = state_dir.into();
        let db_path = state_dir.join("qurbrix_hwinfo.db");
        let store = Self {
            state_dir: Arc::new(state_dir),
            db_path: Arc::new(db_path),
        };
        let cloned = store.clone();
        tokio::task::spawn_blocking(move || cloned.initialize()).await??;
        Ok(store)
    }

    pub fn state_dir(&self) -> &Path {
        self.state_dir.as_path()
    }

    pub fn database_path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub async fn publish_snapshot(
        &self,
        report: ScanReport,
        probe: QuickProbeReport,
    ) -> Result<SnapshotId> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.publish_sync(report, probe)).await?
    }

    pub async fn load_snapshot(&self, snapshot_id: SnapshotId) -> Result<Option<StoredSnapshot>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.load_snapshot_sync(snapshot_id)).await?
    }

    pub async fn load_scan_report(&self, snapshot_id: SnapshotId) -> Result<Option<ScanReport>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.load_report_sync(snapshot_id)).await?
    }

    pub async fn list_snapshots(
        &self,
        machine_bind_id: Option<String>,
        page: PageRequest,
    ) -> Result<Vec<StoredSnapshot>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_snapshots_sync(machine_bind_id, page))
            .await?
    }

    pub async fn list_devices(
        &self,
        snapshot_id: SnapshotId,
        page: PageRequest,
    ) -> Result<Vec<StoredDeviceSummary>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_devices_sync(snapshot_id, page)).await?
    }

    pub async fn upload_projection(
        &self,
        snapshot_id: SnapshotId,
        page: PageRequest,
    ) -> Result<Option<UploadSnapshotProjection>> {
        let snapshot = match self.load_snapshot(snapshot_id).await? {
            Some(snapshot) => snapshot,
            None => return Ok(None),
        };
        let devices = self.list_devices(snapshot_id, page).await?;
        Ok(Some(UploadSnapshotProjection {
            schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
            snapshot,
            devices,
        }))
    }

    pub async fn recover_orphan_artifacts(&self) -> Result<u64> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.recover_orphans_sync()).await?
    }

    fn initialize(&self) -> Result<()> {
        artifact::ensure_private_directory(&self.state_dir)?;
        let connection = self.connect()?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 1 {
            return Err(InventoryError::UnsupportedSchema(version));
        }
        connection.execute_batch(INITIAL_MIGRATION)?;
        connection.execute(
            "INSERT OR IGNORE INTO schema_migration(version, name, applied_at) VALUES (1, 'initial', ?1)",
            [now_rfc3339()?],
        )?;
        connection.pragma_update(None, "user_version", 1)?;
        artifact::ensure_private_file(&self.db_path)?;
        self.recover_orphans_sync()?;
        Ok(())
    }

    fn connect(&self) -> Result<Connection> {
        let connection = Connection::open(self.db_path.as_path())?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(connection)
    }

    fn publish_sync(&self, report: ScanReport, probe: QuickProbeReport) -> Result<SnapshotId> {
        if report.status == ScanStatus::Failed {
            return Err(InventoryError::InvalidReport(
                "failed scans cannot be published",
            ));
        }
        if !probe.coverage.core_complete() {
            return Err(InventoryError::InvalidReport("core identity is incomplete"));
        }
        let snapshot_id = SnapshotId::new_v7();
        let created_at = now_rfc3339()?;
        let artifact_metadata = artifact::write_report(&self.state_dir, snapshot_id, &report)?;
        let publish_result = self.publish_transaction(
            snapshot_id,
            &created_at,
            &report,
            &probe,
            &artifact_metadata,
        );
        if publish_result.is_err() {
            let _ = artifact::remove_report(&self.state_dir, &artifact_metadata.relative_path);
        }
        publish_result?;
        Ok(snapshot_id)
    }

    fn publish_transaction(
        &self,
        snapshot_id: SnapshotId,
        created_at: &str,
        report: &ScanReport,
        probe: &QuickProbeReport,
        artifact_metadata: &ArtifactMetadata,
    ) -> Result<()> {
        let mut connection = self.connect()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let status = match report.status {
            ScanStatus::Complete => "complete",
            ScanStatus::Partial => "partial",
            ScanStatus::Failed => unreachable!(),
        };
        transaction.execute(
            "INSERT INTO hardware_snapshot(snapshot_id, created_at, scan_status, schema_version, scanner_version, machine_bind_id, bindid_algorithm, configuration_fingerprint, device_count, warning_count, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                snapshot_id.to_string(),
                created_at,
                status,
                report.schema_version,
                report.metadata.scanner_version,
                probe.machine_bind_id,
                probe.bindid_algorithm,
                probe.configuration_fingerprint,
                report.devices.len() as i64,
                report.warnings.len() as i64,
                report.metadata.duration_ms.map(|value| value as i64),
            ],
        )?;
        for (ordinal, device) in report.devices.iter().enumerate() {
            insert_device(&transaction, snapshot_id, device, ordinal)?;
        }
        for (ordinal, device) in report.devices.iter().enumerate() {
            insert_device_details(&transaction, snapshot_id, device, ordinal)?;
        }
        for (ordinal, warning) in report.warnings.iter().enumerate() {
            transaction.execute(
                "INSERT INTO snapshot_warning(snapshot_id, device_id, code, message, source, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![snapshot_id.to_string(), warning.device_id, warning.code, warning.message, warning.source, ordinal as i64],
            )?;
        }
        transaction.execute(
            "INSERT INTO snapshot_artifact(snapshot_id, artifact_kind, relative_path, sha256, size_bytes, schema_version, created_at) VALUES (?1, 'scan_report_json', ?2, ?3, ?4, ?5, ?6)",
            params![snapshot_id.to_string(), artifact_metadata.relative_path, artifact_metadata.sha256, artifact_metadata.size_bytes as i64, artifact_metadata.schema_version, created_at],
        )?;
        transaction.execute(
            "INSERT INTO snapshot_lifecycle(snapshot_id, pinned, uploaded_at, delete_pending, updated_at) VALUES (?1, 0, NULL, 0, ?2)",
            params![snapshot_id.to_string(), created_at],
        )?;
        transaction.execute(
            "UPDATE inventory_state SET current_snapshot_id = ?1, current_machine_bind_id = ?2, bindid_algorithm = ?3, last_configuration_fingerprint = ?4, core_identity_count = ?5, fingerprint_version = ?6, last_quick_probe_at = ?7, updated_at = ?7 WHERE id = 1",
            params![snapshot_id.to_string(), probe.machine_bind_id, probe.bindid_algorithm, probe.configuration_fingerprint, probe.identity_records.len() as i64, probe.fingerprint_version as i64, probe.observed_at],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn load_snapshot_sync(&self, snapshot_id: SnapshotId) -> Result<Option<StoredSnapshot>> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT h.snapshot_id, h.machine_bind_id, h.bindid_algorithm, h.schema_version, h.scanner_version, h.created_at, h.scan_status, h.configuration_fingerprint, a.relative_path, a.sha256, a.size_bytes, a.schema_version, h.device_count, h.warning_count, h.duration_ms, l.pinned, l.uploaded_at FROM hardware_snapshot h JOIN snapshot_artifact a USING(snapshot_id) JOIN snapshot_lifecycle l USING(snapshot_id) WHERE h.snapshot_id = ?1",
                [snapshot_id.to_string()],
                stored_snapshot_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn load_report_sync(&self, snapshot_id: SnapshotId) -> Result<Option<ScanReport>> {
        let Some(snapshot) = self.load_snapshot_sync(snapshot_id)? else {
            return Ok(None);
        };
        artifact::read_report(&self.state_dir, &snapshot.artifact).map(Some)
    }

    fn list_snapshots_sync(
        &self,
        machine_bind_id: Option<String>,
        page: PageRequest,
    ) -> Result<Vec<StoredSnapshot>> {
        let connection = self.connect()?;
        let base = "SELECT h.snapshot_id, h.machine_bind_id, h.bindid_algorithm, h.schema_version, h.scanner_version, h.created_at, h.scan_status, h.configuration_fingerprint, a.relative_path, a.sha256, a.size_bytes, a.schema_version, h.device_count, h.warning_count, h.duration_ms, l.pinned, l.uploaded_at FROM hardware_snapshot h JOIN snapshot_artifact a USING(snapshot_id) JOIN snapshot_lifecycle l USING(snapshot_id)";
        let mut snapshots = Vec::new();
        if let Some(machine_bind_id) = machine_bind_id {
            let sql = format!("{base} WHERE h.machine_bind_id = ?1 ORDER BY h.created_at DESC, h.snapshot_id DESC LIMIT ?2 OFFSET ?3");
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params![machine_bind_id, page.bounded_limit(), page.offset],
                stored_snapshot_from_row,
            )?;
            for row in rows {
                snapshots.push(row?);
            }
        } else {
            let sql =
                format!("{base} ORDER BY h.created_at DESC, h.snapshot_id DESC LIMIT ?1 OFFSET ?2");
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params![page.bounded_limit(), page.offset],
                stored_snapshot_from_row,
            )?;
            for row in rows {
                snapshots.push(row?);
            }
        }
        Ok(snapshots)
    }

    fn list_devices_sync(
        &self,
        snapshot_id: SnapshotId,
        page: PageRequest,
    ) -> Result<Vec<StoredDeviceSummary>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT snapshot_id, device_id, kind, name, vendor, model, serial, bus_kind, bus_address, driver_name, driver_status, parent_device_id, ordinal FROM snapshot_device WHERE snapshot_id = ?1 ORDER BY ordinal, device_id LIMIT ?2 OFFSET ?3",
        )?;
        let rows = statement.query_map(
            params![snapshot_id.to_string(), page.bounded_limit(), page.offset],
            stored_device_from_row,
        )?;
        let mut devices = Vec::new();
        for row in rows {
            devices.push(row?);
        }
        Ok(devices)
    }

    fn recover_orphans_sync(&self) -> Result<u64> {
        let connection = self.connect()?;
        let mut statement = connection.prepare("SELECT relative_path FROM snapshot_artifact")?;
        let paths = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        artifact::recover_orphans(&self.state_dir, &paths)
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| InventoryError::InvalidReport("UTC timestamp formatting failed"))
}

fn stored_snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSnapshot> {
    let id: String = row.get(0)?;
    let status: String = row.get(6)?;
    Ok(StoredSnapshot {
        snapshot_id: SnapshotId::from_str(&id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        machine_bind_id: row.get(1)?,
        bindid_algorithm: row.get(2)?,
        schema_version: row.get(3)?,
        scanner_version: row.get(4)?,
        created_at: row.get(5)?,
        scan_status: if status == "complete" {
            PublishedScanStatus::Complete
        } else {
            PublishedScanStatus::Partial
        },
        configuration_fingerprint: row.get(7)?,
        artifact: ArtifactMetadata {
            relative_path: row.get(8)?,
            sha256: row.get(9)?,
            size_bytes: row.get::<_, i64>(10)? as u64,
            schema_version: row.get(11)?,
        },
        device_count: row.get::<_, i64>(12)? as u64,
        warning_count: row.get::<_, i64>(13)? as u64,
        duration_ms: row.get::<_, Option<i64>>(14)?.map(|value| value as u64),
        pinned: row.get::<_, i64>(15)? != 0,
        uploaded_at: row.get(16)?,
    })
}

fn stored_device_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDeviceSummary> {
    let snapshot_id: String = row.get(0)?;
    Ok(StoredDeviceSummary {
        snapshot_id: SnapshotId::from_str(&snapshot_id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        device_id: row.get(1)?,
        kind: row.get(2)?,
        name: row.get(3)?,
        vendor: row.get(4)?,
        model: row.get(5)?,
        serial: row.get(6)?,
        bus_kind: row.get(7)?,
        bus_address: row.get(8)?,
        driver_name: row.get(9)?,
        driver_status: row.get(10)?,
        parent_device_id: row.get(11)?,
        ordinal: row.get::<_, i64>(12)? as u64,
    })
}

fn insert_device(
    transaction: &Transaction<'_>,
    snapshot_id: SnapshotId,
    device: &Device,
    ordinal: usize,
) -> Result<()> {
    let (bus_kind, bus_address) = bus_projection(device.bus.as_ref());
    let driver_status = device
        .driver
        .as_ref()
        .map(|driver| format!("{:?}", driver.status).to_ascii_lowercase());
    transaction
        .prepare_cached(
            "INSERT INTO snapshot_device(snapshot_id, device_id, kind, name, vendor, model, serial, bus_kind, bus_address, driver_name, driver_status, parent_device_id, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?
        .execute(params![snapshot_id.to_string(), device.id, device.kind.to_string(), device.name, device.vendor, device.model, device.serial, bus_kind, bus_address, device.driver.as_ref().and_then(|driver| driver.name.as_deref()), driver_status, device.parent_id, ordinal as i64])?;
    Ok(())
}

fn insert_device_details(
    transaction: &Transaction<'_>,
    snapshot_id: SnapshotId,
    device: &Device,
    _device_ordinal: usize,
) -> Result<()> {
    for (ordinal, identifier) in device.identifiers.iter().enumerate() {
        transaction
            .prepare_cached(
                "INSERT OR IGNORE INTO snapshot_device_identifier(snapshot_id, device_id, identifier_kind, identifier_value, ordinal) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?
            .execute(params![snapshot_id.to_string(), device.id, identifier.kind, identifier.value, ordinal as i64])?;
    }
    for (ordinal, child) in device.children.iter().enumerate() {
        transaction
            .prepare_cached(
                "INSERT OR IGNORE INTO snapshot_device_relation(snapshot_id, source_device_id, relation_kind, target_device_id, ordinal) VALUES (?1, ?2, 'child', ?3, ?4)",
            )?
            .execute(params![snapshot_id.to_string(), device.id, child, ordinal as i64])?;
    }
    let property_value = serde_json::to_value(&device.properties)?;
    let mut properties = Vec::new();
    flatten_properties("properties", &property_value, &mut properties);
    for property in properties {
        insert_property(transaction, snapshot_id, &device.id, property)?;
    }
    for (ordinal, source) in device.sources.iter().enumerate() {
        transaction
            .prepare_cached(
                "INSERT INTO snapshot_source(snapshot_id, device_id, source, source_kind, source_status, summary, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?
            .execute(params![snapshot_id.to_string(), device.id, source.source, format!("{:?}", source.kind).to_ascii_lowercase(), format!("{:?}", source.status).to_ascii_lowercase(), source.summary, ordinal as i64])?;
    }
    Ok(())
}

#[derive(Debug)]
struct FlatProperty {
    key: String,
    value: Value,
    ordinal: usize,
}

fn flatten_properties(prefix: &str, value: &Value, output: &mut Vec<FlatProperty>) {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                let child = format!("{prefix}.{key}");
                flatten_properties(&child, value, output);
            }
        }
        Value::Array(values) => {
            for (ordinal, value) in values.iter().enumerate() {
                match value {
                    Value::Object(_) | Value::Array(_) => {
                        flatten_properties(&format!("{prefix}.{ordinal}"), value, output)
                    }
                    Value::Null => {}
                    _ => output.push(FlatProperty {
                        key: prefix.to_string(),
                        value: value.clone(),
                        ordinal,
                    }),
                }
            }
        }
        Value::Null => {}
        _ => output.push(FlatProperty {
            key: prefix.to_string(),
            value: value.clone(),
            ordinal: 0,
        }),
    }
}

fn insert_property(
    transaction: &Transaction<'_>,
    snapshot_id: SnapshotId,
    device_id: &str,
    property: FlatProperty,
) -> Result<()> {
    let unit = infer_unit(&property.key);
    let (value_type, text, integer, real, boolean) = match property.value {
        Value::String(value) => ("text", Some(value), None, None, None),
        Value::Bool(value) => ("boolean", None, None, None, Some(i64::from(value))),
        Value::Number(value) if value.as_i64().is_some() => {
            ("integer", None, value.as_i64(), None, None)
        }
        Value::Number(value) if value.as_u64().is_some_and(|value| value <= i64::MAX as u64) => (
            "integer",
            None,
            value.as_u64().map(|value| value as i64),
            None,
            None,
        ),
        Value::Number(value) => ("real", None, None, value.as_f64(), None),
        _ => return Ok(()),
    };
    transaction
        .prepare_cached(
            "INSERT OR REPLACE INTO snapshot_device_property(snapshot_id, device_id, property_key, value_type, text_value, integer_value, real_value, boolean_value, unit, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?
        .execute(params![snapshot_id.to_string(), device_id, property.key, value_type, text, integer, real, boolean, unit, property.ordinal as i64])?;
    Ok(())
}

fn infer_unit(key: &str) -> Option<&'static str> {
    if key.ends_with("_bytes") || key.ends_with(".size_bytes") {
        Some("bytes")
    } else if key.ends_with("_mhz") {
        Some("mhz")
    } else if key.ends_with("_mtps") {
        Some("mtps")
    } else if key.ends_with("_celsius") {
        Some("celsius")
    } else {
        None
    }
}

fn bus_projection(bus: Option<&BusInfo>) -> (Option<&'static str>, Option<String>) {
    match bus {
        Some(BusInfo::Pci { address, .. }) => (Some("pci"), Some(address.clone())),
        Some(BusInfo::Usb { bus, device, .. }) => (
            Some("usb"),
            Some(format!(
                "{}:{}",
                bus.as_deref().unwrap_or(""),
                device.as_deref().unwrap_or("")
            )),
        ),
        Some(BusInfo::Platform { path }) => (Some("platform"), Some(path.clone())),
        Some(BusInfo::Virtual) => (Some("virtual"), None),
        Some(BusInfo::Unknown) => (Some("unknown"), None),
        None => (None, None),
    }
}
