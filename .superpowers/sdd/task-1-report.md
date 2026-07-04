# Task 1 Report: Workspace Reshape and Crate Skeleton

## Status
Implemented Task 1 in `/home/qur/Desktop/20260704_qurbrix-hw/qurbrix-hwinfo/.claude/worktrees/general-hw-scanner` using TDD.

## TDD Evidence
- RED: Created `tests/facade_exports.rs` with `facade_exports_schema_version` before implementation.
- RED verification: `cargo test --test facade_exports` failed initially because the target checkout was still treated as part of the parent workspace and had no local `[workspace]` declaration; this confirmed the scaffold/workspace reshape was required before the facade export could compile.
- GREEN: Added the workspace skeleton, facade export, output schema function, CLI crate, probe crate, testdata crate, collector placeholder, and model placeholder.
- GREEN verification: `cargo test --test facade_exports` passed with 1 test.

## Files Changed
- `Cargo.toml`
- `src/lib.rs`
- `crates/hw-probe/Cargo.toml`
- `crates/hw-probe/src/lib.rs`
- `crates/hw-output/Cargo.toml`
- `crates/hw-output/src/lib.rs`
- `crates/hw-cli/Cargo.toml`
- `crates/hw-cli/src/main.rs`
- `crates/hw-testdata/Cargo.toml`
- `crates/hw-testdata/src/lib.rs`
- `crates/hw-collect/Cargo.toml`
- `crates/hw-collect/src/lib.rs`
- `crates/hw-model/Cargo.toml`
- `crates/hw-model/src/lib.rs`
- `tests/facade_exports.rs`
- `Cargo.lock`

## Verification
- `cargo test --test facade_exports`: passed, 1 passed, 0 failed.
- `cargo check --workspace`: passed.

## Deviations / Notes
- The brief specified `members = ["crates/*"]`; this was retained.
- Because existing workspace members under `crates/*` still compile against earlier symbols, minimal compatibility placeholders were added for `SystemInfo`, `collect_system_info`, and `refresh_system_info` so `cargo check --workspace` succeeds while preserving the Task 1 facade outputs.
- `zbus = "4"` was added to workspace dependencies because existing `crates/hw-api` inherits it from the workspace root.
- `cargo check --workspace` emits one warning from existing `crates/hw-api/src/lib.rs`: unexpected cfg condition value `dbus`. This warning is not introduced by the Task 1 files directly, and the command exits successfully.

## Commit
Pending at report creation time; commit will use message `chore: scaffold scanner workspace` with the requested co-author trailer.
