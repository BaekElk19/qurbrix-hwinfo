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
| C | 0.2.0-alpha.3 | complete | `993efcf` |
| D | 0.2.0-alpha.4 | complete | `0be142d` |
| E | 0.2.0-beta.1 | complete | `e81c4db` |
| F | 0.2.0-rc.1 | complete | `ab6bebd` |
| G | 0.2.0 | release validation complete | version checkpoint pending |

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
- Retention deletion: remove the relational snapshot in one transaction, queue
  its immutable artifact path, then delete/retry the file outside the
  transaction. This protects the current snapshot and keeps filesystem failure
  observable without lengthening the SQLite write lock.
- License audit: retain `MIT OR Apache-2.0`; no source was copied or adapted from
  GPL projects. All 11 workspace packages declare that expression. The only
  locked dependency expression containing `LGPL` is `r-efi`'s disjunctive
  `MIT OR Apache-2.0 OR LGPL-2.1-or-later`, for which the permissive branch is
  selected. There are no missing license declarations or GPL-family-only
  dependency expressions among 131 locked packages.

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

## Phase D Evidence

- Source cache/semaphore/cancellation: `5125c4a`; deterministic probe execution:
  `a62063f`; tests/benchmarks: `d06f319`; graph and initial measurements:
  `ebf4eda`; prepared-statement batching: `b7a2aca`; single-core evidence:
  `723b920`.
- Dedicated gates: `cargo test -p hw-source --test cache` PASS (3 tests),
  `cargo test -p hw-collect` PASS (13 tests including 6 execution tests),
  `cargo test -p hw-inventory --test store` PASS (9 tests), and targeted clippy
  with warnings denied PASS.
- Fixture gates prove serial/concurrent report equality after excluding elapsed
  measurement, command peak enforcement, same-scan `lspci` dedup, preserved kind
  filters, deadline cancellation with zero residual source calls, and more than
  25% delayed-fixture speedup.
- Real-machine 10-round paired P95: serial 6490 ms, concurrent 2157 ms, a 66.76%
  reduction. Every round preserved scan status, device count and warning count;
  each concurrent run had 47 cache hits and peak external concurrency 4. Raw
  data: `docs/hardware-snapshot-full-scan-performance.csv`.
- Resource-constrained command ceiling 1: 10 rounds at 5433-5452 ms, peak exactly
  1, which is 13.96% faster than the regular serial median and therefore does not
  exceed the 10% regression limit. Raw data:
  `docs/hardware-snapshot-constrained-performance.csv`.
- True single-core validation used `taskset -c 0
  target/debug/examples/scan_performance 10`. Serial P95 was 5060 ms and
  concurrent P95 was 4328 ms (14.47% faster); all equivalence columns passed.
  Raw data: `docs/hardware-snapshot-single-core-performance.csv`.
- Correctness remains dominant: only explicit read-only commands are cached,
  merge order remains the original probe order, command children are
  `kill_on_drop`, and SQLite publication still occurs outside scanning in one
  short transaction using cached prepared statements.

## Phase E Evidence

- State/history/lease primitives: `a1edfa6`; service state machine and thin
  `full_scan()` wrapper: `3f492e7`; integration tests: `5913462`; retry evidence:
  `2fc775a`.
- Dedicated gates: `cargo test -p hw-inventory --test service` PASS (12 tests),
  complete `cargo test -p hw-inventory` PASS (29 tests), and inventory clippy
  with warnings denied PASS.
- Covered transitions: first publish, unchanged reuse, force, zero TTL,
  configuration change, physical change/new bindid, quick failure fallback,
  full failure retaining current, partial publish/reject, incomplete core reject,
  tampered current replacement, eight concurrent callers sharing one publish,
  expired lease recovery, stale running probe recovery, and DB writability while
  full scan is blocked.
- The scan lease is acquired in a short `BEGIN IMMEDIATE` transaction and held as
  a row, not a write transaction. Publication atomically writes snapshot/device/
  artifact/lifecycle/current state and full probe completion. Failures return an
  explicit error rather than an old ID; the previous artifact remains readable.

## Phase F Evidence

- Diff/export Rust API: `42d6bb2`; facade and CLI implementation: `303a146`;
  contract tests: `68197b2`; English/Chinese docs: `f14aa02`.
- Dedicated gates: `cargo test -p hw-cli` PASS (13 argument/permission tests),
  inventory store tests PASS (10), facade tests PASS (2), CLI contract tests PASS
  (4), README smoke PASS (1), and workspace clippy with warnings denied PASS.
- The top-level crate exports type-safe store, quick/full/ensure, query, diff and
  export APIs. CLI provides `snapshot ensure/show/list/diff/export`, bounded
  pagination, stable ordering, caller-selected state/output paths, no-overwrite
  export and `qurbrix.hw.snapshot.cli.v1` stdout JSON.
- CLI error contract: diagnostics only on stderr; exit 0 success, 1 parse or
  serialization, 2 scan/policy, 4 permission, 5 not found, 6 storage/integrity,
  124 lease timeout. Snapshot readers do not require root; ensure does. Existing
  scan/list/schema/bindid argument, output and exit tests remain unchanged/green.
- Both READMEs cover the on-demand/non-monitor boundary, Rust and CLI examples,
  default/override paths, modes 0700/0600, integrity behavior, export, cleanup,
  schema and exit codes. Smoke tests execute documented snapshot help/list paths.

## Phase G Evidence

- Retention/health implementation: `11021c0`; maintenance tests: `3eaaa55`;
  maintenance CLI: `a930f5f`; CLI tests: `53719e0`; English/Chinese docs:
  `df6cc21`; release/growth validation: `2efa3f7`.
- Dedicated gate `cargo test -p hw-inventory --test maintenance -- --nocapture`:
  PASS (5 tests). It proves current/pinned/unuploaded/recent protection, eligible
  cleanup ordering, artifact deletion retry, orphan/corruption health detection,
  WAL checkpoint behavior, bounded multi-snapshot growth and a 1000-device
  transaction/query fixture.
- Maintenance contract tests in `hw-cli` and the root CLI suite pass for health,
  dry-run/prune, pin/unpin and mark-uploaded JSON behavior. Existing verified
  snapshot diff/export tests remain green.
- Growth fixture: 20 snapshots/60 devices use a 233,472-byte SQLite database and
  33,440 artifact bytes; checkpoint reports zero busy/log frames after the
  operation. The 1000-device fixture published in 312 ms, queried in 7 ms and
  used a 1,314,816-byte database. Raw data is in
  `docs/hardware-snapshot-database-performance.csv`.
- G checkpoint-precondition unified gates all PASS: `cargo fmt --all -- --check`,
  `cargo check --workspace --all-targets`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and `cargo test --workspace`.
- `cargo build --release --locked --offline`: PASS for the only installed target,
  `x86_64-unknown-linux-gnu`. The unstripped PIE is 12,945,088 bytes; bundled
  SQLite introduces no dynamic SQLite dependency. Other architectures remain
  covered by repository CI configuration but were not locally compiled because
  no other Rust target is installed; fixture tests provide architecture-neutral
  behavior evidence as allowed by 17.6.
- `scripts/check-hardware-snapshot-release.sh 0.2.0-rc.1`: PASS, covering exact
  workspace version/license declarations, complete locked license metadata,
  rejection of GPL-family-only expressions, monitor/udev/netlink dependency
  absence, runtime-file absence, whitespace and phase-A contract evidence.

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

A final paired real-machine resource sample, measured with BusyBox `time -v`,
used 20,164 KiB maximum RSS, 138% aggregate CPU, 6.82 user seconds and 2.30 system
seconds. Its serial/concurrent wall times were 5154/1437 ms (72.12% reduction),
with external-process peak 4. Raw data is in
`docs/hardware-snapshot-resource-performance.csv`.

## Acceptance Evidence

| # | Result | Evidence |
|---:|:---:|---|
| 1 | PASS | `service::first_run_publishes_and_second_run_reuses` returns a loadable UUIDv7 snapshot. |
| 2 | PASS | The same service test asserts unchanged input reuses the ID and the full-scan call counter remains one. |
| 3 | PASS | `service::physical_and_configuration_changes_have_distinct_identity_semantics` creates new IDs and loads every old snapshot. |
| 4 | PASS | `store::transaction_failure_preserves_current_and_removes_new_artifact` and SQLite transaction publication prevent partial visibility. |
| 5 | PASS | `service::full_failure_retains_previous_snapshot_and_returns_error` reloads the prior current artifact. |
| 6 | PASS | `service::concurrent_callers_publish_once_and_share_id` gives eight callers one ID and one publication. |
| 7 | PASS | `store::migration_is_idempotent_and_has_required_schema` tests fresh, repeated and V0 upgrade paths; future versions are refused. |
| 8 | PASS | Full workspace regressions plus `cli_contract` preserve scan/schema/list output and mapped exit codes. |
| 9 | PASS | Phase-G `cargo test --workspace` completed with zero failures. |
| 10 | PASS | `README.md` and `README.zh-CN.md` document APIs, DB/artifact paths, 0700/0600 permissions and cleanup; README smoke passes. |
| 11 | PASS | Release script finds no udev/netlink/listener coupling; architecture and both READMEs state strictly on-demand behavior. |
| 12 | PASS | Release script finds no `qurbrix-monitor` source/manifest dependency; all workspace gates and the binary run without it. |
| 13 | PASS | `hardware_snapshot_contract` verifies V1 schema/golden fixture; service tests cover all accepted transition classes. |
| 14 | PASS | `execution::serial_and_parallel_reports_are_semantically_identical`, semaphore peak and cancellation tests pass. |
| 15 | PASS | Ten-round real P95 is 6490 ms serial versus 2157 ms concurrent, a 66.76% reduction; single-core P95 improves 14.47%. |
| 16 | PASS | `snapshot_id_contract_is_lowercase_uuid_v7`; store/service tests parse and sort independently generated IDs. |
| 17 | PASS | Existing bindid v1 golden tests pass; v2 recomputes full SHA-256 and physical-change fixtures change machine identity. |
| 18 | PASS | Quick-probe and service firmware/kernel/driver fixtures keep bindid and change configuration fingerprint/snapshot ID. |
| 19 | PASS | Migration provides normalized device/identifier/property/relation tables and indexes; store tests query projection without report JSON columns. |
| 20 | PASS | Store round-trip, same-size tamper, missing-file, transaction and orphan crash-point recovery tests verify every successful artifact SHA-256. |
| 21 | PASS | `publishes_projection_and_verified_artifact_atomically` compares paginated versioned upload DTOs and verified full artifacts. |
| 22 | PASS | Facade export tests and CLI show/list/diff/export contract tests operate on the same seeded snapshot IDs and artifact content. |

## Gate Failure Ledger

- Phase A checkpoint attempt 1: `cargo fmt --all -- --check` exited 1 because
  rustfmt condensed `IdentityCoverage::core_complete`. Root cause was source
  formatting only; applied `cargo fmt --all`, then reran the full phase gates.
- Phase C implementation attempt 1: `cargo check -p hw-inventory --all-targets`
  exited 101 because `?` was applied to a borrowed `Option<&str>` in the
  canonical record iterator. Replaced it with explicit `as_ref().and_then`,
  added canonicalizer regression coverage, and reran the targeted gates.
- Phase D test attempt 1: the chained source-cache/collector test command reached
  `cargo test -p hw-collect --test execution` and exited 101 because the new
  integration test implemented the async source trait without declaring the
  direct `async-trait` dev dependency. Added the workspace dev dependency and
  reran both dedicated test suites.
- Phase D test attempt 2: `cargo test -p hw-collect --test execution` exited 101
  because the performance fixture asserted a cache hit although its selected
  probes did not request the same command. Kept the performance assertion
  focused on latency and added a dedicated PCI/GPU `lspci` dedup regression test.
- Phase E test attempt 1: `cargo test -p hw-inventory --test service` exited 101
  at compile time because the optional quick report was moved into the leased
  full-scan helper before probe-history finalization. Changed the helper to
  borrow the report, preserving one canonical observation across both paths.
- Phase E targeted gate attempt 2: clippy exited 101 on a derivable `Default`
  implementation and `unwrap_or_else(BTreeSet::new)`. Applied the exact clippy
  recommendations (`derive(Default)`, `unwrap_or_default`) and reran clippy plus
  the full inventory test suite.
- Phase G license-audit attempt 1: two `cargo metadata --locked` calls attempted
  to download platform-specific locked packages and failed after DNS retries in
  the restricted sandbox. The authorized network retry downloaded the missing
  packages; `cargo metadata --locked --offline` then completed, and the release
  checker now intentionally uses offline mode for reproducibility.
