# Open-Source Release Prep Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prepare `qurbrix-hwinfo` for its first GitHub open-source release by adding dual LICENSE files, an English CONTRIBUTING guide, a CI workflow (fmt/clippy/test), a tag-triggered release workflow that builds three Linux binaries, and matching README additions in both English and Chinese.

**Architecture:** All changes are additive documentation and CI configuration. No source, `Cargo.toml`, crate structure, CLI semantics, or schema is touched. Tag `v*` fires the release workflow, which runs a 3-target matrix (native cargo for `x86_64`, `cross` for `aarch64` and `loongarch64`), packages each binary as `qurbrix-hw-<version>-<target>.tar.gz`, and uploads all archives plus `SHA256SUMS` to the corresponding GitHub Release.

**Tech Stack:** GitHub Actions, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `cross-rs/cross`, `softprops/action-gh-release@v2`, Rust stable, `cargo`, Markdown.

## Global Constraints

- Target GitHub repo: `BaekRui/qurbrix-hwinfo`.
- Dual license: `MIT OR Apache-2.0`. Copyright line: `Copyright (c) 2026 BaekRui`.
- Default branch: `master` (not `main`).
- Do not modify `Cargo.toml`, `src/`, or any file under `crates/`.
- Do not add `dependabot.yml`, `cargo-audit`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue/PR templates, DCO, or CLA — those are explicitly out of scope.
- `CONTRIBUTING.md` is English only; no `CONTRIBUTING.zh-CN.md`.
- Prebuilt binaries are glibc dynamically-linked only. No musl static build. No Windows or macOS targets.
- Cargo.toml already declares `publish = false`; keep it that way.
- Every task ends with `git add <specific paths>` and `git commit -m "<type>: ..."` — no `git add .` or `git add -A`.

---

## Task 1: Dual LICENSE files

**Files:**
- Create: `LICENSE-MIT`
- Create: `LICENSE-APACHE`

**Interfaces:**
- Consumes: nothing.
- Produces: two license files that Task 5's README `## License` section links to and that Task 4's release workflow copies into every release tarball.

- [ ] **Step 1: Verify files don't already exist**

Run: `ls LICENSE-MIT LICENSE-APACHE 2>&1`
Expected: both paths reported as `No such file or directory`.

- [ ] **Step 2: Create `LICENSE-MIT` with the standard MIT template**

Write the following content, byte-for-byte, to `LICENSE-MIT`:

```
MIT License

Copyright (c) 2026 BaekRui

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 3: Fetch canonical Apache 2.0 text into `LICENSE-APACHE`**

Run:
```bash
curl -fsSL https://www.apache.org/licenses/LICENSE-2.0.txt -o LICENSE-APACHE
```

The canonical text starts with a blank line then `                                 Apache License` and ends with `END OF TERMS AND CONDITIONS` followed by the appendix "How to apply the Apache License to your work" and the copyright placeholder block. Do **not** append a NOTICE file; the project has no embedded third-party code requiring attribution. Do **not** fill in the placeholder copyright block at the bottom of the appendix — leave the appendix template intact (this matches convention in `rust-lang/rust` and most rust-lang-org crates).

- [ ] **Step 4: Verify both files**

Run:
```bash
head -n 3 LICENSE-MIT
head -n 5 LICENSE-APACHE
tail -n 1 LICENSE-APACHE
wc -l LICENSE-MIT LICENSE-APACHE
```
Expected:
- `LICENSE-MIT` line 1 is `MIT License`, line 3 is `Copyright (c) 2026 BaekRui`.
- `LICENSE-APACHE` contains `Apache License` on line 2 and `Version 2.0, January 2004` on line 3.
- `LICENSE-APACHE` last non-empty line is the appendix's `limitations under the License.` line.
- `LICENSE-MIT` line count is 21; `LICENSE-APACHE` line count is 202.

If line counts drift by ±1 due to trailing-newline handling, that's acceptable, but content must match.

- [ ] **Step 5: Commit**

```bash
git add LICENSE-MIT LICENSE-APACHE
git commit -m "chore: add MIT and Apache-2.0 license files"
```

---

## Task 2: `CONTRIBUTING.md`

**Files:**
- Create: `CONTRIBUTING.md`

**Interfaces:**
- Consumes: the `type(scope): summary` commit style already established in git history.
- Produces: file referenced by Task 5's README `## Contributing` section.

- [ ] **Step 1: Verify file doesn't exist**

Run: `ls CONTRIBUTING.md 2>&1`
Expected: `No such file or directory`.

- [ ] **Step 2: Create `CONTRIBUTING.md`**

Write the following content to `CONTRIBUTING.md`:

````markdown
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
````

- [ ] **Step 3: Verify**

Run: `head -n 1 CONTRIBUTING.md && grep -c '^## ' CONTRIBUTING.md`
Expected: first line is `# Contributing to qurbrix-hwinfo`; count of `## ` headings is `4` (Local development, Pull requests, Issues, License).

- [ ] **Step 4: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs: add CONTRIBUTING guide"
```

---

## Task 3: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: workspace `cargo fmt`, `cargo clippy`, `cargo test` — must all pass on `ubuntu-latest` with no root and no hardware. If any workspace test turns out to require real hardware (unlikely per current fixture-driven layout), the fix belongs in the failing test's crate, not in this workflow.
- Produces: CI status badge URL `https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml/badge.svg` used by Task 5.

- [ ] **Step 1: Verify local checks pass before wiring CI**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all three exit `0`. If any fail, stop and report — CI will fail identically and this task must not proceed until the underlying issue is fixed (fixing tests/fmt is out of scope for this plan but the plan cannot ship a red CI).

- [ ] **Step 2: Create workflow directory and file**

Run: `mkdir -p .github/workflows`

Write the following content to `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [master]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  ci:
    name: fmt / clippy / test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install stable toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - uses: Swatinem/rust-cache@v2

      - name: cargo fmt --check
        run: cargo fmt --all -- --check

      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: cargo test
        run: cargo test --workspace
```

- [ ] **Step 3: YAML syntax check**

Run:
```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo OK
```
Expected: `OK`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add fmt/clippy/test workflow"
```

---

## Task 4: Release workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `LICENSE-MIT`, `LICENSE-APACHE` (from Task 1), `README.md`, `README.zh-CN.md` — copied into each tarball.
- Produces: three release artifacts `qurbrix-hw-<version>-<target>.tar.gz` (for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `loongarch64-unknown-linux-gnu`) plus `SHA256SUMS`, attached to the GitHub Release created for each `v*` tag. Release URL pattern `https://github.com/BaekRui/qurbrix-hwinfo/releases` is used by Task 5's Installation section.

- [ ] **Step 1: Create the workflow file**

Write the following content to `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: build ${{ matrix.target }}
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            use_cross: false
          - target: aarch64-unknown-linux-gnu
            use_cross: true
          - target: loongarch64-unknown-linux-gnu
            use_cross: true
    steps:
      - uses: actions/checkout@v4

      - name: Install stable toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build (cargo)
        if: ${{ !matrix.use_cross }}
        run: cargo build --release --target ${{ matrix.target }}

      - name: Build (cross)
        if: matrix.use_cross
        run: cross build --release --target ${{ matrix.target }}

      - name: Package
        run: |
          set -euo pipefail
          VERSION="${GITHUB_REF_NAME#v}"
          STAGE="qurbrix-hw-${VERSION}-${{ matrix.target }}"
          mkdir -p "${STAGE}"
          cp "target/${{ matrix.target }}/release/qurbrix-hw" "${STAGE}/"
          cp README.md README.zh-CN.md LICENSE-MIT LICENSE-APACHE "${STAGE}/"
          tar -czf "${STAGE}.tar.gz" "${STAGE}"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: qurbrix-hw-${{ matrix.target }}
          path: qurbrix-hw-*.tar.gz
          if-no-files-found: error

  release:
    name: publish release
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v4
        with:
          path: dist
          merge-multiple: true

      - name: Generate SHA256SUMS
        working-directory: dist
        run: sha256sum *.tar.gz > SHA256SUMS

      - name: Publish GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/*.tar.gz
            dist/SHA256SUMS
          generate_release_notes: true
```

- [ ] **Step 2: YAML syntax check**

Run:
```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))" && echo OK
```
Expected: `OK`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add tag-triggered release workflow"
```

- [ ] **Step 4: End-to-end verification (deferred to post-plan)**

Live verification of the release workflow requires a tag push against the real GitHub repo (`BaekRui/qurbrix-hwinfo` — to be created before first release). The concrete post-plan steps are:

1. Ensure the GitHub repo exists and `master` is pushed.
2. `git tag v0.1.0-rc1 && git push origin v0.1.0-rc1`.
3. On GitHub, watch the `Release` workflow — all three matrix jobs must succeed and the release job must create a release attached to `v0.1.0-rc1` containing three `.tar.gz` files and a `SHA256SUMS`.
4. Download `qurbrix-hw-0.1.0-rc1-x86_64-unknown-linux-gnu.tar.gz`, extract, and run:
   ```bash
   ./qurbrix-hw --help
   sudo ./qurbrix-hw scan --format json | head
   ```
   Both must exit `0` and produce expected output.
5. Confirm `sha256sum -c SHA256SUMS` in the extracted directory reports OK for all three archives.

Failure modes to watch for:
- `loongarch64` may fail if `cross` doesn't ship a Docker image for that target. Fallback: pin `cross` to a version with a working `loongarch64` image (check the `cross-rs/cross` releases page), or add a `Cross.toml` with an explicit image. Do not attempt fallback in this plan — file a follow-up issue.
- `aarch64` glibc mismatch is unlikely because `cross`'s default aarch64 image uses a widely-compatible glibc; still worth eyeballing.

If step 3 fails, do **not** mark this task complete — either fix in the plan (yaml tweak) or open a follow-up.

---

## Task 5: README updates (English + Chinese)

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`

**Interfaces:**
- Consumes: everything from Tasks 1–4 — badges reference the CI workflow file path from Task 3 and Release page created by Task 4; License section links to `LICENSE-MIT`/`LICENSE-APACHE` from Task 1; Contributing section links to `CONTRIBUTING.md` from Task 2.
- Produces: final external-facing docs.

Existing sections must remain **unchanged in wording and order**. This task only inserts new sections at three specific locations in each file.

- [ ] **Step 1: Insert badges + Installation section in `README.md`**

Current `README.md:1-6` starts with:

```
# Qurbrix HW Info

Qurbrix HW Info is a set of Rust crates for collecting, parsing, normalizing, and reporting Linux hardware information. It turns command output, `/proc`, `/sys`, PCI, USB, DMI, display, power, and peripheral data into a typed `ScanReport` plus flat JSON, JSONL, summary, and table views.

Chinese documentation is available in [README.zh-CN.md](README.zh-CN.md).
```

Insert three badge lines and a blank line **between the title and the description paragraph**, so it becomes:

```markdown
# Qurbrix HW Info

[![CI](https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml/badge.svg)](https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/BaekRui/qurbrix-hwinfo)](https://github.com/BaekRui/qurbrix-hwinfo/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Qurbrix HW Info is a set of Rust crates for collecting, parsing, normalizing, and reporting Linux hardware information. It turns command output, `/proc`, `/sys`, PCI, USB, DMI, display, power, and peripheral data into a typed `ScanReport` plus flat JSON, JSONL, summary, and table views.

Chinese documentation is available in [README.zh-CN.md](README.zh-CN.md).
```

Then, immediately **before** the existing `## Build` heading, insert this `## Installation` section (blank line before and after):

````markdown
## Installation

### Prebuilt binaries

Download the latest release from
[GitHub Releases](https://github.com/BaekRui/qurbrix-hwinfo/releases). Pick
the archive matching your machine:

| Archive | Architecture |
|---|---|
| `qurbrix-hw-<version>-x86_64-unknown-linux-gnu.tar.gz` | 64-bit Intel/AMD |
| `qurbrix-hw-<version>-aarch64-unknown-linux-gnu.tar.gz` | 64-bit ARM |
| `qurbrix-hw-<version>-loongarch64-unknown-linux-gnu.tar.gz` | LoongArch64 |

Verify and install:

```bash
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf qurbrix-hw-<version>-<target>.tar.gz
sudo install -m 0755 qurbrix-hw-<version>-<target>/qurbrix-hw /usr/local/bin/
```

Prebuilt binaries are glibc dynamically-linked. They target the glibc shipped
by GitHub `ubuntu-latest` runners (currently 2.35+); older distros may need
to build from source.

### From source

```bash
cargo install --path .
```

````

- [ ] **Step 2: Append Contributing and License sections to `README.md`**

Immediately **after** the last existing bullet under `## Notes` (the line ending with `structured command output goes to stdout.`), append:

```markdown

## Contributing

Contributions are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) for local
setup, test commands, and commit conventions. Bug reports and feature
requests go to
[GitHub Issues](https://github.com/BaekRui/qurbrix-hwinfo/issues); code
changes come through pull requests.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
```

- [ ] **Step 3: Verify `README.md` structure**

Run:
```bash
grep -n '^## ' README.md
```
Expected output (exact order):
```
7:## Features
16:## Layout
32:## Runtime Requirements
44:## Installation
75:## Build
82:## Basic Usage
104:## Integration Contract
133:## Data Flow
141:## Notes
149:## Contributing
157:## License
```

Line numbers are indicative — the important checks are: `## Installation` comes before `## Build`, `## Contributing` and `## License` come after `## Notes`, all previously existing `##` headings are still present exactly once, and no heading was reworded.

- [ ] **Step 4: Insert badges + `## 安装` section in `README.zh-CN.md`**

Current `README.zh-CN.md:1-4` starts with:

```
# Qurbrix HW Info

Qurbrix HW Info 是一组用于 Linux 硬件信息采集、解析、归一化和输出的 Rust crate。项目把命令输出、`/proc`、`/sys`、PCI、USB、DMI、显示、电源和外设信息整理为 typed `ScanReport`，并提供 flat JSON、JSONL、summary 和 table 输出。
```

Insert the same three badge lines between the title and the description paragraph so it becomes:

```markdown
# Qurbrix HW Info

[![CI](https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml/badge.svg)](https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/BaekRui/qurbrix-hwinfo)](https://github.com/BaekRui/qurbrix-hwinfo/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#许可证)

Qurbrix HW Info 是一组用于 Linux 硬件信息采集、解析、归一化和输出的 Rust crate。项目把命令输出、`/proc`、`/sys`、PCI、USB、DMI、显示、电源和外设信息整理为 typed `ScanReport`，并提供 flat JSON、JSONL、summary 和 table 输出。
```

Note the License badge anchor is `#许可证` (Chinese heading), unlike the English file's `#license`.

Then, immediately **before** the existing `## 构建` heading, insert this `## 安装` section:

````markdown
## 安装

### 下载预编译二进制

去 [GitHub Releases](https://github.com/BaekRui/qurbrix-hwinfo/releases) 下载最新版本，
根据机器架构选择对应压缩包：

| 压缩包 | 适用架构 |
|---|---|
| `qurbrix-hw-<version>-x86_64-unknown-linux-gnu.tar.gz` | 64 位 Intel/AMD |
| `qurbrix-hw-<version>-aarch64-unknown-linux-gnu.tar.gz` | 64 位 ARM |
| `qurbrix-hw-<version>-loongarch64-unknown-linux-gnu.tar.gz` | LoongArch64 |

校验并安装：

```bash
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf qurbrix-hw-<version>-<target>.tar.gz
sudo install -m 0755 qurbrix-hw-<version>-<target>/qurbrix-hw /usr/local/bin/
```

预编译二进制为 glibc 动态链接版本，仅保证在不老于 GitHub `ubuntu-latest`
运行器所提供的 glibc（当前 2.35+）的发行版上运行；较老发行版请自行从源码构建。

### 从源码构建

```bash
cargo install --path .
```

````

- [ ] **Step 5: Append `## 贡献` and `## 许可证` sections to `README.zh-CN.md`**

Immediately **after** the last existing bullet under `## 注意事项` (the line ending with `结构化命令输出写入 stdout。`), append:

```markdown

## 贡献

欢迎贡献代码。本地开发环境、测试命令与提交约定见
[`CONTRIBUTING.md`](CONTRIBUTING.md)（英文）。缺陷和需求走
[GitHub Issues](https://github.com/BaekRui/qurbrix-hwinfo/issues)，
代码变更通过 pull request 提交。

## 许可证

按下列任一许可证发布，用户可自行选择：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)
  或 <https://www.apache.org/licenses/LICENSE-2.0>）
- MIT License（[LICENSE-MIT](LICENSE-MIT)
  或 <https://opensource.org/licenses/MIT>）

### 贡献者授权

除非贡献者明确声明，任何以 Apache-2.0 定义方式提交的贡献均按上述双许可证发布，
不附加任何额外条款。
```

- [ ] **Step 6: Verify `README.zh-CN.md` structure**

Run:
```bash
grep -n '^## ' README.zh-CN.md
```
Expected sections (exact order): `## 能力范围`, `## 目录结构`, `## 运行环境`, `## 安装`, `## 构建`, `## 命令行用法`, `## 集成合约`, `## 库用法`, `## 主要数据流`, `## 注意事项`, `## 贡献`, `## 许可证`. `## 安装` sits immediately before `## 构建`; `## 贡献` and `## 许可证` are the last two headings.

- [ ] **Step 7: Sanity-check symmetry between the two files**

Run:
```bash
grep -c '^## ' README.md
grep -c '^## ' README.zh-CN.md
```
Expected: both `11`.

Also run:
```bash
grep -c 'BaekRui/qurbrix-hwinfo' README.md
grep -c 'BaekRui/qurbrix-hwinfo' README.zh-CN.md
```
Expected: both `≥ 4` (badges + releases link + issues link). Values should match between the two files.

- [ ] **Step 8: Commit**

```bash
git add README.md README.zh-CN.md
git commit -m "docs: add badges, installation, contributing, license sections"
```

---

## Post-plan Verification

Once all five tasks are committed, run the full local check chain one more time as a smoke test:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); yaml.safe_load(open('.github/workflows/release.yml'))" && echo YAML-OK
ls LICENSE-MIT LICENSE-APACHE CONTRIBUTING.md .github/workflows/ci.yml .github/workflows/release.yml
git log --oneline -5
```

Expected: all commands exit `0`; the five files listed are all present; the last five commits are the five tasks in order.

End-to-end release verification (tag push, GitHub Release creation, `x86_64` binary smoke test) happens against the real repo after `BaekRui/qurbrix-hwinfo` exists — see Task 4 Step 4.
