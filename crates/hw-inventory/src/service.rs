use crate::{
    canonicalize_devices,
    error::{InventoryError, Result},
    probe::{quick_probe, QuickProbeConfig},
    InventoryState, InventoryStore, ProbeCompletion, ProbeKind,
};
use async_trait::async_trait;
use hw_model::{
    EnsureSnapshotOptions, PartialPolicy, QuickProbeReport, ScanConfig, ScanReport, ScanStatus,
    SnapshotId,
};
use std::time::Duration;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const LEASE_DURATION: Duration = Duration::from_secs(2 * 60);
const LEASE_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const LEASE_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[async_trait]
pub trait SnapshotScanner: Send + Sync {
    async fn quick_probe(&self) -> Result<QuickProbeReport>;
    async fn full_scan(&self) -> Result<ScanReport>;
}

#[derive(Debug, Clone, Default)]
pub struct RealSnapshotScanner {
    pub quick_config: QuickProbeConfig,
    pub scan_config: ScanConfig,
}

#[async_trait]
impl SnapshotScanner for RealSnapshotScanner {
    async fn quick_probe(&self) -> Result<QuickProbeReport> {
        quick_probe(self.quick_config).await
    }

    async fn full_scan(&self) -> Result<ScanReport> {
        full_scan(self.scan_config.clone()).await
    }
}

pub async fn full_scan(config: ScanConfig) -> Result<ScanReport> {
    hw_collect::collect_scan_report(config)
        .await
        .map_err(|_| InventoryError::FullScanFailed)
}

pub async fn ensure_snapshot(
    store: &InventoryStore,
    options: EnsureSnapshotOptions,
) -> Result<SnapshotId> {
    ensure_snapshot_with_scanner(store, options, &RealSnapshotScanner::default()).await
}

pub async fn ensure_snapshot_with_scanner(
    store: &InventoryStore,
    options: EnsureSnapshotOptions,
    scanner: &dyn SnapshotScanner,
) -> Result<SnapshotId> {
    let initial_state = store.current_state().await?;
    let baseline_current = initial_state.current_snapshot_id;
    let quick_history = store
        .start_probe(ProbeKind::Quick, baseline_current)
        .await?;
    let quick_started = std::time::Instant::now();
    let quick_result = scanner.quick_probe().await;
    let quick_duration = elapsed_ms(quick_started);
    let quick = match quick_result {
        Ok(report) => {
            if let Some(snapshot_id) =
                reusable_snapshot(store, &initial_state, &report, &options, false).await
            {
                store
                    .finish_probe(
                        quick_history,
                        ProbeCompletion::Succeeded,
                        Some(snapshot_id),
                        Some(report.machine_bind_id.clone()),
                        Some(report.configuration_fingerprint.clone()),
                        Some(quick_duration),
                        Some(report.warnings.len() as u64),
                        None,
                        None,
                    )
                    .await?;
                return Ok(snapshot_id);
            }
            store
                .finish_probe(
                    quick_history,
                    ProbeCompletion::Succeeded,
                    None,
                    Some(report.machine_bind_id.clone()),
                    Some(report.configuration_fingerprint.clone()),
                    Some(quick_duration),
                    Some(report.warnings.len() as u64),
                    None,
                    None,
                )
                .await?;
            Some(report)
        }
        Err(error) => {
            store
                .finish_probe(
                    quick_history,
                    ProbeCompletion::Failed,
                    None,
                    None,
                    None,
                    Some(quick_duration),
                    None,
                    Some(error.code().to_string()),
                    Some("quick probe failed; full scan fallback started".to_string()),
                )
                .await?;
            None
        }
    };

    let owner_id = SnapshotId::new_v7().to_string();
    let lease_wait_started = std::time::Instant::now();
    loop {
        if store
            .try_acquire_lease(owner_id.clone(), LEASE_DURATION)
            .await?
        {
            break;
        }
        if let Some(report) = &quick {
            let state = store.current_state().await?;
            let published_by_peer = state.current_snapshot_id != baseline_current;
            if let Some(snapshot_id) =
                reusable_snapshot(store, &state, report, &options, published_by_peer).await
            {
                store
                    .finish_probe(
                        quick_history,
                        ProbeCompletion::Succeeded,
                        Some(snapshot_id),
                        Some(report.machine_bind_id.clone()),
                        Some(report.configuration_fingerprint.clone()),
                        Some(quick_duration),
                        Some(report.warnings.len() as u64),
                        None,
                        None,
                    )
                    .await?;
                return Ok(snapshot_id);
            }
        }
        if lease_wait_started.elapsed() >= LEASE_WAIT_TIMEOUT {
            return Err(InventoryError::LeaseTimeout);
        }
        tokio::time::sleep(LEASE_POLL_INTERVAL).await;
    }

    let result =
        run_full_scan_under_lease(store, &options, scanner, quick.as_ref(), baseline_current).await;
    if let (Ok(snapshot_id), Some(report)) = (&result, &quick) {
        let _ = store
            .finish_probe(
                quick_history,
                ProbeCompletion::Succeeded,
                Some(*snapshot_id),
                Some(report.machine_bind_id.clone()),
                Some(report.configuration_fingerprint.clone()),
                Some(quick_duration),
                Some(report.warnings.len() as u64),
                None,
                None,
            )
            .await;
    }
    let _ = store.release_lease(owner_id).await;
    result
}

async fn run_full_scan_under_lease(
    store: &InventoryStore,
    options: &EnsureSnapshotOptions,
    scanner: &dyn SnapshotScanner,
    quick: Option<&QuickProbeReport>,
    baseline_current: Option<SnapshotId>,
) -> Result<SnapshotId> {
    if let Some(quick) = quick {
        let state = store.current_state().await?;
        if state.current_snapshot_id != baseline_current {
            if let Some(snapshot_id) = reusable_snapshot(store, &state, quick, options, true).await
            {
                return Ok(snapshot_id);
            }
        }
    }

    let previous = store.current_state().await?.current_snapshot_id;
    let full_history = store.start_probe(ProbeKind::Full, previous).await?;
    let full_started = std::time::Instant::now();
    let report = match scanner.full_scan().await {
        Ok(report) if report.status != ScanStatus::Failed => report,
        Ok(_) | Err(_) => {
            store
                .finish_probe(
                    full_history,
                    ProbeCompletion::Failed,
                    None,
                    None,
                    None,
                    Some(elapsed_ms(full_started)),
                    None,
                    Some(InventoryError::FullScanFailed.code().to_string()),
                    Some("full scan failed; previous snapshot retained".to_string()),
                )
                .await?;
            return Err(InventoryError::FullScanFailed);
        }
    };
    if report.status == ScanStatus::Partial && options.partial_policy == PartialPolicy::Reject {
        store
            .finish_probe(
                full_history,
                ProbeCompletion::Failed,
                None,
                None,
                None,
                Some(elapsed_ms(full_started)),
                Some(report.warnings.len() as u64),
                Some(InventoryError::PartialRejected.code().to_string()),
                Some("partial scan rejected by policy".to_string()),
            )
            .await?;
        return Err(InventoryError::PartialRejected);
    }

    let trusted_absent = quick
        .map(|quick| quick.coverage.trusted_absent.iter().copied().collect())
        .unwrap_or_default();
    let warnings = report
        .warnings
        .iter()
        .map(|warning| format!("{}: {}", warning.code, warning.message))
        .collect();
    let canonical = canonicalize_devices(&report.devices, warnings, trusted_absent, now_rfc3339())?;
    if !canonical.coverage.core_complete() {
        store
            .finish_probe(
                full_history,
                ProbeCompletion::Failed,
                None,
                Some(canonical.machine_bind_id),
                Some(canonical.configuration_fingerprint),
                Some(elapsed_ms(full_started)),
                Some(report.warnings.len() as u64),
                Some(InventoryError::CoreIdentityIncomplete.code().to_string()),
                Some("full scan did not satisfy core identity contract".to_string()),
            )
            .await?;
        return Err(InventoryError::CoreIdentityIncomplete);
    }
    store
        .publish_snapshot_for_probe(report, canonical, full_history)
        .await
}

async fn reusable_snapshot(
    store: &InventoryStore,
    state: &InventoryState,
    quick: &QuickProbeReport,
    options: &EnsureSnapshotOptions,
    ignore_force: bool,
) -> Option<SnapshotId> {
    let snapshot_id = state.current_snapshot_id?;
    if options.force_full_scan && !ignore_force {
        return None;
    }
    if state.current_machine_bind_id.as_deref() != Some(&quick.machine_bind_id)
        || state.last_configuration_fingerprint.as_deref() != Some(&quick.configuration_fingerprint)
        || state.fingerprint_version != Some(quick.fingerprint_version)
        || !is_fresh(
            state.current_snapshot_created_at.as_deref(),
            options.max_snapshot_age,
        )
    {
        return None;
    }
    matches!(store.load_scan_report(snapshot_id).await, Ok(Some(_))).then_some(snapshot_id)
}

fn is_fresh(created_at: Option<&str>, max_age: Option<Duration>) -> bool {
    let Some(max_age) = max_age else {
        return true;
    };
    let Some(created_at) = created_at else {
        return false;
    };
    let Ok(created_at) = OffsetDateTime::parse(created_at, &Rfc3339) else {
        return false;
    };
    let age = OffsetDateTime::now_utc() - created_at;
    age.is_negative() || age <= time::Duration::seconds(max_age.as_secs() as i64)
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn elapsed_ms(started: std::time::Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
