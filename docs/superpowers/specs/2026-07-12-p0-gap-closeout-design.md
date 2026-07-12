# P0 差距收敛设计（Q1/Q3/Q4/Q6/Q7）

- **日期**：2026-07-12
- **状态**：Draft — 待用户 review
- **背景文档**：`docs/component-gap-report-2026-07-11.md` §14.1
- **目标分支**：`fix/p0-gap-closeout`

---

## 1. 目标与非目标

### 1.1 目标

关闭 `component-gap-report-2026-07-11.md` §14.1 中定义为 P0（"快速修复：低成本高回报，1–3 天"）的差距。经代码勘察后，实际需动的差距为 **5 项**：

| 编号 | 差距 | 涉及模块 |
|---|---|---|
| Q1 | 蓝牙 parser 恢复 HCI/LMP Version、Manufacturer、Class、Features 字段 | `hw-parser/src/bluetooth.rs`、`hw-model` |
| Q3 | 显示器 `edid_hex: Option<String>` 输出 | `hw-parser/src/monitor.rs`、`hw-model` |
| Q4 | GPU `DriverInfo.version` 从 `/sys/module/<driver>/version` 或 `modinfo` 填充 | `hw-probe/src/existing.rs` |
| Q6 | `dmidecode -t 11` OEM Strings 采集 | `hw-probe`、`hw-parser/src/dmi.rs`、`hw-model` |
| Q7 | 存储分区 & 挂载：`lsblk -o` 加 `MOUNTPOINT/FSTYPE/PARTUUID/LABEL`（本 PR 仅采集与解析，不落 `StorageInfo`） | `hw-probe`、`hw-parser/src/storage.rs` |

### 1.2 报告漂移（不做）

差距报告写就于 2026-07-11。截至 2026-07-12 代码勘察，以下两项已由后续提交修复，从本 PR 剔除：

- **Q2 网络 MTU 落库**：`crates/hw-probe/src/existing.rs:1367` 已写 `mtu: net.mtu`。
- **Q5 `ufs_spec_version` 填值**：`crates/hw-probe/src/existing.rs:2674`、`:2887` 两处已填。

差距报告本身**不在**本 PR 内刷新完成度百分比，仅在每个 commit 内追加对应条目的 "fixed in <sha>" 标注。差距报告全量刷新单独安排。

### 1.3 非目标

- **不引入** `PartitionInfo` / `DeviceKind::Partition` 结构（Q7 分区数据的落库形态留给后续扩展）。
- **不重构** `network_apply_modinfo` 为跨类型通用 helper（YAGNI，等第二个复用点出现再抽）。
- **不实施** 报告 §14.2 / §14.3 的 M/L 项。
- **不改动** Q1/Q3 已有的老字段与调用约定（新字段以加法方式并存）。

---

## 2. 架构与做法

### 2.1 做法基调：纯加法

所有 model 层字段新增均为 `Option<T>` 或 `Vec<T>`，配合 `#[serde(default)]`。这保证：

- `hw-output` / `hw-bindid` 等下游 crate 反序列化历史 JSON 不破坏。
- 每个 commit 独立可回滚，不产生跨 commit 依赖。
- Q1 中新增字段与 `BluetoothControllerRecord.flags: Vec<String>` 共存，UP/RUNNING/PSCAN 类原始 flag 保留。

### 2.2 提交序列（5 commit，1 PR）

```
commit 1  fix(bluetooth): recover HCI/LMP version, manufacturer, class, features
commit 2  fix(monitor): expose edid_hex on MonitorInfo
commit 3  fix(gpu): populate DriverInfo.version from sysfs with modinfo fallback
commit 4  feat(dmi): collect OEM Strings via dmidecode -t 11
commit 5  fix(storage): extend lsblk columns with mountpoint/fstype/partuuid/label
```

排序原则："改动面从小到大 + 项间独立性"。前 4 项之间无耦合；Q7 放尾巴，因 lsblk fixture 的 JSON 变更会触发下游 storage golden 测试的重跑，独立在一个 commit 便于二分。

### 2.3 每 commit 自包含清单

1. Fixture 新增或更新（`crates/hw-testdata/fixtures/**`）
2. Parser 单测（`crates/hw-parser/tests/**`）— 先红
3. Model 字段（`crates/hw-model/src/properties.rs`，`#[serde(default)]`）
4. Parser 实现（`crates/hw-parser/src/**`）— 转绿
5. Probe 集成（`crates/hw-probe/src/**`）
6. Probe 层测试（`crates/hw-probe/tests/**`，仅在 fixture 能覆盖时）
7. `docs/component-gap-report-2026-07-11.md` 对应条目末尾追加 `(fixed in <short-sha>)`

---

## 3. 各项详细契约

### 3.1 Q1 — 蓝牙 parser 补齐

**Model** —— `crates/hw-model/src/properties.rs`，`BluetoothInfo` 新增：

```rust
pub hci_version: Option<String>,   // 例："12 (0xc)"
pub lmp_version: Option<String>,   // 例："12 (0xc)"
pub manufacturer: Option<String>,  // 例："Intel Corp. (2)"
pub device_class: Option<String>,  // 例："0x7c010c"
#[serde(default)]
pub features: Vec<String>,         // 例：["0xff", "0xfe", "0x8f", ...]
```

字段类型统一使用 `String` / `Vec<String>` 保留原文；后续如需 `u16` / `u32` 强类型化再单独处理，本 PR 不涉及。

**Record** —— `crates/hw-parser/src/bluetooth.rs`，`BluetoothControllerRecord` 同名 5 字段追加：

```rust
pub struct BluetoothControllerRecord {
    pub name: Option<String>,
    pub address: Option<String>,
    pub bus: Option<String>,
    pub flags: Vec<String>,           // 保留
    pub hci_version: Option<String>,  // 新
    pub lmp_version: Option<String>,  // 新
    pub manufacturer: Option<String>, // 新
    pub device_class: Option<String>, // 新
    pub features: Vec<String>,        // 新
}
```

**Parser 逻辑** —— `parse_hciconfig` 内既有 for-line 循环追加分支（在现有 flags 分支之前判定，以免把 `HCI Version:` 之类的行误吞成 flags）：

- `line.trim_start().starts_with("HCI Version:")` → `split_once(':').1.trim()` 存入 `hci_version`
- `line.trim_start().starts_with("LMP Version:")` → 同
- `line.trim_start().starts_with("Manufacturer:")` → 同
- `line.trim_start().starts_with("Class:")` → 同（保留原始 `0xNNNNNN` 格式）
- `line.trim_start().starts_with("Features:")` → 冒号之后的 hex tokens `split_whitespace()`，过滤空 token，写入 `features`

保持使用 `starts_with` + `split_once(':')`，避免正则膨胀。原有 `address_re` / `name_re` 不变。

**Probe** —— `crates/hw-probe/src/bluetooth.rs` 现有合入逻辑仅在 `BluetoothInfo` 上把 5 字段透传即可，无新数据源。

**Fixture** —— `crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt`：先核验现有内容是否覆盖 5 字段；若缺，补一份典型 Intel 或 Realtek 控制器的 `hciconfig -a` 完整输出。

**测试**：`crates/hw-parser/tests/`（新增文件 `bluetooth.rs` 或若已有则追加用例），断言：
- 5 个新字段解析出预期值
- `flags` 与新字段互不吞并
- `Features:` 行的十六进制 tokens 全部保留

`crates/hw-probe/tests/peripheral_probes.rs` 追加断言 `BluetoothInfo` 上 5 字段流通到探针输出。

---

### 3.2 Q3 — MonitorInfo.edid_hex 暴露

**Model** —— `MonitorInfo` 追加：

```rust
#[serde(default)]
pub edid_hex: Option<String>,   // 小写、无空格、无 "0x" 前缀
```

**Parser** —— `crates/hw-parser/src/monitor.rs`：

- 现有 `edid_hex: String` 局部变量已经在文件 line 83 拼装。
- 把它随 `MonitorRecord`（或 xrandr verbose record）返回；如现有 record 结构没有 hex 字段，追加 `pub raw_hex: String`。
- 规范化：全部转小写、去掉所有空白字符（`.chars().filter(|c| !c.is_whitespace()).collect()`）。

**Probe** —— `crates/hw-probe/src/existing.rs` MonitorProbe 合并处：把 parser 返回的 hex 写入 `MonitorInfo.edid_hex`，多来源合并时（`/sys/class/drm/*/edid` 二进制 vs xrandr `--verbose`）以第一个非空为准。

**Fixture** —— `crates/hw-testdata/fixtures/edid/aoc.hex` 已存在，直接复用。若 xrandr verbose 分支需要覆盖，可从 `fixtures/xrandr/` 挑一份含 EDID 段的输出。

**测试**：
- `crates/hw-parser/tests/edid.rs` 追加：`edid_hex` 存在且为 `[0-9a-f]+`，长度是 EDID 原始字节数的 2 倍。
- `crates/hw-parser/tests/monitor_verbose.rs` 追加：从 xrandr --verbose fixture 解析出 hex，规范化后与预期一致。

---

### 3.3 Q4 — GPU DriverInfo.version 填充

**新增函数** —— `crates/hw-probe/src/existing.rs`：

```rust
async fn gpu_driver_version(ctx: &ProbeContext<'_>, driver_name: &str) -> Option<String>
```

优先级：
1. `ctx.filesystem.read_to_string_lossy(format!("/sys/module/{driver_name}/version"))` → trim，非空即返回。
2. Fallback：`ctx.runner.run_command(&CommandSpec::new("modinfo", [driver_name]), ...)` → 找第一行 `^version:` 的值。

选择直接读 `sysfs` 而非命令：单次 read 开销约 μs 级，比新起 `modinfo` 子进程快数量级；`modinfo` 只在 sysfs 无值时兜底（模块内建至内核会没有 `/sys/module/<n>/version`）。

**集成点** —— `GpuProbe::probe`（`existing.rs:5281+`）中 11 处 `.with_driver(DriverInfo { name: ..., version: None, ... })`：

- 在 device 组装完成、其他 enrichment 已应用后，用 `device.driver.as_ref().and_then(|d| d.name.clone())` 拿驱动名。
- 若非空，`spawn` 一次 `gpu_driver_version`，把结果通过 `device.driver.as_mut().unwrap().version = ...` 回写。
- GPU 数量通常 ≤ 4，用 `futures::future::join_all` 或顺序 `await` 皆可，无并发压力。

**Fixture** —— 新增：
- `crates/hw-testdata/fixtures/gpu/sys_module_i915_version.txt` — 内容如 `6.6.30\n`
- `crates/hw-testdata/fixtures/gpu/modinfo-nvidia.txt` — 含 `version:        550.90.07` 行的典型输出

**测试**：
- `crates/hw-parser/tests/gpu.rs` 追加纯字符串解析用例（若 modinfo 解析走 parser 层）；或在 `hw-probe` 层用 mock filesystem/runner。
- 断言：sysfs 命中优先、fallback 到 modinfo、两者都无则 `version = None`。

**风险**：11 处 `with_driver` 集成点意味着改动分散。缓解：抽一个 `apply_gpu_driver_version(ctx, device).await -> Device` 小 helper，所有 11 处末尾统一 `.pipe(apply_gpu_driver_version)`（或用 for-loop 后置处理）。

---

### 3.4 Q6 — dmidecode -t 11 OEM Strings

**Model** —— `BiosInfo` 追加：

```rust
#[serde(default)]
pub oem_strings: Vec<String>,
```

**Parser** —— `crates/hw-parser/src/dmi.rs` 新增：

```rust
pub fn parse_dmi_oem_strings(input: &str) -> Vec<String>
```

识别 `Handle 0x..., DMI type 11` 段头，之后逐行匹配缩进的 `String N: <value>`，`value.trim()` 存入结果 `Vec`。段末以下一个 `Handle` 行或 EOF 结束。空 `String N:` 值（`Not Specified`）过滤。

**Probe** —— `crates/hw-probe/src/existing.rs` 主板 / BIOS 探针段：追加 `dmidecode -t 11` 命令（与现有 `-t 13`、`-t 16` 并列）。返回值走 `parse_dmi_oem_strings`，结果写入 `BiosInfo.oem_strings`。

**Fixture** —— 新增 `crates/hw-testdata/fixtures/dmi/dmidecode-t11.txt`：

```
# dmidecode 3.3
Getting SMBIOS data from sysfs.
SMBIOS 3.2.0 present.

Handle 0x000E, DMI type 11, 5 bytes
OEM Strings
        String 1: Default string
        String 2: LENOVO_MT_20UAS0LK00_BU_Think_FM_ThinkPad X1 Carbon Gen 9
        String 3: LENOVO_BIOS: N32ET75W (1.50 )
```

**测试**：
- `crates/hw-parser/tests/`（若已有 dmi 测试则追加，否则新增 `dmi.rs`）：断言 3 条 string 解析、空/`Not Specified` 值被过滤。
- `crates/hw-probe/tests/existing_category_probes.rs` 断言 `BiosInfo.oem_strings` 从 fixture 流通。

---

### 3.5 Q7 — lsblk 列扩展（采集 + 解析，不落库）

**Probe** —— `crates/hw-probe/src/existing.rs:3756`：

```rust
"lsblk",
["-J", "-b", "-o",
 "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV,MOUNTPOINT,FSTYPE,PARTUUID,LABEL"]
```

**Parser** —— `crates/hw-parser/src/storage.rs`，`LsblkDevice` record 追加：

```rust
pub mountpoint: Option<String>,
pub fstype: Option<String>,
pub partuuid: Option<String>,
pub label: Option<String>,
```

配合 `parse_lsblk_json_result` 的 serde 反序列化（4 字段均 `#[serde(default)]`，兼容缺列）。

**本 PR 范围到此为止**：`StorageInfo` 不加字段、下游 output 不变、`BindId` 不受影响。

**为什么本 PR 不落库到 `StorageInfo`**：
1. 落库需引入 `Vec<PartitionInfo>` 或类似结构，会牵动 output 序列化格式，是 spec 级变更。
2. 与差距报告 §14.1 "快速修复" 定位不符。
3. 落库要求本身在澄清中被用户选为"只加列，不新增分区设备"，进一步落库放到后续扩展。

**Fixture** —— 更新 `crates/hw-testdata/fixtures/storage/lsblk.json`：至少一个包含 `mountpoint`、`fstype`、`partuuid`、`label` 的分区节点。

**测试**：
- `crates/hw-parser/tests/storage.rs` 断言：`LsblkDevice` 4 字段解析出预期值。
- 现有 storage golden 测试若因 JSON 更新失败，需一并刷新 golden — 这是本 commit 独立在最后的原因。

---

## 4. 测试策略

**TDD 顺序**：每项都是 `fixture → 单测红 → 实现 → 单测绿`。

**工作区级校验**（PR 合入门槛）：

```
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo build --release
```

**不做实机验证**：`qurbrix-hwinfo` 是数据采集库，无 UI。output 变化通过下游 `hw-cli` / `hw-output` 的现有 fixture 测试覆盖。

**Fixture 位置汇总**：

| P0 | Fixture 路径 | 状态 |
|---|---|---|
| Q1 | `crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt` | 存在，需核验完整性 |
| Q3 | `crates/hw-testdata/fixtures/edid/aoc.hex` | 存在 |
| Q4 | `crates/hw-testdata/fixtures/gpu/sys_module_i915_version.txt`、`modinfo-nvidia.txt` | 新增 |
| Q6 | `crates/hw-testdata/fixtures/dmi/dmidecode-t11.txt` | 新增 |
| Q7 | `crates/hw-testdata/fixtures/storage/lsblk.json` | 存在，需扩列 |

---

## 5. 回滚策略

- **单项回滚**：`git revert <commit-sha>`。任一 commit 都可独立退出，其它 4 项与主干均不受影响 — 所有新字段皆 `Option<T>` / `Vec<T>` + `#[serde(default)]`，向后兼容。
- **全量回滚**：`git revert <merge-sha>` 或 `git revert <first>..<last>`。
- **禁止**：force push、`git reset --hard`、`--no-verify`。

---

## 6. 已知不做

- Q2 / Q5（已修）
- 差距报告完成度百分比刷新（仅追加 "fixed" 标注）
- `PartitionInfo` / `DeviceKind::Partition` 建模
- `network_apply_modinfo` 抽通用 helper
- 差距报告 §14.2 / §14.3 的中长期项
- 蓝牙字段的 `u16` / `u32` 强类型化（保留 `String` 原文）
- Wayland / DBus 兜底、Vulkan、GPU 温度功耗、ethtool、wireless 数据面
