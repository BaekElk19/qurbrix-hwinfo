# Qurbrix HW Info

Qurbrix HW Info 是一组用于 Linux 硬件信息采集、解析、归一化和输出的 Rust crate。项目把命令输出、`/proc`、`/sys`、PCI、USB、DMI、显示、电源和外设信息整理为 typed `ScanReport`，并提供 flat JSON、JSONL、summary 和 table 输出。

## 能力范围

- 采集 CPU、内存、BIOS/主板、显示器、存储、GPU、网络、PCI、USB、音频、蓝牙、输入设备、摄像头、电池、打印机和 CD-ROM 信息。
- 保留 source evidence，便于排错、回放和对比采集结果。
- 为 Rust 调用方提供 typed `ScanReport` 模型。
- 为脚本和 agent 提供 flat JSON、JSONL、summary 和 table 输出。
- 提供 fake source runner 与 fixture 驱动的 parser/probe 测试。

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

## 构建

```bash
cargo check --workspace
cargo test --workspace
```

## 命令行用法

默认输出面向脚本和 agent，结构化结果写入 stdout，日志写入 stderr：

```bash
qurbrix-hw scan --format json --pretty
qurbrix-hw scan --format jsonl
qurbrix-hw summary
qurbrix-hw table --kind storage
qurbrix-hw list-kinds
```

扫描状态：

- `complete`：扫描完成且没有重要 warning。
- `partial`：生成了可用报告，但部分数据源缺失、失败、超时或权限不足。
- `failed`：无法生成有效报告。

`partial` 仍返回退出码 `0`，方便脚本继续消费已有结果。

## 集成合约

Rust 调用方直接依赖顶层 `qurbrix-hw` 库 facade。其他语言的上层程序调用 CLI，
解析 stdout JSON；这是当前稳定的跨语言边界。

面向机器调用时：

- 优先使用 `qurbrix-hw scan --format json`，消费 flat 外部 schema。
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

## 注意事项

- `dmidecode`、部分 `/sys` 路径和设备信息可能需要更高权限。
- 显示器采集依赖 EDID 和可选的 `xrandr` 输出；无图形会话时仍会尝试读取 sysfs。
- `partial` 报告仍然应当可以被机器消费。
- 日志和诊断信息写入 stderr；结构化命令输出写入 stdout。
