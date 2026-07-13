# Contributing to qurbrix-hwinfo

Thanks for your interest. This project targets Linux hardware information
collection; contributions that improve parsers, probes, output formats,
tests, or docs are all welcome.

## Local development

Prerequisites: a stable Rust toolchain (matching the one on GitHub Actions
`stable`, typically the current stable). The workspace uses Rust 2021
edition.

Common commands:

```bash
cargo check --workspace
cargo test --workspace
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
```

Workspace tests are fixture-driven through helpers under `crates/hw-testdata`
and do not require root or specific hardware.

## Pull requests

Before opening a PR:

1. `cargo fmt` — CI enforces `cargo fmt --all -- --check`.
2. `cargo clippy --workspace --all-targets -- -D warnings` — clippy runs with
   warnings promoted to errors in CI.
3. `cargo test --workspace` — all tests pass.
4. For new parsers, probes, or output paths: add a fixture-driven test in
   the same style as neighboring code (`hw-testdata` helpers).

Commit messages follow the existing `type(scope): summary` style used
throughout git history — e.g. `feat(bindid): ...`, `fix(network): ...`,
`docs: ...`, `chore: ...`. There's no strict enforcement, but consistency
keeps release notes readable.

## Issues

Bug reports and feature requests go to
<https://github.com/BaekRui/qurbrix-hwinfo/issues>. Include:

- What you ran (command line, environment).
- What you expected.
- What actually happened (paste `stderr` and relevant `stdout` fields).
- Distro, kernel version, and — if relevant — hardware.

## License

By contributing you agree that your contributions will be licensed under the
project's dual license, `MIT OR Apache-2.0`, as declared in `Cargo.toml` and
the top-level `LICENSE-MIT` and `LICENSE-APACHE` files.
