# Hardware Snapshot Execution Log

## Run Identity

- Runbook: `2026-07-22_22-33-55-hardware-snapshot-plan.md`
- Starting repository HEAD: `93185c1`
- Runbook baseline commit: `e9c3ed9`
- Working branch: `codex/hardware-snapshot-v0.2.0`
- Started: 2026-07-23 CST
- Host: Linux x86_64, kernel `6.6.143-amd64-desktop-hwe`
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- Cargo: `cargo 1.97.1 (c980f4866 2026-06-30)`

## Operating Constraints

- Tests and smoke tests use temporary directories; the real
  `/var/lib/qurbrix-hwinfo` is not modified.
- No remote push, pull request, release publication, daemon, hotplug listener,
  monitor dependency, or monitoring time series is introduced.
- Existing user changes are preserved and excluded from task commits.

## License Decision

The implementation is an independent Rust implementation based on the local
runbook and the repository's existing APIs. No Deepin Device Manager source is
copied, adapted, or linked. The repository therefore remains licensed under
`MIT OR Apache-2.0`. Candidate alternatives were direct GPL-derived reuse or
independent implementation; the latter preserves the existing license,
minimizes dependencies, and fits the established Rust architecture. Final
validation includes an automated declaration and dependency-license audit.

## Phase Status

| Phase | Version | Status | Checkpoint |
|---|---|---|---|
| A | 0.2.0-alpha.1 | complete | `184d1a4` |
| B | 0.2.0-alpha.2 | complete | `d7958d8` |
| C | 0.2.0-alpha.3 | in progress | not created |
| D | 0.2.0-alpha.4 | pending | not created |
| E | 0.2.0-beta.1 | pending | not created |
| F | 0.2.0-rc.1 | pending | not created |
| G | 0.2.0 | pending | not created |

## Baseline Gates

- `cargo fmt --all -- --check`: PASS (2026-07-23)
- `cargo check --workspace --all-targets`: PASS (2026-07-23)
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS (2026-07-23)
- `cargo test --workspace`: PASS (2026-07-23)

## Autonomous Decisions

- Persistence driver: `rusqlite` with bundled SQLite, executed through a
  dedicated blocking boundary as required by the runbook.
- Artifact format: canonical UTF-8 JSON with SHA-256 metadata and same-filesystem
  atomic rename.
- Public identifiers: UUIDv7 serialized as lowercase hyphenated text.

## Phase A Evidence

- Inputs: accepted ADR, field mapping, state transition matrix, two SHA-256
  golden vectors and ten real-machine serial samples.
- Implementation: `cceb861`; tests: `a254efd`; docs: `be572e8`;
  checkpoint: `184d1a4`.
- Dedicated gates: `bash scripts/verify-hardware-snapshot-contract.sh` PASS;
  `cargo test --test hardware_snapshot_contract` PASS (4 tests).
- Unified gates: fmt, workspace all-target check, clippy with warnings denied and
  workspace tests all PASS after the formatting retry recorded below.

## Phase B Evidence

- Migration/schema commit: `332dd7e`; store/artifact implementation: `259e9ba`;
  tests: `7d92f7c` (format-only follow-ups `d5a87f2`, `3182689`).
- `cargo test -p hw-inventory`: PASS (10 tests: 1 unit, 9 integration).
- `cargo clippy -p hw-inventory --all-targets -- -D warnings`: PASS.
- The tests cover migration idempotence and V0 upgrade, future-version refusal,
  WAL/foreign keys/indexes, immutable rows, typed property projection, pagination,
  artifact round-trip, same-size tampering, missing files, transaction rollback,
  temporary/renamed orphan recovery, path traversal and mode 0700/0600.
- SQLite work is isolated in `spawn_blocking`; the complete report exists only as
  an immutable checked JSON artifact, while database queries use relation tables.

## Phase C Evidence

- Canonicalizer/probe implementation: `7fdb5d9`; fixtures/tests and benchmark
  tool: `19c2d08`; baseline/failure docs: `a0b5049`.
- `cargo test -p hw-inventory --test quick_probe`: PASS (7 tests).
- `cargo clippy -p hw-inventory --all-targets -- -D warnings`: PASS.
- Fixtures prove byte-deterministic ordering, whitespace/case normalization,
  duplicate removal, placeholder filtering, source failure vs trusted absence,
  random/virtual MAC and software-renderer exclusion. Physical network changes
  alter both IDs; kernel/firmware/driver changes alter only configuration; hot
  and network runtime fields alter neither. Existing bindid v1 tests remain green.
- Ten real-machine samples used `cargo run --quiet --example
  quick_probe_baseline -- 10` with two-second source timeouts. All were core
  complete with 7 identity records and 4 warnings. Wall times were
  `2211, 2223, 2211, 2205, 2207, 2206, 2223, 2202, 2198, 2196` ms; median 2206.5
  ms and nearest-rank P95 2223 ms. Raw evidence is in
  `docs/hardware-snapshot-quick-probe-baseline.csv`.
- Dependency/runtime audit: quick probe imports only existing probe/source APIs,
  executes a finite awaited sequence, and has no udev/netlink listener, daemon,
  monitor dependency, detached task or active source call after return.

## Performance Evidence

Phase A records real-machine serial samples and a reproducible delayed-fixture
baseline. Phase D records serial/concurrent comparisons and the enforced
regression threshold.

Phase A real-machine serial baseline used `cargo run --quiet --example
scan_baseline -- 10` with a two-second per-source timeout. All ten observations
returned 45 devices, 10 warnings and `partial`. Wall samples in milliseconds:
`6968, 6904, 6887, 6939, 6983, 6965, 6936, 6936, 6918, 5972`. Median is 6936 ms;
nearest-rank P95 is 6983 ms. The warm tenth sample is retained rather than
discarded. The raw data is in `docs/hardware-snapshot-performance-baseline.csv`.

## Acceptance Evidence

The 22 acceptance criteria are populated with command or test evidence during
phase G release validation.

## Gate Failure Ledger

- Phase A checkpoint attempt 1: `cargo fmt --all -- --check` exited 1 because
  rustfmt condensed `IdentityCoverage::core_complete`. Root cause was source
  formatting only; applied `cargo fmt --all`, then reran the full phase gates.
- Phase C implementation attempt 1: `cargo check -p hw-inventory --all-targets`
  exited 101 because `?` was applied to a borrowed `Option<&str>` in the
  canonical record iterator. Replaced it with explicit `as_ref().and_then`,
  added canonicalizer regression coverage, and reran the targeted gates.
