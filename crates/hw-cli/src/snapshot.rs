use crate::args::{SnapshotArgs, SnapshotCommand};
use hw_inventory::{
    diff_snapshots, ensure_snapshot, InventoryError, InventoryStore, PageRequest, RetentionPolicy,
    SNAPSHOT_CLI_SCHEMA_VERSION,
};
use hw_model::{EnsureSnapshotOptions, PartialPolicy};
use serde::Serialize;

#[derive(Serialize)]
struct EnsureOutput {
    schema_version: &'static str,
    snapshot_id: hw_model::SnapshotId,
}

#[derive(Serialize)]
struct ShowOutput {
    schema_version: &'static str,
    snapshot: hw_model::StoredSnapshot,
    report: hw_model::ScanReport,
}

#[derive(Serialize)]
struct ListOutput {
    schema_version: &'static str,
    limit: u32,
    offset: u64,
    snapshots: Vec<hw_model::StoredSnapshot>,
}

#[derive(Serialize)]
struct LifecycleOutput {
    schema_version: &'static str,
    snapshot_id: hw_model::SnapshotId,
    action: &'static str,
}

pub async fn run_snapshot_command(args: SnapshotArgs) -> Result<(), InventoryError> {
    match args.command {
        SnapshotCommand::Ensure(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            let snapshot_id = ensure_snapshot(
                &store,
                EnsureSnapshotOptions {
                    force_full_scan: args.force,
                    max_snapshot_age: Some(args.max_age),
                    partial_policy: if args.reject_partial {
                        PartialPolicy::Reject
                    } else {
                        PartialPolicy::PublishIfCoreComplete
                    },
                },
            )
            .await?;
            write_json(
                &EnsureOutput {
                    schema_version: SNAPSHOT_CLI_SCHEMA_VERSION,
                    snapshot_id,
                },
                args.pretty,
            )?;
        }
        SnapshotCommand::Show(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            let snapshot = store
                .load_snapshot(args.snapshot_id)
                .await?
                .ok_or(InventoryError::SnapshotNotFound(args.snapshot_id))?;
            let report = store
                .load_scan_report(args.snapshot_id)
                .await?
                .ok_or(InventoryError::SnapshotNotFound(args.snapshot_id))?;
            write_json(
                &ShowOutput {
                    schema_version: SNAPSHOT_CLI_SCHEMA_VERSION,
                    snapshot,
                    report,
                },
                args.pretty,
            )?;
        }
        SnapshotCommand::List(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            let snapshots = store
                .list_snapshots(
                    args.machine_bind_id,
                    PageRequest {
                        limit: args.limit,
                        offset: args.offset,
                    },
                )
                .await?;
            write_json(
                &ListOutput {
                    schema_version: SNAPSHOT_CLI_SCHEMA_VERSION,
                    limit: args.limit,
                    offset: args.offset,
                    snapshots,
                },
                args.pretty,
            )?;
        }
        SnapshotCommand::Diff(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            let diff = diff_snapshots(&store, args.from_snapshot_id, args.to_snapshot_id).await?;
            write_json(&diff, args.pretty)?;
        }
        SnapshotCommand::Export(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            let metadata = store
                .export_scan_report(args.snapshot_id, args.output, args.overwrite)
                .await?;
            write_json(&metadata, args.pretty)?;
        }
        SnapshotCommand::Health(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            write_json(&store.health_check().await?, args.pretty)?;
        }
        SnapshotCommand::Prune(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            let report = store
                .apply_retention(RetentionPolicy {
                    keep_recent_per_machine: args.keep_recent,
                    uploaded_max_age: args.max_age,
                    dry_run: args.dry_run,
                })
                .await?;
            write_json(&report, args.pretty)?;
        }
        SnapshotCommand::Pin(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            store.set_pinned(args.snapshot_id, !args.unset).await?;
            write_json(
                &LifecycleOutput {
                    schema_version: SNAPSHOT_CLI_SCHEMA_VERSION,
                    snapshot_id: args.snapshot_id,
                    action: if args.unset { "unpinned" } else { "pinned" },
                },
                args.pretty,
            )?;
        }
        SnapshotCommand::MarkUploaded(args) => {
            let store = InventoryStore::open(args.state_dir).await?;
            store.mark_uploaded(args.snapshot_id, args.at).await?;
            write_json(
                &LifecycleOutput {
                    schema_version: SNAPSHOT_CLI_SCHEMA_VERSION,
                    snapshot_id: args.snapshot_id,
                    action: "marked_uploaded",
                },
                args.pretty,
            )?;
        }
    }
    Ok(())
}

fn write_json(value: &impl Serialize, pretty: bool) -> Result<(), InventoryError> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}
