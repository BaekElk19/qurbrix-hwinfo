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

## P2

### Full probe history remains `running` when publication fails

Status: **DONE** (2026-07-24)

### Lease timeout leaves the quick probe history `running`

Status: **DONE** (2026-07-24)

### Snapshot commands are documented as non-root but fail with the default state directory

Status: **DONE** (2026-07-24)

Locations: `README.md`, `README.zh-CN.md`

Fix: document that default state dir is root-owned after privileged scans and
that non-root snapshot use needs a readable `--state-dir`.

### CLI `--timeout` does not constrain quick probes

Status: open
