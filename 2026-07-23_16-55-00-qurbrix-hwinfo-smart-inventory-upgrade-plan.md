# qurbrix-hwinfo 智能硬件库存架构改造计划

> 适用仓库：`BaekElk19/qurbrix-hwinfo`
> 执行方式：适合 GPT 在无人值守时按版本连续实施
> 核心目标：把项目收敛为**一套采集实现、一个智能库存入口、一份连续硬件事实历史**

---

## 1. 最终目标

将 qurbrix-hwinfo 从“多个命令分别触发硬件扫描的工具集合”，改造成统一的本地 Hardware Inventory Engine。

默认调用流程：

```text
qurbrix-hw scan
        │
        ▼
创建本次 scan_run
        │
        ▼
执行唯一的 quick probe
        │
        ├── 硬件未变化
        │       │
        │       ├── 关联 current_snapshot
        │       └── 从数据库读取并返回完整 ScanReport
        │
        └── 硬件发生变化 / --force
                │
                ▼
        执行唯一的 full scan
                │
                ▼
        保存不可变 snapshot
                │
                ▼
        更新 current_snapshot
                │
                ▼
        返回新的完整 ScanReport
```

### 核心原则

1. **全仓库只能有一套轻量采集逻辑。**
2. **全仓库只能有一套全量采集逻辑。**
3. CLI、库存服务和未来的 qurbrix-core 只能调用统一应用服务，不得各自拼接采集流程。
4. 每次 CLI 调用都必须留下 `scan_run`。
5. 每次真实全量扫描都必须保存结果。
6. 轻量检测确认无变化时，复用数据库中的完整快照。
7. 只要产生了新的可用全量结果，就保存并更新 `current_snapshot`。
8. 不存在 `--no-store`、`--no-promote` 或“诊断结果不入库”模式。
9. 失败、部分成功、警告和耗时也属于历史证据，不能静默丢弃。
10. 任何阶段不得为了兼容而长期保留两套采集代码。

---

# 2. 全局硬约束

以下约束适用于全部版本阶段。

## 2.1 单一采集源

推荐唯一职责：

```text
hw-source      唯一负责调用命令、读取 /proc、/sys 等原始来源
hw-parser      唯一负责解析原始数据
hw-probe       唯一负责将解析结果转换为 Device
hw-collect     唯一负责组织完整 ScanReport
hw-inventory   唯一负责 quick probe、快照判断、持久化和历史
hw-cli         只负责参数解析、调用应用服务和格式化输出
```

禁止：

- CLI 中直接执行 `lscpu`、`dmidecode`、`lsblk`、`lspci`、`lsusb` 等命令。
- `hw-inventory` 自己实现第二套完整设备枚举。
- `bindid` 再维护一套独立的主板、内存、磁盘、网卡采集器。
- `summary`、`table`、`scan` 各自调用不同的采集入口。
- 为兼容旧接口复制一份旧采集代码。
- 同一个硬件字段在两个 crate 中通过不同算法生成。
- 保留“暂时未使用但以后也许有用”的旧 collector、旧 parser 或旧 probe。

## 2.2 允许存在的两种采集粒度

项目可以存在：

```text
quick probe
full scan
```

但它们必须职责明确：

### quick probe

只负责低成本判断：

- 当前机器身份
- 当前硬件配置指纹
- 指纹算法版本
- 是否需要执行 full scan

它不是第二套全量采集器，不得输出完整设备清单。

### full scan

唯一负责产生完整 `ScanReport`。

所有完整硬件结果都必须来自同一个 `full_scan()` 调用链。

---

# 3. 每阶段统一执行规则

每个版本阶段必须独立完成、独立验证、独立提交。

## 3.1 每阶段开始前

执行：

```bash
git status
git log --oneline -10
cargo test --workspace
```

确认工作区干净，现有测试基线可用。

## 3.2 每阶段完成后

必须执行：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
```

如仓库已有脚本或 CI 命令，额外执行对应命令。

## 3.3 每阶段版本动作

每个阶段必须：

1. 更新 workspace/package 版本号。
2. 更新 `CHANGELOG.md`。
3. 更新 README 中与行为相关的命令说明。
4. 创建单一、清晰的 commit。
5. 创建对应 git tag。
6. 确认没有未提交文件。

推荐顺序：

```bash
git add -A
git commit -m "<commit message>"
git tag vX.Y.Z
```

未经验证不得提交，不得提前打 tag。

---

# 4. V0.2.1：全仓库采集逻辑审计与单一化

## 4.1 阶段目标

在改造智能入口之前，先确认仓库中是否存在历史残留的两套或多套采集逻辑。

本阶段必须做到：

> 从原始硬件来源到完整 `ScanReport`，全仓库只剩一条正式调用链。

## 4.2 必须完成的审计

搜索所有可能触发硬件采集的代码：

```bash
rg "collect_scan_report|full_scan|quick_probe|Command::new|std::process::Command|tokio::process::Command" .
rg "lscpu|dmidecode|lsblk|lspci|lsusb|xrandr|/proc|/sys" crates src
rg "ScanReport|DeviceKind|BindId|configuration_fingerprint|machine_bind_id" crates src
```

建立调用链清单，至少覆盖：

- `scan`
- `summary`
- `table`
- `bindid`
- `snapshot ensure`
- `hw_collect::collect_scan_report`
- `hw_inventory::full_scan`
- `quick_probe`
- 所有 source runner
- 所有 parser
- 所有 probe
- 测试 fake runner
- 顶层 facade

新增文档：

```text
docs/architecture/hardware-collection-call-graph.md
```

文档必须列出：

1. 当前存在的采集入口。
2. 每个入口最终调用的函数。
3. 是否执行原始系统命令。
4. 是否产生完整 `ScanReport`。
5. 是否写数据库。
6. 是否与其他路径重复。
7. 最终保留或删除决定。

## 4.3 唯一保留的正式调用链

建议收敛为：

```text
RealSnapshotScanner::full_scan
        │
        ▼
hw_inventory::full_scan
        │
        ▼
hw_collect::collect_scan_report
        │
        ▼
hw_probe
        │
        ▼
hw_parser
        │
        ▼
hw_source
```

允许为了测试使用 trait 和 fake scanner，但 fake 实现不能进入生产构建路径。

## 4.4 必须删除的历史残留

发现以下情况时必须删除，不得只标记 deprecated：

- CLI 内直接采集硬件。
- `bindid` 内独立执行完整采集。
- inventory 中复制的完整设备采集流程。
- 旧版 collector。
- 未使用 parser。
- 未使用 probe。
- 同名但不同实现的 source runner。
- 仅用于旧入口的转换层。
- 已被统一接口替代的 helper。
- 被注释掉的旧实现。
- 只为旧测试服务的生产代码。

如旧 API 需要短期兼容，只允许保留薄包装：

```rust
pub async fn old_api(...) -> Result<...> {
    new_single_source_api(...).await
}
```

薄包装不得包含任何采集、判断或持久化逻辑，并应在后续阶段删除。

## 4.5 `bindid` 特别检查

必须检查 `hw-bindid` 是否通过独立采集器获取：

- system
- motherboard
- memory
- storage
- network
- gpu

目标：

- `bindid` 不得拥有第二套硬件事实采集逻辑。
- 可从统一 quick probe 或统一完整 ScanReport 派生。
- 绑定算法可以独立，但输入数据来源必须统一。
- CLI 的短 bind ID 与库存的 SHA-256 bind ID 如继续并存，必须明确说明用途，不能分别再采一遍硬件。

## 4.6 测试要求

新增架构级测试或可执行检查，确保：

1. `scan`、`summary`、`table` 不直接调用原始 source。
2. 完整 `ScanReport` 只有一个生产构造入口。
3. quick probe 不返回完整 Device 清单。
4. bindid 不拥有独立 full collector。
5. 删除旧路径后所有测试仍通过。
6. `cargo tree` 中不存在旧采集 crate 的重复依赖路径。

可新增脚本：

```text
scripts/check-single-collection-path.sh
```

用于 CI 检查禁止模式，例如：

- 在 `hw-cli` 中出现 `collect_scan_report`
- 在 `hw-cli` 中出现系统命令执行
- 在 `hw-bindid` 中出现完整 source runner
- 新增第二个生产 `full_scan` 实现

## 4.7 验收标准

- 有完整调用链审计文档。
- 已明确唯一 quick probe。
- 已明确唯一 full scan。
- 旧采集代码已物理删除。
- 没有“暂时保留”的第二套路径。
- 所有生产入口最终汇聚到同一实现。
- 全部测试通过。

## Commit

```text
refactor: enforce single hardware collection pipeline
```

## Version

```text
v0.2.1
```

---

# 5. V0.2.2：建立统一 Inventory Observe 应用服务

## 5.1 阶段目标

消除 `scan` 与 `snapshot ensure` 两套业务流程。

采集实现已经在 V0.2.1 唯一化，本阶段进一步统一：

- 是否复用快照
- 是否触发全量扫描
- 如何保存结果
- 如何返回完整结果

## 5.2 新增统一接口

建议新增：

```rust
pub async fn observe_inventory(
    store: &InventoryStore,
    options: ObserveInventoryOptions,
) -> Result<InventoryObservation>;
```

建议模型：

```rust
pub struct ObserveInventoryOptions {
    pub force_full_scan: bool,
    pub scan_config: ScanConfig,
}

pub struct InventoryObservation {
    pub report: ScanReport,
    pub snapshot_id: SnapshotId,
    pub result_source: ObservationSource,
    pub hardware_changed: bool,
}

pub enum ObservationSource {
    ReusedSnapshot,
    NewFullScan,
}
```

## 5.3 统一流程

```text
observe_inventory()
        │
        ├── 创建 scan_run
        ├── 执行 quick probe
        ├── 对比 current snapshot
        │
        ├── 未变化
        │      └── 加载并返回已有完整 ScanReport
        │
        └── 变化 / force
               ├── 调用唯一 full scan
               ├── 保存 snapshot
               ├── 更新 current_snapshot
               └── 返回新 ScanReport
```

## 5.4 CLI 改造

以下命令必须全部调用 `observe_inventory()`：

```text
qurbrix-hw scan
qurbrix-hw summary
qurbrix-hw table
```

禁止它们直接调用：

```rust
hw_collect::collect_scan_report(...)
```

`snapshot ensure` 处理方式二选一：

### 推荐

删除 `snapshot ensure`，只保留历史管理命令。

### 兼容过渡

保留一版薄包装，内部直接调用 `observe_inventory()`，不得保留原有业务逻辑。

## 5.5 验收标准

- `scan`、`summary`、`table` 和 `bindid` 都通过 `observe_inventory()`。
- 每次硬件观测都会记录 quick probe 历史。
- quick fingerprint 未变化时返回经过完整性验证的当前快照。
- 变化、强制、过期或 quick probe 失败时执行唯一 full scan。
- 每个可用且核心身份完整的 full scan 都保存为不可变快照并更新 current snapshot。
- 并发观测者共享已发布快照，不重复发布同一次硬件事实。
- 所有失败、partial、warning、耗时和租约超时都有历史记录。
- 全部质量门通过。

## Commit

```text
refactor: unify hardware inventory observation
```

## Version

```text
v0.2.2
```

---

# 6. V0.2.3：完整库存与并发边界硬化

## 6.1 阶段目标

关闭统一 observe 链路上线审查发现的边界问题，确保 CLI 视图参数不会污染库存事实，
并确保长时间扫描、并发打开 store 和直接发布都遵守同一租约协议。

## 6.2 硬约束

1. `--kind`、`--exclude-kind`、`--no-optional-sources`、`--no-sources` 和
   `--no-warnings` 只能过滤 stdout，不得缩小采集或持久化结果。
2. 等待者的超时按“租约无进展时间”计算；持有者续租必须重置等待预算。
3. 有效扫描租约存在时，任何进程打开 store 都不得把其运行中 probe 标记为失败。
4. 所有发布入口都必须持有有效租约，并在发布事务中校验 owner token。
5. CLI `--timeout` 同时约束 quick probe、每个 source 和 full scan 全局 deadline。
6. stdout 过滤不得篡改底层硬件观测的 `status` 和退出语义。

## 6.3 验收标准

- 完整库存不变量有 CLI view 单元测试。
- 多次续租超过单个等待周期仍可让等待者成功复用或继续扫描。
- store 恢复同时覆盖“有效租约保护”和“无租约恢复”两条路径。
- 直接发布无法越过已有扫描租约。
- workspace 版本、锁文件、README、changelog 和发布脚本统一为 `0.2.3`。
- `cargo fmt --all -- --check`、Clippy、workspace tests、workspace check、架构检查和
  release 检查全部通过。

## Commit

```text
fix(inventory): harden complete observation concurrency
```

## Version

```text
v0.2.3
```
