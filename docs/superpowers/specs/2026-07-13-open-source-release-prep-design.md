# qurbrix-hw 开源发布准备设计方案

日期：2026-07-13

## 1. 背景

`qurbrix-hwinfo` 准备作为开源项目对外发布。当前仓库已有完整代码、workspace、双语 README（`README.md` 与 `README.zh-CN.md`），Cargo.toml 已声明 `license = "MIT OR Apache-2.0"`，但缺少发布一个开源项目所需的以下要素：

1. 兑现许可证声明的 `LICENSE-MIT` 与 `LICENSE-APACHE` 文件都不存在。
2. 没有 `CONTRIBUTING.md`，外部贡献者缺乏入门入口。
3. 没有 CI（fmt / clippy / test），也没有 release workflow，不能自动产出可下载的二进制。
4. README 缺 badges、安装指引、贡献与许可证章节。

对外用户目前只能从源码构建；对于希望在真实机器上直接跑 `qurbrix-hw` 的运维/测试场景，缺乏预编译分发。

## 2. 目标

1. 补齐双许可（MIT OR Apache-2.0）所需的 LICENSE 文件。
2. 新增 `CONTRIBUTING.md`，说明如何本地构建、跑测试、提交 PR。
3. 新增 GitHub Actions CI workflow，PR 与 push 触发 fmt/clippy/test。
4. 新增 GitHub Actions release workflow，tag `v*` 触发构建三份 Linux 二进制并发到 GitHub Release：
   - `x86_64-unknown-linux-gnu`
   - `aarch64-unknown-linux-gnu`
   - `loongarch64-unknown-linux-gnu`
5. 扩展中英 README，新增 badges、Installation、Contributing、License 章节；保留原有章节结构。

## 3. 非目标

1. 不发布到 crates.io（保持 `publish = false`）。
2. 不产出 musl 静态二进制。首次发布只提供 glibc 动态链接版本；覆盖更老发行版可后续再加。
3. 不新增 Windows / macOS 目标——项目就是 Linux only。
4. 不引入 DCO、CLA、Contributor Agreement 等法务流程。
5. 不引入 `dependabot.yml`、`cargo-audit`、SECURITY.md、CODE_OF_CONDUCT.md、issue/PR 模板等开源治理配置——需要时增量加。
6. 不改动现有 crate 结构、CLI 语义、schema。
7. 不为 CONTRIBUTING 出中文版——首版只有英文，需要时再加。

## 4. 仓库路径与许可证

- GitHub 目标仓库：`BaekRui/qurbrix-hwinfo`（首次发布前创建）。
- 许可证：`MIT OR Apache-2.0` 双许可，与 Cargo.toml 已声明的一致。
- copyright 行：`Copyright (c) 2026 BaekRui`。

## 5. 新增文件

### 5.1 `LICENSE-MIT`

MIT License 官方模板全文；顶部 copyright 行为 `Copyright (c) 2026 BaekRui`。

### 5.2 `LICENSE-APACHE`

Apache License 2.0 官方模板全文；不附 NOTICE 文件（当前项目没有内嵌需要归属的第三方代码）。

### 5.3 `CONTRIBUTING.md`

英文，单文件，覆盖以下内容：

1. Quick start：`cargo check --workspace`、`cargo test --workspace`、`cargo fmt`、`cargo clippy --workspace --all-targets -- -D warnings`。
2. 提交要求：clippy 零 warning、rustfmt 干净、为新逻辑加 fixture 驱动的测试（沿用 `hw-testdata` 模式）。
3. Commit message：沿用现有 `type(scope): summary` 风格（`feat`, `fix`, `docs`, `chore` 等）。不新增强制约束。
4. Issue / PR 走 `https://github.com/BaekRui/qurbrix-hwinfo/issues` 与 pull requests。
5. 许可证声明：贡献即以 `MIT OR Apache-2.0` 双许可发布。

### 5.4 `.github/workflows/ci.yml`

触发：`push` 到 `master`、`pull_request`。

单 job `ci`，`runs-on: ubuntu-latest`，步骤：

1. `actions/checkout@v4`
2. 安装 stable toolchain（`dtolnay/rust-toolchain@stable`），components: `rustfmt`, `clippy`。
3. `Swatinem/rust-cache@v2`
4. `cargo fmt --all -- --check`
5. `cargo clippy --workspace --all-targets -- -D warnings`
6. `cargo test --workspace`

已知风险：若某些测试实际读 `/sys` 或依赖真硬件而非 fixture，在无 root/无相关设备的 CI runner 上会失败。implementation plan 阶段会先在 fork 上跑一次 CI，观察是否需要过滤或标 `#[ignore]`。截至目前从代码结构（`hw-testdata`、fixture 目录）判断，测试都是 fixture 驱动。

### 5.5 `.github/workflows/release.yml`

触发：push tag 匹配 `v*`。

两个 job：

**Job `build`**（matrix）：

| target | runner | 构建方式 |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ubuntu-latest | `cargo build --release --target …` |
| `aarch64-unknown-linux-gnu` | ubuntu-latest | `cross build --release --target …` |
| `loongarch64-unknown-linux-gnu` | ubuntu-latest | `cross build --release --target …` |

步骤：

1. `actions/checkout@v4`
2. 安装 stable toolchain，`targets: <matrix.target>`。
3. `Swatinem/rust-cache@v2`（含 `key: ${{ matrix.target }}`）。
4. 对 arm64 / loongarch64：`cargo install cross --git https://github.com/cross-rs/cross`。
5. 构建：native target 用 `cargo`，其余用 `cross`。
6. 打包：`qurbrix-hw-<version>-<target>.tar.gz` 内含 `qurbrix-hw` 二进制、`README.md`、`README.zh-CN.md`、`LICENSE-MIT`、`LICENSE-APACHE`。
7. 上传 artifact。

**Job `release`**（`needs: build`）：

1. `actions/download-artifact@v4` 下载全部产物到同一目录。
2. 生成 `SHA256SUMS`（`sha256sum *.tar.gz > SHA256SUMS`）。
3. `softprops/action-gh-release@v2` 创建/更新 tag 对应 Release，附上全部 `*.tar.gz` 与 `SHA256SUMS`，`generate_release_notes: true`。

版本号：从 tag 提取（`${GITHUB_REF_NAME#v}`），用于 tar.gz 命名；不要求同步修改 Cargo.toml `version`（这是首次发布准备，本方案不引入版本管理策略）。

## 6. 修改文件

### 6.1 `README.md`

在文档顶部（第一行标题之后）加 3 个 Shields.io badge：

```markdown
[![CI](https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml/badge.svg)](https://github.com/BaekRui/qurbrix-hwinfo/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/BaekRui/qurbrix-hwinfo)](https://github.com/BaekRui/qurbrix-hwinfo/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
```

在 `## Build` 章节**之前**插入 `## Installation`：

- *Prebuilt binaries*：指向 Releases 页；列出 3 个 target 与其对应的物理机架构；给一段解压 + `sudo install -m 0755 qurbrix-hw /usr/local/bin/` 的示例；说明校验 `SHA256SUMS`；点一句 “glibc dynamic binaries — targeting glibc as shipped on GitHub `ubuntu-latest` runners (currently 2.35+); older distros may need to build from source”。
- *From source*：`cargo install --path .`，或 `cargo build --release` 后手动安装。

在文末 `## Notes` 之后追加两节：

- `## Contributing`：一段话，指向 `CONTRIBUTING.md` 与 issues/PR 页。
- `## License`：说明双许可 `MIT OR Apache-2.0`；照 rust-lang 项目常用范式给出选择性声明；指向 `LICENSE-MIT`、`LICENSE-APACHE`。

现有 Features / Layout / Runtime Requirements / Build / Basic Usage / Integration Contract / Data Flow / Notes 章节全部保留不动，仅插入新章节。

### 6.2 `README.zh-CN.md`

结构与英文完全对称：顶部同样 3 个 badge（复用相同 URL）；在 `## 构建` 之前插入 `## 安装` 章节；在 `## 注意事项` 之后追加 `## 贡献` 与 `## 许可证` 章节。术语沿用当前中文 README 已有的措辞。`## 贡献` 章节指向英文 CONTRIBUTING.md。

## 7. 影响范围

- `Cargo.toml`：不改。
- 现有 `src/`、`crates/`：不改。
- 现有 `docs/`：本 spec 与后续 plan 之外不改。
- 现有 CLI 语义、schema：不改。

## 8. 风险与已知限制

1. **CI 首跑可能红**：若发现某些测试实际依赖真硬件，需要在 implementation plan 阶段决定是过滤、标 `#[ignore]`，还是提供 fixture。
2. **glibc 版本兼容**：产物只保证在 glibc ≥ ubuntu-latest 提供的版本上运行；老发行版需自行编译。README 会明说。
3. **`cross` 供应链**：release workflow 从 `cross-rs/cross` 仓库 install。这是社区标准做法；若担心可 pin 具体 tag，本方案首版不 pin，未来治理阶段再加。
4. **loongarch64 覆盖度**：`cross` 官方支持 `loongarch64-unknown-linux-gnu`，但构建镜像相对小众，若失败需要 fallback 到自定义 Docker image。plan 阶段验证。
5. **tag 与 Cargo.toml 版本脱钩**：本方案不强制两者一致。首次发布前用户可自行同步。

## 9. 验证计划

在 implementation plan 中：

1. 本地 `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test --workspace` 全部通过。
2. 在个人 fork 或 branch 推一次 push，观察 `ci.yml` 全绿。
3. 打一个测试 tag（如 `v0.1.0-rc1`），观察 `release.yml` 三份二进制成功产出并出现在 Release 页；下载 `x86_64` 版本本地跑 `qurbrix-hw --help` 与 `sudo qurbrix-hw scan --format json` 冒烟。
4. README badges 在 GitHub 上正确渲染。
