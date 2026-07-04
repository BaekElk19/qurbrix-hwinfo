# Qurbrix HW Info

Qurbrix HW Info 是一组用于 Linux 硬件信息采集、解析、归一化和入库的 Rust crate。项目把命令输出、`/proc`、`/sys`、EDID、PCI 与网络信息整理为统一的 `Inventory`，再生成稳定的机器 `bindid`、组件表行和面向打印/上报的 JSON 结构。

## 能力范围

- 采集 CPU、内存、BIOS/主板、显示器、存储、GPU 和网络信息。
- 保留原始命令输出，便于排错、回放和对比采集结果。
- 根据硬件组合键生成稳定的机器 `bindid`。
- 将硬件信息映射为 `component_records` 表结构兼容的 `ComponentRow`。
- 可生成包含主板、CPU、内存、存储、网络与组件行镜像的格式化 JSON。
- 提供 SQLite upsert 存储能力。

## 目录结构

```text
.
├── src/                    # 顶层 qurbrix-hw 库，对外聚合采集、bindid、行转换和 JSON 归一化
├── crates/
│   ├── hw-model/           # 硬件数据模型、Inventory、ComponentInfo trait
│   ├── hw-source/          # 命令与文件采集源，带超时控制
│   ├── hw-parser/          # lscpu、dmidecode、lsblk、xrandr、EDID、ip、ethtool 等解析逻辑
│   ├── hw-collect/         # 采集编排，将 source/parser 输出合并成 Inventory
│   ├── hw-store/           # bindid、ComponentRow 映射和 SQLite 仓储
│   ├── hw-merge/           # 解析结果合并入口
│   └── hw-api/             # API/DBus 草稿模块，当前需要与新模型继续对齐
└── Cargo.toml              # 顶层库 manifest
```

## 运行环境

目标环境是 Linux。采集质量取决于系统中可用的命令和权限：

- 基础信息：`lscpu`、`cat /proc/cpuinfo`、`/proc/meminfo`
- BIOS、主板、内存插槽：`dmidecode`，通常需要 root 权限
- 存储：`lsblk`、`udevadm`
- 显示器/GPU：`xrandr`、`glxinfo`、`/sys/class/drm`
- 网络：`ip`、`lspci`、`ethtool`

缺少部分命令时，采集器会尽量回退到可用的数据源，返回的字段可能减少。

## 当前构建前提

当前目录中的 `Cargo.toml` 和多个子 crate 使用了 `*.workspace = true` 继承 workspace 级配置，但此目录没有包含 `[workspace]` 根配置。因此把本目录作为独立 checkout 直接执行 `cargo check` 会失败。

要构建本项目，需要满足其中一种条件：

- 将本目录放回原始 Cargo workspace 中，由上层 workspace 提供 `workspace.package` 和 `workspace.dependencies`。
- 或者补齐本目录的 `[workspace]`、`[workspace.package]`、`[workspace.dependencies]` 配置。

在 workspace 恢复后，可使用：

```bash
cargo check
cargo test
```

## 基本用法

```rust
use qurbrix_hw::{
    build_formatprint_payload, collect, compute_bind_id, to_component_rows, CollectConfig,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CollectConfig::default();
    let inventory = collect(&config).await?;

    let bind_id = compute_bind_id(&inventory);
    let machine_sn = inventory.machine_serial();
    let rows = to_component_rows(&inventory, machine_sn, &bind_id);
    let payload = build_formatprint_payload(&inventory, &bind_id, machine_sn, &rows);

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}
```

## 存储示例

```rust
use hw_store::repo::ComponentRepo;

async fn save(rows: &[qurbrix_hw::ComponentRow]) -> anyhow::Result<()> {
    let repo = ComponentRepo::open_or_create("hardware.db").await?;
    repo.upsert_batch(rows).await?;
    Ok(())
}
```

`ComponentRepo` 会创建 `component_records` 表，并以 `(fd_CODE, fd_NAME, fd_SN, fd_INFO_EX10)` 作为冲突键执行 upsert。

## 主要数据流

1. `hw-source` 执行命令或读取系统文件。
2. `hw-parser` 将原始文本解析为 `CpuInfo`、`MemoryInfo`、`BiosInfo` 等结构。
3. `hw-collect` 负责编排采集流程并生成 `Inventory`。
4. 顶层 `qurbrix-hw` 计算 `bindid`，生成 `ComponentRow` 和格式化 JSON。
5. `hw-store` 可将组件行写入 SQLite。

## 注意事项

- `dmidecode`、部分 `/sys` 路径和设备信息可能需要更高权限。
- 显示器采集依赖 EDID 和可选的 `xrandr` 输出；无图形会话时仍会尝试读取 sysfs。
- `bindid` 使用组件组合键排序后计算 SHA1，并取前 16 位 hex 字符。
- `hw-api` 目前保留了旧接口草稿，里面的类型和采集函数需要与当前 `Inventory` 模型继续对齐后再纳入主构建。
