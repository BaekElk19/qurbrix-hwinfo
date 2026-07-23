use crate::{
    artifact,
    error::{InventoryError, Result},
    model::{
        InventoryHealth, InventoryMetrics, RetentionPolicy, RetentionReport, WalCheckpointResult,
        SNAPSHOT_CLI_SCHEMA_VERSION,
    },
    InventoryStore,
};
use hw_model::{ArtifactMetadata, SnapshotId};
use rusqlite::params;
use std::{collections::HashMap, str::FromStr};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug)]
struct RetentionCandidate {
    snapshot_id: SnapshotId,
    machine_bind_id: String,
    created_at: String,
    pinned: bool,
    uploaded_at: Option<String>,
    relative_path: String,
    sha256: String,
}

impl InventoryStore {
    pub async fn set_pinned(&self, snapshot_id: SnapshotId, pinned: bool) -> Result<()> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.set_pinned_sync(snapshot_id, pinned)).await?
    }

    pub async fn mark_uploaded(
        &self,
        snapshot_id: SnapshotId,
        uploaded_at: Option<String>,
    ) -> Result<()> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.mark_uploaded_sync(snapshot_id, uploaded_at))
            .await?
    }

    pub async fn apply_retention(&self, policy: RetentionPolicy) -> Result<RetentionReport> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.apply_retention_sync(policy)).await?
    }

    pub async fn metrics(&self) -> Result<InventoryMetrics> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.metrics_sync()).await?
    }

    pub async fn health_check(&self) -> Result<InventoryHealth> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.health_check_sync()).await?
    }

    pub async fn wal_checkpoint(&self) -> Result<WalCheckpointResult> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.wal_checkpoint_sync()).await?
    }

    fn set_pinned_sync(&self, snapshot_id: SnapshotId, pinned: bool) -> Result<()> {
        let changed = self.connect()?.execute(
            "UPDATE snapshot_lifecycle SET pinned = ?1, updated_at = ?2 WHERE snapshot_id = ?3",
            params![i64::from(pinned), now()?, snapshot_id.to_string()],
        )?;
        if changed == 0 {
            return Err(InventoryError::SnapshotNotFound(snapshot_id));
        }
        Ok(())
    }

    fn mark_uploaded_sync(
        &self,
        snapshot_id: SnapshotId,
        uploaded_at: Option<String>,
    ) -> Result<()> {
        let uploaded_at = match uploaded_at {
            Some(uploaded_at) => {
                OffsetDateTime::parse(&uploaded_at, &Rfc3339)
                    .map_err(|_| InventoryError::InvalidReport("uploaded_at must be RFC 3339"))?;
                uploaded_at
            }
            None => now()?,
        };
        let changed = self.connect()?.execute(
            "UPDATE snapshot_lifecycle SET uploaded_at = ?1, updated_at = ?1 WHERE snapshot_id = ?2",
            params![uploaded_at, snapshot_id.to_string()],
        )?;
        if changed == 0 {
            return Err(InventoryError::SnapshotNotFound(snapshot_id));
        }
        Ok(())
    }

    fn apply_retention_sync(&self, policy: RetentionPolicy) -> Result<RetentionReport> {
        let mut report = RetentionReport {
            schema_version: SNAPSHOT_CLI_SCHEMA_VERSION.to_string(),
            examined: 0,
            protected_current: 0,
            protected_pinned: 0,
            protected_unuploaded: 0,
            protected_recent: 0,
            eligible: 0,
            database_deleted: 0,
            artifacts_deleted: 0,
            artifact_delete_failures: 0,
            pending_artifact_deletes: 0,
            dry_run: policy.dry_run,
        };
        if !policy.dry_run {
            self.process_delete_queue(&mut report)?;
        }
        let mut connection = self.connect()?;
        let current: Option<String> = connection.query_row(
            "SELECT current_snapshot_id FROM inventory_state WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT h.snapshot_id, h.machine_bind_id, h.created_at, l.pinned, l.uploaded_at, a.relative_path, a.sha256 FROM hardware_snapshot h JOIN snapshot_lifecycle l USING(snapshot_id) JOIN snapshot_artifact a USING(snapshot_id) ORDER BY h.machine_bind_id, h.created_at DESC, h.snapshot_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let id: String = row.get(0)?;
            Ok(RetentionCandidate {
                snapshot_id: SnapshotId::from_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                machine_bind_id: row.get(1)?,
                created_at: row.get(2)?,
                pinned: row.get::<_, i64>(3)? != 0,
                uploaded_at: row.get(4)?,
                relative_path: row.get(5)?,
                sha256: row.get(6)?,
            })
        })?;
        let candidates = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        report.examined = candidates.len() as u64;
        let cutoff = OffsetDateTime::now_utc()
            - time::Duration::seconds(
                i64::try_from(policy.uploaded_max_age.as_secs()).unwrap_or(i64::MAX),
            );
        let mut ranks = HashMap::<String, u32>::new();
        let mut eligible = Vec::new();
        for candidate in candidates {
            let rank = ranks.entry(candidate.machine_bind_id.clone()).or_default();
            *rank += 1;
            if current.as_deref() == Some(&candidate.snapshot_id.to_string()) {
                report.protected_current += 1;
            } else if candidate.pinned {
                report.protected_pinned += 1;
            } else if candidate.uploaded_at.is_none() {
                report.protected_unuploaded += 1;
            } else if *rank <= policy.keep_recent_per_machine {
                report.protected_recent += 1;
            } else if OffsetDateTime::parse(&candidate.created_at, &Rfc3339)
                .is_ok_and(|created_at| created_at <= cutoff)
            {
                report.eligible += 1;
                eligible.push(candidate);
            } else {
                report.protected_recent += 1;
            }
        }
        if policy.dry_run || eligible.is_empty() {
            report.pending_artifact_deletes = self.pending_delete_count()?;
            return Ok(report);
        }
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for candidate in &eligible {
            transaction.execute(
                "INSERT OR REPLACE INTO artifact_delete_queue(relative_path, sha256, attempts, last_error, updated_at) VALUES (?1, ?2, 0, NULL, ?3)",
                params![candidate.relative_path, candidate.sha256, now()?],
            )?;
            transaction.execute(
                "UPDATE probe_history SET snapshot_id = NULL WHERE snapshot_id = ?1",
                [candidate.snapshot_id.to_string()],
            )?;
            transaction.execute(
                "UPDATE probe_history SET previous_snapshot_id = NULL WHERE previous_snapshot_id = ?1",
                [candidate.snapshot_id.to_string()],
            )?;
            transaction.execute(
                "DELETE FROM hardware_snapshot WHERE snapshot_id = ?1",
                [candidate.snapshot_id.to_string()],
            )?;
        }
        transaction.commit()?;
        report.database_deleted = eligible.len() as u64;
        self.process_delete_queue(&mut report)?;
        report.pending_artifact_deletes = self.pending_delete_count()?;
        Ok(report)
    }

    fn process_delete_queue(&self, report: &mut RetentionReport) -> Result<()> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT relative_path FROM artifact_delete_queue ORDER BY updated_at, relative_path",
        )?;
        let paths = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        for path in paths {
            match artifact::remove_report(self.state_dir(), &path) {
                Ok(()) => {
                    connection.execute(
                        "DELETE FROM artifact_delete_queue WHERE relative_path = ?1",
                        [&path],
                    )?;
                    report.artifacts_deleted += 1;
                }
                Err(error) => {
                    connection.execute(
                        "UPDATE artifact_delete_queue SET attempts = attempts + 1, last_error = ?1, updated_at = ?2 WHERE relative_path = ?3",
                        params![error.code(), now()?, path],
                    )?;
                    report.artifact_delete_failures += 1;
                }
            }
        }
        Ok(())
    }

    fn pending_delete_count(&self) -> Result<u64> {
        self.connect()?
            .query_row("SELECT count(*) FROM artifact_delete_queue", [], |row| {
                row.get::<_, i64>(0).map(|value| value as u64)
            })
            .map_err(Into::into)
    }

    fn metrics_sync(&self) -> Result<InventoryMetrics> {
        let connection = self.connect()?;
        Ok(InventoryMetrics {
            schema_version: SNAPSHOT_CLI_SCHEMA_VERSION.to_string(),
            snapshot_count: count(&connection, "hardware_snapshot")?,
            device_count: count(&connection, "snapshot_device")?,
            artifact_bytes: connection.query_row(
                "SELECT COALESCE(sum(size_bytes), 0) FROM snapshot_artifact",
                [],
                |row| row.get::<_, i64>(0).map(|value| value as u64),
            )?,
            probe_count: count(&connection, "probe_history")?,
            failed_probe_count: connection.query_row(
                "SELECT count(*) FROM probe_history WHERE status = 'failed'",
                [],
                |row| row.get::<_, i64>(0).map(|value| value as u64),
            )?,
            running_probe_count: connection.query_row(
                "SELECT count(*) FROM probe_history WHERE status = 'running'",
                [],
                |row| row.get::<_, i64>(0).map(|value| value as u64),
            )?,
            average_probe_duration_ms: connection.query_row(
                "SELECT avg(duration_ms) FROM probe_history WHERE duration_ms IS NOT NULL",
                [],
                |row| row.get(0),
            )?,
            pending_artifact_deletes: count(&connection, "artifact_delete_queue")?,
        })
    }

    fn health_check_sync(&self) -> Result<InventoryHealth> {
        let connection = self.connect()?;
        let sqlite_integrity: String =
            connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        let foreign_key_violations: u64 = connection
            .prepare("PRAGMA foreign_key_check")?
            .query_map([], |_| Ok(()))?
            .count() as u64;
        let mut statement = connection.prepare(
            "SELECT relative_path, sha256, size_bytes, schema_version FROM snapshot_artifact ORDER BY relative_path",
        )?;
        let artifacts = statement
            .query_map([], |row| {
                Ok(ArtifactMetadata {
                    relative_path: row.get(0)?,
                    sha256: row.get(1)?,
                    size_bytes: row.get::<_, i64>(2)? as u64,
                    schema_version: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        let known_paths = artifacts
            .iter()
            .map(|artifact| artifact.relative_path.clone())
            .collect::<Vec<_>>();
        let mut missing_artifacts = 0;
        let mut corrupt_artifacts = 0;
        for metadata in &artifacts {
            match artifact::read_report(self.state_dir(), metadata) {
                Ok(_) => {}
                Err(InventoryError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                    missing_artifacts += 1;
                }
                Err(_) => corrupt_artifacts += 1,
            }
        }
        drop(connection);
        let metrics = self.metrics_sync()?;
        let wal_checkpoint = self.wal_checkpoint_sync()?;
        let orphan_artifacts = artifact::inspect_orphans(self.state_dir(), &known_paths)?;
        let healthy = sqlite_integrity == "ok"
            && foreign_key_violations == 0
            && missing_artifacts == 0
            && corrupt_artifacts == 0
            && orphan_artifacts == 0
            && metrics.pending_artifact_deletes == 0;
        Ok(InventoryHealth {
            schema_version: SNAPSHOT_CLI_SCHEMA_VERSION.to_string(),
            healthy,
            sqlite_integrity,
            foreign_key_violations,
            missing_artifacts,
            corrupt_artifacts,
            orphan_artifacts,
            metrics,
            wal_checkpoint,
        })
    }

    fn wal_checkpoint_sync(&self) -> Result<WalCheckpointResult> {
        self.connect()?
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
                Ok(WalCheckpointResult {
                    busy: row.get::<_, i64>(0)? as u64,
                    log_frames: row.get::<_, i64>(1)? as u64,
                    checkpointed_frames: row.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(Into::into)
    }
}

fn count(connection: &rusqlite::Connection, table: &str) -> Result<u64> {
    let allowed = [
        "hardware_snapshot",
        "snapshot_device",
        "probe_history",
        "artifact_delete_queue",
    ];
    if !allowed.contains(&table) {
        return Err(InventoryError::InvalidReport("invalid metric table"));
    }
    connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0).map(|value| value as u64)
        })
        .map_err(Into::into)
}

fn now() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| InventoryError::InvalidReport("UTC timestamp formatting failed"))
}
