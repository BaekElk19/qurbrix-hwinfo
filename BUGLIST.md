# Bug List

Review date: 2026-07-23

Scope: independent reviews of the inventory service/store and of the CLI/bindid
surface after `v0.2.2`. Both reviewers inspected the same committed source
independently. All items below are closed.

## P1

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

Fix: restore `--no-optional-sources` and skip non-core optional probes when set.

## P2

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
