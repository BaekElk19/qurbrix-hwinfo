# Qurbrix HW Info

[![CI](https://github.com/BaekElk19/qurbrix-hwinfo/actions/workflows/ci.yml/badge.svg)](https://github.com/BaekElk19/qurbrix-hwinfo/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/BaekElk19/qurbrix-hwinfo)](https://github.com/BaekElk19/qurbrix-hwinfo/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#许可证)

Qurbrix HW Info 是一组用于 Linux 硬件信息采集、解析、归一化和输出的 Rust crate。项目把命令输出、`/proc`、`/sys`、PCI、USB、DMI、显示、电源和外设信息整理为 typed `ScanReport`，并提供 flat JSON、JSONL、summary 和 table 输出。

## 能力范围

- 采集 CPU、内存、BIOS/主板、显示器、存储、GPU、网络、PCI、USB、音频、蓝牙、输入设备、摄像头、电池、打印机和 CD-ROM 信息。
- 保留 source evidence，便于排错、回放和对比采集结果。
- 为 Rust 调用方提供 typed `ScanReport` 模型。
- 为脚本和 agent 提供 flat JSON、JSONL、summary 和 table 输出。
- 提供 `bindid` 轻量业务绑定 ID，用于常规读取和低频硬件绑定检查。
- 提供 fake source runner 与 fixture 驱动的 parser/probe 测试。
- 使用 UUIDv7、bindid v2、SHA-256 artifact 与 SQLite 历史保存不可变硬件快照。

## 目录结构

```text
.
├── src/                    # 顶层 qurbrix-hw 库，对外聚合采集和 schema helper
├── crates/
│   ├── hw-model/           # ScanReport、Device、DeviceKind 和属性模型
│   ├── hw-source/          # 命令与文件采集源，带超时控制
│   ├── hw-parser/          # lscpu、dmidecode、lsblk、xrandr、ip、lspci、lsusb 等解析逻辑
│   ├── hw-probe/           # 将解析结果转换为 Device 的分类 probe
│   ├── hw-collect/         # 采集编排，生成 ScanReport
│   ├── hw-inventory/       # 快照状态机、SQLite 投影和 artifact
│   ├── hw-bindid/          # 生成轻量业务绑定 ID
│   ├── hw-output/          # flat JSON、JSONL、summary、table 和 schema helper
│   ├── hw-cli/             # qurbrix-hw CLI 参数和命令
│   └── hw-testdata/        # parser fixture helper
└── Cargo.toml              # 顶层库 manifest
```

## 运行环境

目标环境是 Linux。采集质量取决于系统中可用的命令和权限：

- 基础信息：`lscpu`、`/proc/bus/input/devices`、`/proc/asound/cards`
- BIOS、主板、内存插槽：`dmidecode`，通常需要 root 权限
- 存储：`lsblk`
- 显示器/GPU：`xrandr`、`lspci`、`/sys/class/drm`
- 网络：`ip`

缺少部分命令时，采集器会尽量回退到可用的数据源，返回的字段可能减少。
`scan`、`summary`、`table`、`bindid` 和 `snapshot ensure` 需要 root 权限；
`schema`、`list-kinds` 和 `sources` 这类元数据命令不需要 root。

## 安装

### 下载预编译二进制

去 [GitHub Releases](https://github.com/BaekElk19/qurbrix-hwinfo/releases) 下载最新版本，
根据机器架构选择对应压缩包：

| 压缩包 | 适用架构 | glibc 下限 |
|---|---|---|
| `qurbrix-hw-<version>-x86_64-unknown-linux-gnu-glibc2.28.tar.gz` | 64 位 Intel/AMD | 2.28 |
| `qurbrix-hw-<version>-aarch64-unknown-linux-gnu-glibc2.28.tar.gz` | 64 位 ARM | 2.28 |
| `qurbrix-hw-<version>-loongarch64-unknown-linux-gnu-glibc2.36.tar.gz` | LoongArch64 | 2.36 |

校验并安装：

```bash
ARCHIVE="qurbrix-hw-0.1.4-x86_64-unknown-linux-gnu-glibc2.28" # 从上表选择
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf "${ARCHIVE}.tar.gz"
sudo install -m 0755 "${ARCHIVE}/qurbrix-hw" /usr/local/bin/
```

预编译二进制采用 glibc 动态链接。x86_64 与 aarch64 产物使用固定版本的
`cargo-zigbuild` 构建，需要 glibc **2.28** 或更高版本；LoongArch64 使用固定版本的
`cross` 工具链构建，需要 glibc **2.36** 或更高版本。每个 release job 都会
验证产物中最高的 `GLIBC_*` 符号不超过表中声明的下限。更老的发行版需要从
源码构建。

### 从源码构建

```bash
cargo install --path .
```

## 构建

```bash
cargo check --workspace
cargo test --workspace
```

## 命令总览

| 命令         | 需要 root | 用途                                        | 输出                                  |
|--------------|-----------|---------------------------------------------|---------------------------------------|
| `scan`       | 是        | 采集全部硬件并输出结构化报告                | JSON / JSONL / typed-JSON / summary-JSON |
| `summary`    | 是        | 按类别打印设备数量，便于人读                | 纯文本                                |
| `table`      | 是        | 以两列表格列出设备（可按类别过滤）          | 纯文本                                |
| `bindid`     | 是        | 从硬件生成轻量业务绑定 ID                   | JSON                                  |
| `list-kinds` | 否        | 列出扫描器支持的所有设备类别                | 文本或 JSON                           |
| `schema`     | 否        | 打印扫描输出的 schema 版本                  | 文本                                  |
| `sources`    | 否        | 列出采集过程用到的原始 source               | JSON                                  |
| `snapshot`   | 仅 ensure | 确保、查询、对比或导出硬件快照              | 稳定 JSON                             |

通用参数：`qurbrix-hw --help`、`qurbrix-hw <command> --help`、`qurbrix-hw --version`。

结构化结果写入 stdout，日志写入 stderr，方便脚本消费。

### `scan` — 结构化硬件报告

```bash
sudo qurbrix-hw scan --format json --pretty
```

参数：

- `--format json|jsonl|typed-json|summary-json`（默认 `json`）
  - `json`：flat schema，推荐外部程序消费
  - `jsonl`：一行一个设备，便于流式处理
  - `typed-json`：Rust 内部模型形状（可能变更，非稳定合约）
  - `summary-json`：`summary` 命令的 JSON 版
- `--pretty`：格式化 JSON
- `--kind <k>` / `--exclude-kind <k>`：可重复，如 `--kind cpu --kind memory`
- `--timeout 30s`：单个 source 的超时
- `--no-optional-sources`：跳过可选/较慢的 probe
- `--no-sources`：不在报告中输出原始 `sources` 段
- `--no-warnings`：抑制非致命 warning

示例（截断）：

```json
{
  "schema_version": "qurbrix.hw.scan.v2",
  "status": "complete",
  "summary": { "device_count": 1, "counts_by_kind": {"cpu": 1}, "warning_count": 0 },
  "devices": [
    {
      "id": "cpu:0",
      "kind": "cpu",
      "name": "AMD Ryzen 7 5800H with Radeon Graphics",
      "properties": { "data": { "cores": 8, "threads": 16, ... } }
    }
  ]
}
```

扫描状态：

- `complete`：扫描完成且没有重要 warning。
- `partial`：生成了可用报告，但部分数据源缺失、失败、超时或权限不足。
- `failed`：无法生成有效报告。

`partial` 仍返回退出码 `0`，方便脚本继续消费已有结果。

### `summary` — 设备数量速览

```bash
sudo qurbrix-hw summary
```

```text
Status: Partial
Devices: 65
Warnings: 5
audio: 1
battery: 1
bios: 1
cpu: 1
gpu: 1
memory: 2
storage: 1
...
```

### `table` — 表格视图

```bash
sudo qurbrix-hw table                # 全部设备
sudo qurbrix-hw table --kind storage # 指定类别
```

```text
KIND       ID                           NAME
storage    storage:dev:/dev/sda         VMware, VMware Virtual S
```

### `bindid` — 硬件绑定 ID

从主板/内存/存储/网络等信息生成稳定 ID，可用于授权绑定、遥测去重、机器盘点。
缺失的组件会显式列出，调用方可自行决定该 ID 是否满足业务需求。

```bash
sudo qurbrix-hw bindid --pretty
```

```json
{
  "schema_version": "qurbrix.hw.bindid.v1",
  "algorithm": "qurbrix-hw-bindid-sha1-hex16-v1",
  "status": "complete",
  "value": "a05173f4b72b4597",
  "required_kinds": ["system","motherboard","memory","storage","network"],
  "optional_kinds": ["gpu"],
  "covered_kinds": ["gpu","memory","motherboard","network","storage","system"],
  "missing_required_kinds": [],
  "missing_optional_kinds": []
}
```

`bindid` 输出 16 位小写 SHA1 十六进制前缀，用作轻量业务绑定 ID，
用于常规读取和低频硬件绑定检查。它不是 `fingerprint`，也不是完整机器指纹。
它只覆盖窄组件集：必需 `system`、`motherboard`、`memory`、`storage`、`network`，
可选 `gpu`。CPU 和显示器/显示设备不参与；网络只使用 MAC，
不使用网络类型、接口、IP、速率或链路状态。缺少必需组件时 `status` 为 `failed`、
`value` 为 `null`，仍会输出 JSON 并返回退出码 `2`。权限失败会在探测前返回
退出码 `4`，stdout 为空。

### `list-kinds` — 支持的设备类别

```bash
qurbrix-hw list-kinds                # 文本，每行一个
qurbrix-hw list-kinds --format json  # JSON 数组
```

```text
system
motherboard
bios
cpu
memory
storage
gpu
monitor
network
audio
bluetooth
input
camera
battery
printer
cdrom
usb
pci
other-pci
other-device
```

### `schema` / `sources`

```bash
qurbrix-hw schema             # -> qurbrix.hw.scan.v2
qurbrix-hw sources            # -> {"sources":[]}
```

## 硬件快照

快照是按需观测，不是实时监控。两次调用之间不会监听 udev、netlink、USB、PCI
或网络事件；上层需要新清单时再次调用 `snapshot ensure`。本项目不依赖
`qurbrix-monitor`，也不保存指标或事件时间序列。

默认状态目录是 `/var/lib/qurbrix-hwinfo`：

```text
/var/lib/qurbrix-hwinfo/qurbrix_hwinfo.db
/var/lib/qurbrix-hwinfo/reports/<snapshot_id>.json
```

目录权限为 `0700`，数据库和报告 artifact 权限为 `0600`。测试、容器或由调用方
管理的目录可使用 `--state-dir /controlled/path` 覆盖。完整 `ScanReport` 以不可变、
SHA-256 校验的 JSON artifact 保存；SQLite 只保存关系化查询投影，不保存完整
report/device JSON blob。

```bash
sudo qurbrix-hw snapshot ensure
sudo qurbrix-hw snapshot ensure --force --max-age 0s
qurbrix-hw snapshot show 01900000-0000-7000-8000-000000000000 --pretty
qurbrix-hw snapshot list --limit 30 --offset 0
qurbrix-hw snapshot diff <旧-snapshot-id> <新-snapshot-id>
qurbrix-hw snapshot export <snapshot-id> --output ./scan-report.json
qurbrix-hw snapshot health --pretty
qurbrix-hw snapshot prune --dry-run
qurbrix-hw snapshot pin <snapshot-id>
qurbrix-hw snapshot mark-uploaded <snapshot-id>
```

`ensure` 默认 TTL 为 24 小时；partial 只有在核心身份完整时才发布。严格调用方可加
`--reject-partial`。`export` 默认拒绝覆盖已有文件，显式使用 `--overwrite` 才覆盖。
所有 snapshot stdout JSON 使用 `qurbrix.hw.snapshot.cli.v1` schema，诊断只写
stderr。退出码：`0` 成功、`1` CLI/序列化、`2` 扫描/策略失败、`4` 权限、
`5` 未找到、`6` 存储/完整性、`124` 租约超时；旧命令退出码保持不变。

Rust 调用方使用同一 store 和 artifact：

```rust,no_run
use qurbrix_hw::{ensure_snapshot, EnsureSnapshotOptions, InventoryStore};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let store = InventoryStore::open("/var/lib/qurbrix-hwinfo").await?;
let id = ensure_snapshot(&store, EnsureSnapshotOptions::default()).await?;
let snapshot = store.load_snapshot(id).await?.expect("已发布快照");
let report = store.load_scan_report(id).await?.expect("已校验 artifact");
assert_eq!(snapshot.snapshot_id, id);
assert_eq!(snapshot.device_count as usize, report.devices.len());
# Ok(()) }
```

自动保留规则保护当前快照、所有 pinned 或未上传快照，以及每个 machine bind ID
最近 30 个快照。`snapshot prune` 只能清理超过 90 天且不在最近窗口内的已上传快照；
先用 `--dry-run` 预览，需要其他边界时使用 `--keep-recent` 和 `--max-age`。artifact
删除失败会进入可重试队列。`snapshot health` 会执行 SQLite 完整性/外键检查、逐个
校验 artifact、报告孤儿与指标，并执行被动 WAL checkpoint。

如需删除全部本地历史，先停止调用方，再以目录所有者身份删除所选 state directory
（或默认 `/var/lib/qurbrix-hwinfo`）。不要只删除数据库或只删除 `reports/`，完整性
检查会有意拒绝这种分裂状态。

## 集成合约

Rust 调用方直接依赖顶层 `qurbrix-hw` 库 facade。其他语言的上层程序调用 CLI，
解析 stdout JSON；这是当前稳定的跨语言边界。

面向机器调用时：

- 优先使用 `qurbrix-hw scan --format json`，消费 flat 外部 schema。
- 需要常规读取或低频硬件绑定检查时，使用 `qurbrix-hw bindid`，消费
  `qurbrix.hw.bindid.v1` 输出。
- 只有明确需要 Rust 模型形状时，才使用 `qurbrix-hw scan --format typed-json`。
- 不要解析 `summary` 或 `table` 这类面向人的输出。
- 不要依赖 JSON 字段顺序或空白格式。
- 以 `schema_version` 作为兼容性标记；破坏性输出变更必须升级 schema version，
  兼容性变更可以追加字段。

## 库用法

```rust
use hw_model::ScanConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let report = qurbrix_hw::collect_scan_report(ScanConfig::default()).await?;
    let flat = hw_output::to_flat_report(&report);
    println!("{}", serde_json::to_string_pretty(&flat)?);
    Ok(())
}
```

## 主要数据流

1. `hw-source` 执行命令或读取系统文件。
2. `hw-parser` 将原始文本解析为紧凑 source records。
3. `hw-probe` 将 source records 转换为 typed `Device`。
4. `hw-collect` 负责编排 probe 并生成 `ScanReport`。
5. `hw-output` 将报告转换为 flat JSON、JSONL、summary 和 table。
6. `bindid` 从采集结果中选择窄组件集，归一化字段、排序 component key，
   用 SHA1 生成轻量业务绑定 ID。

## 注意事项

- `dmidecode`、部分 `/sys` 路径和设备信息可能需要更高权限。
- `bindid` 与硬件采集命令一样需要 root；元数据命令不需要 root。
- 显示器采集依赖 EDID 和可选的 `xrandr` 输出；无图形会话时仍会尝试读取 sysfs。
- `partial` 报告仍然应当可以被机器消费。
- 日志和诊断信息写入 stderr；结构化命令输出写入 stdout。

## 贡献

欢迎贡献代码。本地开发环境、测试命令与提交约定见
[`CONTRIBUTING.md`](CONTRIBUTING.md)（英文）。缺陷和需求走
[GitHub Issues](https://github.com/BaekElk19/qurbrix-hwinfo/issues)，
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
