# Bug List

Review date: 2026-07-23

Scope: independent reviews of the inventory service/store and of the CLI/bindid
surface after `v0.2.2`.

## P1

### Lease expiry allows a stale scan to overwrite the current snapshot

Status: **DONE** (2026-07-24)

### A healthy slow scan makes concurrent observers fail after 30 seconds

Status: **DONE** (2026-07-24)

### `scan --no-optional-sources` was removed without a compatible replacement

Status: **DONE** (2026-07-24)

Locations: `crates/hw-cli/src/args.rs`, `crates/hw-cli/src/main.rs`,
`crates/hw-collect/src/collector.rs`

Fix: restore `--no-optional-sources` and skip non-core optional probes when set.

## P2

### Full probe history remains `running` when publication fails

Status: open

### Lease timeout leaves the quick probe history `running`

Status: open

### Snapshot commands are documented as non-root but fail with the default state directory

Status: open

### CLI `--timeout` does not constrain quick probes

Status: open
