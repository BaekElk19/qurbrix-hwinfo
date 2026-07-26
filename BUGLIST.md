# Bug List

Review date: 2026-07-26

Scope: `v0.2.3` completion review covering full-inventory publication, lease
coordination, timeout behavior, probe recovery, and stdout-only views. All
identified items are closed.

---

## Closed (2026-07-26)

### P1

### CLI view filters are applied to inventory collection and publication

Status: **DONE** (2026-07-26)

Locations: `crates/hw-cli/src/main.rs`, `crates/hw-cli/src/view.rs`

Fix: `inventory_scan_config(timeout)` always uses a complete `ScanConfig` except
for timeout. Device, optional-source, source-evidence, and warning filters apply
only after `observe_inventory` returns the complete stored report.
Regression: `inventory_scan_config_is_always_full` and the `hw_cli::view` tests.

### `--no-optional-sources` permanently reduces the reusable inventory snapshot

Status: **DONE** (2026-07-26)

Locations: `crates/hw-cli/src/main.rs`, `crates/hw-cli/src/view.rs`

Fix: the flag hides optional peripheral kinds from stdout after full
collection/publication; reusable snapshots remain complete.

### P2

### Concurrent waiters time out after one lease period on multi-renewal scans

Status: **DONE** (2026-07-26)

Locations: `crates/hw-inventory/src/service.rs`,
`crates/hw-inventory/src/store.rs`

Fix: the waiter timeout now measures lease stalls. A changed owner or expiry
timestamp proves progress and resets the timeout, so a healthy renewing holder
can finish without an arbitrary total-wait failure.
Regression: `lease_renewal_resets_waiter_stall_timeout` and
`slow_scan_renews_lease_beyond_original_expiry`.

### CLI `--timeout` does not bound the full-scan global deadline

Status: **DONE** (2026-07-26)

Locations: `crates/hw-cli/src/main.rs`, `crates/hw-inventory/src/service.rs`,
`crates/hw-collect/src/execution.rs`

Fix: inventory full scans derive `ScanExecutionOptions::global_deadline` from
`ScanConfig::timeout`; the same value bounds each source and the whole full
scan, while the quick probe remains capped at five seconds.
Regression: `inventory_full_scan_deadline_matches_source_timeout`.

### Opening the store can mark a still-running healthy probe as failed

Status: **DONE** (2026-07-26)

Locations: `crates/hw-inventory/src/store.rs` (`recover_stale_sync`,
`initialize`)

Fix: startup recovery removes expired leases and checks for a live owner in one
immediate transaction. Old running probes are recovered only when no live scan
lease exists.
Regression: `startup_preserves_old_running_probe_while_lease_is_active` and
`startup_marks_old_running_probe_failed`.

### P3

### Output filtering obscures report status semantics

Status: **DONE** (2026-07-26)

Locations: `crates/hw-cli/src/view.rs` (`filtered_report`)

Resolution: `status` explicitly describes the underlying inventory observation,
not whether a stdout view matched devices. Filtering to an empty view or hiding
warnings therefore preserves the original status and exit behavior.
Regression: `empty_view_and_hidden_warnings_preserve_observation_status`.

### `publish_snapshot` has no lease fence

Status: **DONE** (2026-07-26)

Locations: `crates/hw-inventory/src/store.rs`

Fix: direct publication must acquire the scan lease and passes its owner token
into the fenced publication transaction. It releases the lease on both success
and failure and refuses to publish while another owner is active.
Regression: `direct_publication_respects_the_scan_lease`.

---

## Closed (2026-07-24)

### P1

### Lease expiry allows a stale scan to overwrite the current snapshot

Status: **DONE** (2026-07-24)

Locations: `crates/hw-inventory/src/service.rs`, `crates/hw-inventory/src/store.rs`

Fix: renew lease while scanning; fence publication with lease owner (`StaleLease`).
Regression: `stale_scan_cannot_overwrite_after_lease_expiry`.

### A healthy slow scan makes concurrent observers fail after 30 seconds

Status: **DONE** (2026-07-24)

Locations: `crates/hw-inventory/src/service.rs`

Fix: default lease wait timeout matches lease duration (120s); configurable via
`ObserveInventoryOptions::lease_wait_timeout`.
Regression: `waiter_survives_slow_healthy_scan_beyond_thirty_seconds`.

### `scan --no-optional-sources` was removed without a compatible replacement

Status: **DONE** (2026-07-24)

Locations: `crates/hw-cli/src/args.rs`, `crates/hw-cli/src/main.rs`,
`crates/hw-collect/src/collector.rs`, `README.md`, `README.zh-CN.md`

Fix: restore `--no-optional-sources` (now stdout-only under the iron rule).

### P2

### Full probe history remains `running` when publication fails

Status: **DONE** (2026-07-24)

Locations: `crates/hw-inventory/src/service.rs`

Fix: mark full probe failed on every publish/canonicalization error path.
Regression: `publish_failure_marks_full_probe_failed`.

### Lease timeout leaves the quick probe history `running`

Status: **DONE** (2026-07-24)

Locations: `crates/hw-inventory/src/service.rs`

Fix: finish the open quick probe as failed with `inventory.lease_timeout` before
returning `LeaseTimeout`.
Regression: `lease_timeout_marks_quick_probe_failed`.

### Snapshot commands are documented as non-root but fail with the default state directory

Status: **DONE** (2026-07-24)

Locations: `README.md`, `README.zh-CN.md`

Fix: document default state-dir ownership and `--state-dir` requirements for
non-root snapshot access.

### CLI `--timeout` does not constrain quick probes

Status: **DONE** (2026-07-24)

Locations: `crates/hw-inventory/src/service.rs`, `crates/hw-cli/src/main.rs`

Fix: `observe_inventory` builds `QuickProbeConfig` from `scan_config.timeout`
(capped at 5s). Documented next to the scan timeout flag.
