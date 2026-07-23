use crate::{
    error::{InventoryError, Result},
    model::{ChangedDevice, SnapshotDiff, SNAPSHOT_CLI_SCHEMA_VERSION},
    InventoryStore,
};
use hw_model::{Device, SnapshotId};
use std::collections::BTreeMap;

pub async fn diff_snapshots(
    store: &InventoryStore,
    from_snapshot_id: SnapshotId,
    to_snapshot_id: SnapshotId,
) -> Result<SnapshotDiff> {
    let from_snapshot = store
        .load_snapshot(from_snapshot_id)
        .await?
        .ok_or(InventoryError::SnapshotNotFound(from_snapshot_id))?;
    let to_snapshot = store
        .load_snapshot(to_snapshot_id)
        .await?
        .ok_or(InventoryError::SnapshotNotFound(to_snapshot_id))?;
    let from = store
        .load_scan_report(from_snapshot_id)
        .await?
        .ok_or(InventoryError::SnapshotNotFound(from_snapshot_id))?;
    let to = store
        .load_scan_report(to_snapshot_id)
        .await?
        .ok_or(InventoryError::SnapshotNotFound(to_snapshot_id))?;
    let mut before = from
        .devices
        .into_iter()
        .map(|device| (device.id.clone(), device))
        .collect::<BTreeMap<_, _>>();
    let mut after = to
        .devices
        .into_iter()
        .map(|device| (device.id.clone(), device))
        .collect::<BTreeMap<_, _>>();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let all_ids = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for id in all_ids {
        match (before.remove(&id), after.remove(&id)) {
            (None, Some(device)) => added.push(device),
            (Some(device), None) => removed.push(device),
            (Some(before), Some(after)) if before != after => changed.push(ChangedDevice {
                device_id: id,
                before,
                after,
            }),
            _ => {}
        }
    }
    sort_devices(&mut added);
    sort_devices(&mut removed);
    Ok(SnapshotDiff {
        schema_version: SNAPSHOT_CLI_SCHEMA_VERSION.to_string(),
        from_snapshot_id,
        to_snapshot_id,
        machine_identity_changed: from_snapshot.machine_bind_id != to_snapshot.machine_bind_id,
        configuration_changed: from_snapshot.configuration_fingerprint
            != to_snapshot.configuration_fingerprint,
        added,
        removed,
        changed,
    })
}

fn sort_devices(devices: &mut [Device]) {
    devices.sort_by(|left, right| left.id.cmp(&right.id));
}
