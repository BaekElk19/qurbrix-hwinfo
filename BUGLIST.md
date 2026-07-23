# Bug List

Review date: 2026-07-23

Scope: independent reviews of the inventory service/store and of the CLI/bindid
surface after `v0.2.2`.

## P1

### Lease expiry allows a stale scan to overwrite the current snapshot

Status: **DONE** (2026-07-24)

### A healthy slow scan makes concurrent observers fail after 30 seconds

Status: **DONE** (2026-07-24)

Locations: `crates/hw-inventory/src/service.rs`

Fix: default lease wait timeout matches lease duration (120s); configurable via
`ObserveInventoryOptions::lease_wait_timeout`.
Regression: `waiter_survives_slow_healthy_scan_beyond_thirty_seconds`.

### `scan --no-optional-sources` was removed without a compatible replacement

Status: open

## P2

### Full probe history remains `running` when publication fails

Status: open

### Lease timeout leaves the quick probe history `running`

Status: open

### Snapshot commands are documented as non-root but fail with the default state directory

Status: open

### CLI `--timeout` does not constrain quick probes

Status: open
