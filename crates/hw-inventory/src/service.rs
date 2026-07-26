use crate::{
    canonicalize_devices,
    error::{InventoryError, Result},
    probe::{quick_probe, QuickProbeConfig},
    store::SCAN_LEASE_DURATION,
    InventoryState, InventoryStore, ProbeCompletion, ProbeKind,
};
use async_trait::async_trait;
use hw_model::{PartialPolicy, QuickProbeReport, ScanConfig, ScanReport, ScanStatus, SnapshotId};
use std::time::Duration;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

/// Maximum time without an acquisition or renewal before a waiter declares the lease stalled.
const LEASE_STALL_TIMEOUT: Duration = SCAN_LEASE_DURATION;
const LEASE_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Renew the lease well before expiry so a slow healthy scan keeps ownership.
const LEASE_RENEW_INTERVAL: Duration = Duration::from_secs(30);

#[async_trait]
pub trait InventoryScanner: Send + Sync {
    async fn quick_probe(&self) -> Result<QuickProbeReport>;
    async fn full_scan(&self) -> Result<ScanReport>;
}

#[derive(Debug, Clone, Default)]
pub struct RealInventoryScanner {
    pub quick_config: QuickProbeConfig,
    pub scan_config: ScanConfig,
}

#[async_trait]
impl InventoryScanner for RealInventoryScanner {
    async fn quick_probe(&self) -> Result<QuickProbeReport> {
        quick_probe(self.quick_config).await
    }

    async fn full_scan(&self) -> Result<ScanReport> {
        full_scan(self.scan_config.clone()).await
    }
}

async fn full_scan(config: ScanConfig) -> Result<ScanReport> {
    let execution_options = inventory_execution_options(config.timeout);
    hw_collect::collect_scan_report_with_options(config, execution_options)
        .await
        .map_err(|_| InventoryError::FullScanFailed)
}

fn inventory_execution_options(timeout: Duration) -> hw_collect::ScanExecutionOptions {
    hw_collect::ScanExecutionOptions {
        global_deadline: Some(timeout),
        ..hw_collect::ScanExecutionOptions::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveInventoryOptions {
    pub force_full_scan: bool,
    pub scan_config: ScanConfig,
    pub max_snapshot_age: Option<Duration>,
    pub partial_policy: PartialPolicy,
    /// Maximum time to wait without observing lease progress. Renewals reset this timeout.
    pub lease_wait_timeout: Option<Duration>,
}

impl Default for ObserveInventoryOptions {
    fn default() -> Self {
        Self {
            force_full_scan: false,
            scan_config: ScanConfig::default(),
            max_snapshot_age: Some(Duration::from_secs(24 * 60 * 60)),
            partial_policy: PartialPolicy::PublishIfCoreComplete,
            lease_wait_timeout: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationSource {
    ReusedSnapshot,
    NewFullScan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryObservation {
    pub report: ScanReport,
    pub snapshot_id: SnapshotId,
    pub result_source: ObservationSource,
    pub hardware_changed: bool,
}

pub async fn observe_inventory(
    store: &InventoryStore,
    options: ObserveInventoryOptions,
) -> Result<InventoryObservation> {
    let quick_timeout = options.scan_config.timeout.min(Duration::from_secs(5));
    let scanner = RealInventoryScanner {
        quick_config: QuickProbeConfig {
            timeout: quick_timeout,
        },
        scan_config: options.scan_config.clone(),
    };
    observe_inventory_with_scanner(store, options, &scanner).await
}

pub async fn observe_inventory_with_scanner(
    store: &InventoryStore,
    options: ObserveInventoryOptions,
    scanner: &dyn InventoryScanner,
) -> Result<InventoryObservation> {
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
                return observation_from_snapshot(
                    store,
                    snapshot_id,
                    ObservationSource::ReusedSnapshot,
                    hardware_changed(&initial_state, &report),
                )
                .await;
            }
            // Keep the quick probe `running` until reuse, publication, or an error/timeout path
            // completes it. This avoids orphaned rows on lease timeout.
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
    // When the quick probe failed, there is no open quick history row to finish later.
    let open_quick_history = quick.as_ref().map(|_| quick_history);

    let owner_id = SnapshotId::new_v7().to_string();
    let lease_wait_timeout = options.lease_wait_timeout.unwrap_or(LEASE_STALL_TIMEOUT);
    let mut observed_lease = None;
    let mut lease_progress_started = std::time::Instant::now();
    loop {
        if store
            .try_acquire_lease(owner_id.clone(), SCAN_LEASE_DURATION)
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
                if let Some(history) = open_quick_history {
                    store
                        .finish_probe(
                            history,
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
                }
                return observation_from_snapshot(
                    store,
                    snapshot_id,
                    ObservationSource::ReusedSnapshot,
                    hardware_changed(&initial_state, report),
                )
                .await;
            }
        }
        let active_lease = store.active_lease().await?;
        if active_lease != observed_lease {
            observed_lease = active_lease;
            lease_progress_started = std::time::Instant::now();
        }
        if lease_progress_started.elapsed() >= lease_wait_timeout {
            if let Some(history) = open_quick_history {
                let _ = store
                    .finish_probe(
                        history,
                        ProbeCompletion::Failed,
                        None,
                        None,
                        None,
                        Some(quick_duration),
                        None,
                        Some(InventoryError::LeaseTimeout.code().to_string()),
                        Some("timed out waiting for the snapshot scan lease".to_string()),
                    )
                    .await;
            }
            return Err(InventoryError::LeaseTimeout);
        }
        tokio::time::sleep(LEASE_POLL_INTERVAL).await;
    }

    let result = run_full_scan_under_lease(
        store,
        &options,
        scanner,
        quick.as_ref(),
        baseline_current,
        &owner_id,
    )
    .await;
    let quick_finish_result = match (&result, &quick, open_quick_history) {
        (Ok(full), Some(report), Some(history)) => {
            store
                .finish_probe(
                    history,
                    ProbeCompletion::Succeeded,
                    Some(full.snapshot_id()),
                    Some(report.machine_bind_id.clone()),
                    Some(report.configuration_fingerprint.clone()),
                    Some(quick_duration),
                    Some(report.warnings.len() as u64),
                    None,
                    None,
                )
                .await
        }
        (Err(error), _, Some(history)) => {
            store
                .finish_probe(
                    history,
                    ProbeCompletion::Failed,
                    None,
                    None,
                    None,
                    Some(quick_duration),
                    None,
                    Some(error.code().to_string()),
                    Some(error.to_string()),
                )
                .await
        }
        _ => Ok(()),
    };
    let release_result = store.release_lease(owner_id).await;
    quick_finish_result?;
    release_result?;

    let result = result?;
    observation_from_snapshot(
        store,
        result.snapshot_id(),
        result.source(),
        quick
            .as_ref()
            .is_none_or(|report| hardware_changed(&initial_state, report)),
    )
    .await
}

#[derive(Debug, Clone, Copy)]
enum FullScanResult {
    Reused(SnapshotId),
    Published(SnapshotId),
}

impl FullScanResult {
    fn snapshot_id(self) -> SnapshotId {
        match self {
            Self::Reused(snapshot_id) | Self::Published(snapshot_id) => snapshot_id,
        }
    }

    fn source(self) -> ObservationSource {
        match self {
            Self::Reused(_) => ObservationSource::ReusedSnapshot,
            Self::Published(_) => ObservationSource::NewFullScan,
        }
    }
}

async fn run_full_scan_under_lease(
    store: &InventoryStore,
    options: &ObserveInventoryOptions,
    scanner: &dyn InventoryScanner,
    quick: Option<&QuickProbeReport>,
    baseline_current: Option<SnapshotId>,
    owner_id: &str,
) -> Result<FullScanResult> {
    if let Some(quick) = quick {
        let state = store.current_state().await?;
        if state.current_snapshot_id != baseline_current {
            if let Some(snapshot_id) = reusable_snapshot(store, &state, quick, options, true).await
            {
                return Ok(FullScanResult::Reused(snapshot_id));
            }
        }
    }

    let previous = store.current_state().await?.current_snapshot_id;
    let full_history = store.start_probe(ProbeKind::Full, previous).await?;
    let full_started = std::time::Instant::now();
    let report = match scan_with_lease_renewal(store, scanner, owner_id).await {
        Ok(report) if report.status != ScanStatus::Failed => report,
        Ok(_) => {
            fail_full_probe(
                store,
                full_history,
                full_started,
                None,
                None,
                None,
                &InventoryError::FullScanFailed,
            )
            .await;
            return Err(InventoryError::FullScanFailed);
        }
        Err(InventoryError::StaleLease) => {
            fail_full_probe(
                store,
                full_history,
                full_started,
                None,
                None,
                None,
                &InventoryError::StaleLease,
            )
            .await;
            return Err(InventoryError::StaleLease);
        }
        Err(_) => {
            fail_full_probe(
                store,
                full_history,
                full_started,
                None,
                None,
                None,
                &InventoryError::FullScanFailed,
            )
            .await;
            return Err(InventoryError::FullScanFailed);
        }
    };
    if report.status == ScanStatus::Partial && options.partial_policy == PartialPolicy::Reject {
        fail_full_probe(
            store,
            full_history,
            full_started,
            None,
            None,
            Some(report.warnings.len() as u64),
            &InventoryError::PartialRejected,
        )
        .await;
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
    let warning_count = report.warnings.len() as u64;
    let canonical =
        match canonicalize_devices(&report.devices, warnings, trusted_absent, now_rfc3339()) {
            Ok(canonical) => canonical,
            Err(error) => {
                fail_full_probe(
                    store,
                    full_history,
                    full_started,
                    None,
                    None,
                    Some(warning_count),
                    &error,
                )
                .await;
                return Err(error);
            }
        };
    if !canonical.coverage.core_complete() {
        fail_full_probe(
            store,
            full_history,
            full_started,
            Some(canonical.machine_bind_id),
            Some(canonical.configuration_fingerprint),
            Some(warning_count),
            &InventoryError::CoreIdentityIncomplete,
        )
        .await;
        return Err(InventoryError::CoreIdentityIncomplete);
    }
    let state_probe = quick
        .filter(|quick| quick.coverage.core_complete())
        .cloned()
        .unwrap_or_else(|| canonical.clone());
    match store
        .publish_snapshot_for_probe(
            report,
            canonical,
            state_probe,
            full_history,
            owner_id.to_string(),
        )
        .await
    {
        Ok(snapshot_id) => Ok(FullScanResult::Published(snapshot_id)),
        Err(error) => {
            // Publication rolls back the in-transaction success update; mark failed explicitly.
            fail_full_probe(store, full_history, full_started, None, None, None, &error).await;
            Err(error)
        }
    }
}

async fn scan_with_lease_renewal(
    store: &InventoryStore,
    scanner: &dyn InventoryScanner,
    owner_id: &str,
) -> Result<ScanReport> {
    scan_with_lease_policy(
        store,
        scanner,
        owner_id,
        SCAN_LEASE_DURATION,
        LEASE_RENEW_INTERVAL,
    )
    .await
}

async fn scan_with_lease_policy(
    store: &InventoryStore,
    scanner: &dyn InventoryScanner,
    owner_id: &str,
    lease_duration: Duration,
    renew_interval: Duration,
) -> Result<ScanReport> {
    let scan = scanner.full_scan();
    tokio::pin!(scan);
    let mut renew = tokio::time::interval(renew_interval);
    renew.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // Skip the immediate first tick so we do not renew right after acquire.
    renew.tick().await;
    loop {
        tokio::select! {
            report = &mut scan => return report,
            _ = renew.tick() => {
                if !store
                    .renew_lease(owner_id.to_string(), lease_duration)
                    .await?
                {
                    return Err(InventoryError::StaleLease);
                }
            }
        }
    }
}

async fn fail_full_probe(
    store: &InventoryStore,
    full_history: i64,
    full_started: std::time::Instant,
    machine_bind_id: Option<String>,
    configuration_fingerprint: Option<String>,
    warning_count: Option<u64>,
    error: &InventoryError,
) {
    let _ = store
        .finish_probe(
            full_history,
            ProbeCompletion::Failed,
            None,
            machine_bind_id,
            configuration_fingerprint,
            Some(elapsed_ms(full_started)),
            warning_count,
            Some(error.code().to_string()),
            Some(error.to_string()),
        )
        .await;
}

async fn reusable_snapshot(
    store: &InventoryStore,
    state: &InventoryState,
    quick: &QuickProbeReport,
    options: &ObserveInventoryOptions,
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

fn hardware_changed(state: &InventoryState, quick: &QuickProbeReport) -> bool {
    state.current_snapshot_id.is_none()
        || state.current_machine_bind_id.as_deref() != Some(&quick.machine_bind_id)
        || state.last_configuration_fingerprint.as_deref() != Some(&quick.configuration_fingerprint)
        || state.fingerprint_version != Some(quick.fingerprint_version)
}

async fn observation_from_snapshot(
    store: &InventoryStore,
    snapshot_id: SnapshotId,
    result_source: ObservationSource,
    hardware_changed: bool,
) -> Result<InventoryObservation> {
    let report = store
        .load_scan_report(snapshot_id)
        .await?
        .ok_or(InventoryError::SnapshotNotFound(snapshot_id))?;
    Ok(InventoryObservation {
        report,
        snapshot_id,
        result_source,
        hardware_changed,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct SlowScanner;

    #[async_trait]
    impl InventoryScanner for SlowScanner {
        async fn quick_probe(&self) -> Result<QuickProbeReport> {
            unreachable!("quick probe is not used by the lease renewal test")
        }

        async fn full_scan(&self) -> Result<ScanReport> {
            tokio::time::sleep(Duration::from_millis(120)).await;
            Err(InventoryError::FullScanFailed)
        }
    }

    #[test]
    fn inventory_full_scan_deadline_matches_source_timeout() {
        let timeout = Duration::from_secs(7);
        let options = inventory_execution_options(timeout);
        assert_eq!(options.global_deadline, Some(timeout));
    }

    #[tokio::test]
    async fn slow_scan_renews_lease_beyond_original_expiry() {
        let temp = tempfile::tempdir().unwrap();
        let store = InventoryStore::open(temp.path()).await.unwrap();
        let owner_id = "slow-owner".to_string();
        let lease_duration = Duration::from_millis(50);
        assert!(store
            .try_acquire_lease(owner_id.clone(), lease_duration)
            .await
            .unwrap());

        scan_with_lease_policy(
            &store,
            &SlowScanner,
            &owner_id,
            lease_duration,
            Duration::from_millis(15),
        )
        .await
        .unwrap_err();

        assert!(store
            .renew_lease(owner_id.clone(), lease_duration)
            .await
            .unwrap());
        store.release_lease(owner_id).await.unwrap();
    }
}
