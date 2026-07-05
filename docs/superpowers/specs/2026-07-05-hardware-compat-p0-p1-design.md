# qurbrix-hwinfo 硬件兼容性提升计划（P0 + P1）— 设计文档

生成日期：2026-07-05
上游依据：`docs/hardware-compatibility-gap-report.md`

## 0. 概览

本 spec 覆盖 gap report 建议实施计划中的 **P0（CPU 三源合并）** 与 **P1（归一化 + Monitor EDID）**。P2/P3 明确暂缓，见 §7 决策记录。

实施采用 **方案 C** —— 三阶段串行，每阶段单独 PR、单独验收：

- **P0** — CPU 三源合并（`lscpu` + `lshw -class processor` + `dmidecode -t 4`）
- **P1a** — 归一化模块（arch alias + CPU vendor alias + GPU vendor alias）
- **P1b** — Monitor EDID（`xrandr --verbose` + `/sys/class/drm/*/edid` + qurbrix 自写 EDID parser）

保持 qurbrix 现有分层不变：`hw-source` → `hw-parser` → `hw-probe` → `hw-model` → `hw-collect`。新增/修改集中在 parser 和 probe，`hw-collect` 的合并/去重契约不动。

## 1. 模块架构与边界

### 1.1 crate 拓扑

沿用现有分层：

- `hw-source`：命令/文件执行，`Missing`/`PermissionDenied`/`Timeout`/`Failed` 分类不变。
- `hw-parser`：纯字符串/字节解析，无 IO。
- `hw-probe`：编排数据源，装配 `Device`。
- `hw-model`：数据模型。
- `hw-collect`：编排 probe、按 `Device.id` 合并、后处理去重。

### 1.2 新增模块

- `hw-parser/src/normalize/mod.rs`
  - `hw-parser/src/normalize/arch.rs`
  - `hw-parser/src/normalize/cpu_vendor.rs`
  - `hw-parser/src/normalize/gpu_vendor.rs`
  - `hw-parser/src/normalize/pnp.rs`（P1b 时新增，Monitor 用）

  三个 vendor / arch 归一化函数 + 一份 PNP ID 表，都是 `&str -> Option<&'static str>`。放在 `hw-parser` 而非 `hw-probe`，因归一化本质是"字符串规范化"，与 parser 定位一致；probe 层保持"编排 + 装配"单一职责。

- `hw-parser/src/edid.rs`（P1b 时新增）
  - `EdidRecord` 结构和 `parse_edid(&[u8]) -> Result<EdidRecord, EdidError>`
  - qurbrix 自写 EDID 128B 定长解析。不依赖 `edid-decode` 外部命令，不引入第三方 crate。

### 1.3 新增数据源

全部通过现有 `hw-source::runner`：

- CPU：
  - `lshw -class processor`（command）
  - `dmidecode -t 4`（command）—— 与已在用的 `dmidecode -t 0,1,2,3`/`-t memory` 是同构调用
- Monitor：
  - `xrandr --verbose`（command）
  - `/sys/class/drm/*/edid`（file glob + read_bytes）

`hw-source::runner` 目前是否已具备"glob 一批文件并按二进制读取"的能力，实现阶段确认；如不足，添加一个 `read_glob_bytes` helper。

### 1.4 改造的 probe

- `hw-probe/src/existing.rs::CpuProbe`：从"单命令 lscpu"改为"三源合并"，产出一个 `cpu:0` device（详见 §2）。
- `hw-probe/src/existing.rs::GpuProbe`（或 GPU 装配所在处）：装配 `GpuInfo` 时调用 `normalize_gpu_vendor`。
- `hw-probe/src/existing.rs::MonitorProbe`：改为 `xrandr --query` + `xrandr --verbose` + `/sys/class/drm/*/edid` 三源，填充 EDID 字段（详见 §4）。

### 1.5 不变的部分

- `hw-source::runner` 错误分类。gap report 明确"优于多数脚本式参考实现，建议保留"。
- `hw-collect::merge` 的 `Device.id` 合并策略。
- `Device` / `DeviceProperties` 顶层结构。
- `CpuInfo` 现有字段 —— `max_freq_mhz`/`min_freq_mhz`/`flags` 已预留（`hw-model/src/properties.rs:54-65`），此次仅填充；`current_freq_mhz` 是 P0 唯一新增字段。
- `MonitorInfo` 现有字段全部保留；P1b 加的字段全部 `Option`，向后兼容。

### 1.6 边界原则

- **Parser 无 IO，Probe 无解析**。
- 三源合并逻辑抽为纯函数 `merge_cpu_records(lscpu, lshw, dmi) -> MergedCpu`，`CpuProbe` 只负责"跑命令 → 调 parser → 调 merge → 调 normalize → 装 Device"。合并规则 100% 通过 fixture 单元测试。
- **数据源部分失败不影响其他源**：任一源 `Missing`/`PermissionDenied`/`Failed` 时，其他源继续参与合并；只有全部源失败才走 `ProbeResult::source_failure`。

## 2. CPU 三源合并规则

### 2.1 Parser 结构

`CpuRecord`（已存在，扩字段）：

```
architecture, threads, model_name, vendor,
cores_per_socket, sockets,
// 新增
cpu_mhz, cpu_max_mhz, cpu_min_mhz,
cpu_family, cpu_model, stepping, bogomips,
flags: Vec<String>, virtualization,
```

`LshwCpuRecord`（新）：

```
product, vendor, version,
```

`DmidecodeCpuRecord`（新，每 socket 一条）：

```
socket_designation, manufacturer, version,
family, max_speed_mhz, current_speed_mhz,
core_count, thread_count,
```

`parse_lscpu` 扩展支持新增键；`parse_lshw_processor` 与 `parse_dmidecode_processor` 是原创实现。

### 2.2 合并函数

签名：

```rust
pub fn merge_cpu_records(
    lscpu: Option<CpuRecord>,
    lshw:  Option<LshwCpuRecord>,
    dmi:   &[DmidecodeCpuRecord],
) -> MergedCpu;
```

字段级规则分两类：

- **字符串字段（name / vendor）走 Deepin 的 override 语义**：按 lscpu → lshw → dmi 顺序依次应用，后源在满足条件时覆盖前源当前值。Loongson 保护、ARMv/null 回退作用于覆盖步骤。
- **数值字段（sockets / freq / family）走 first-non-None 优先级链**：数值不适合"最后写入胜出"，取第一个可用值。

**Override 字段（`name` / `vendor`）**

`name` 的合成序列：

1. `name = lscpu.model_name`（若存在）
2. 若 `lshw` 可用且 `name` 不含 `Loongson`（大小写无关）：
   - 令 `candidate = lshw.product`
   - 若 `candidate` 含 `null` 或 `ARMv`（大小写无关）或为空 → `candidate = lshw.version`
   - 若 `candidate` 非空 → `name = candidate`
3. 若 `dmidecode[0].version` 可用且 `name` 不含 `Loongson` → `name = dmidecode[0].version`
4. 若 `name` 仍为空 → `name = "CPU"`（保底）

此序列自然覆盖两种 fallback 场景：
- **特殊机型**：lscpu 缺 model_name → 步骤 2/3 依次填入 lshw/dmi 值。
- **Loongson 保护**：任一步骤中一旦 `name` 含 `Loongson`，后续覆盖跳过。

`vendor` 的合成序列（无 Loongson 保护，普通 override）：

1. `vendor = lscpu.vendor`
2. 若 `lshw.vendor` 非空 → `vendor = lshw.vendor`
3. 若 `dmidecode[0].manufacturer` 非空 → `vendor = dmidecode[0].manufacturer`

归一化在 §3 应用于合成结果。

**数值/单源字段**

| 字段 | 优先级链 | 备注 |
|---|---|---|
| `architecture` | `lscpu.architecture` | 单源。归一化在 §3。 |
| `sockets` | `count_unique(dmidecode[*].socket_designation)` → `lscpu.sockets` → `dmidecode.len()` | Deepin 用 `QSet<Socket Designation>` 去重。 |
| `cores` | 见 §2.3 计数修正 | |
| `threads` | 见 §2.3 计数修正 | |
| `max_freq_mhz` | `lscpu.cpu_max_mhz` → `dmidecode[0].max_speed_mhz` | u32 MHz。 |
| `min_freq_mhz` | `lscpu.cpu_min_mhz` | 单源。 |
| `current_freq_mhz` | `dmidecode[0].current_speed_mhz` | **飞腾/ARM 补丁**：lscpu 拿不到当前频率。qurbrix `CpuInfo` 新增此字段。 |
| `family` | `lscpu.cpu_family` → `dmidecode[0].family` | |
| `model` | `lscpu.cpu_model` | 单源。 |
| `stepping` | `lscpu.stepping` | 单源。 |
| `flags` | `lscpu.flags` | 单源。 |
| `virtualization` | `lscpu.virtualization` | 单源。 |
| `bogomips` | `lscpu.bogomips` | 单源。 |

### 2.3 计数修正

对应 Deepin `generatorCpuDevice:230-236`：

```
cores   = lscpu.cores_per_socket * lscpu.sockets
threads = lscpu.threads

let dmi_cores   = sum(dmi[*].core_count)
let dmi_threads = sum(dmi[*].thread_count)

if dmi_cores > cores && dmi_cores <= 512 && threads != dmi_threads {
    cores = dmi_cores;
}
if dmi_threads > threads && dmi_threads < 1024 {
    threads = dmi_threads;
}
```

`≤512` / `<1024` 上界沿用 Deepin 的经验值，防止畸形 DMI 数据污染结果。

### 2.4 probe 层错误处理

- `lscpu`：optional，`Missing`/`Failed` → 该源为 `None`，`warning` 记录。
- `lshw`：optional，`Missing` → `None` + warning（很多桌面默认没装 lshw）。
- `dmidecode -t 4`：optional，`PermissionDenied` → `None` + warning "dmidecode requires root"。

仅当三源全部为 `None` 或全部产出空 record 时，返回 `ProbeResult::source_failure`。这是**行为变化**：现有 `CpuProbe` 在 `lscpu` 单一失败时整体失败，改造后可容忍单源失败。

### 2.5 Device 装配

**全机聚合为单个 `cpu:0` device**：`sockets`/`cores`/`threads` 是全机总量；`name`/`vendor` 用第一个 socket 的值。qurbrix 当前 `Device.id` 契约就是 `cpu:0` 单实例，多物理 CPU 场景通过 `sockets` 字段表达，不改输出契约。

### 2.6 Loongson 保护的作用范围

**只保护 `name`**，`vendor` 走正常优先级链。这与 Deepin `setInfoFromLshw:311-323` 语义一致（Longson 分支只 skip product）。

## 3. 归一化模块

### 3.1 通用匹配策略

- 输入先 `trim`，ASCII lowercase 比对。
- Vendor 类：**substring contains**。
- Arch 类：**exact match**。
- 输出：`&'static str` 常量字面量。
- CPU vendor 分两个入口：先精确 vendor_id 匹配，未命中再走 model_name substring 推断。

### 3.2 签名

```rust
pub fn normalize_arch(uname_m: &str) -> Option<&'static str>;

pub fn normalize_cpu_vendor_id(vendor_id: &str) -> Option<&'static str>;

pub fn infer_cpu_vendor_from_name(model_name: &str) -> Option<&'static str>;

pub fn normalize_gpu_vendor(vendor: &str) -> Option<&'static str>;
```

CPU probe 的组装：

```rust
let vendor = merged.vendor
    .as_deref()
    .and_then(normalize_cpu_vendor_id)
    .or_else(|| merged.name.as_deref().and_then(infer_cpu_vendor_from_name))
    .map(str::to_string)
    .or(merged.vendor);  // 都未命中 → 保留原始 vendor_id
```

**未命中归一化表的 vendor 保留原字符串**（不返回 `None`，不丢字段）。

### 3.3 arch alias（exact match）

```
x86_64      -> x86_64
amd64       -> x86_64
i386, i686  -> i386
aarch64     -> aarch64
arm64       -> aarch64
loongarch64 -> loongarch64
loongarch   -> loongarch64
sw_64       -> sw_64
mips64      -> mips64
mips64el    -> mips64
riscv64     -> riscv64
```

canonical 侧为 kernel 名（`x86_64` / `aarch64`），与 lscpu 输出一致。

### 3.4 CPU vendor_id alias（精确匹配 lscpu Vendor ID）

```
GenuineIntel   -> Intel
AuthenticAMD   -> AMD
HygonGenuine   -> Hygon
CentaurHauls   -> Zhaoxin    (早期 Zhaoxin)
Shanghai       -> Zhaoxin    (新版 Zhaoxin)
```

### 3.5 CPU 名称推断（substring, case-insensitive）

命中顺序（特殊/国产在前）：

```
loongson                    -> Loongson
phytium                     -> Phytium
kunpeng, hisilicon, kirin   -> HiSilicon
huawei                      -> HiSilicon
zhaoxin                     -> Zhaoxin
hygon                       -> Hygon
sunway                      -> Sunway
intel                       -> Intel
amd                         -> AMD
arm                         -> ARM      (最后 fallback)
```

**Kunpeng/Huawei/HiSilicon 统一映射到 `HiSilicon`**：业界共识，鲲鹏（Kunpeng）是华为设计、海思（HiSilicon）出品的 ARM 服务器 CPU。若后续要求区分品牌与出品方，改一行即可。

### 3.6 GPU vendor alias（substring, case-insensitive）

```
nvidia                      -> NVIDIA
advanced micro devices, amd -> AMD
intel                       -> Intel
matrox                      -> Matrox
aspeed                      -> ASPEED
vmware                      -> VMware
red hat, virtio             -> VirtIO
loongson                    -> Loongson
jingjia, jjm                -> Jingjia Micro
zhaoxin                     -> Zhaoxin
moore threads, mthreads     -> Moore Threads
innosilicon                 -> Innosilicon
wuhan digital engineering   -> WDE
```

不含虚拟机厂商（如 `INNOTEK`/`VBOX`）—— 虚拟机识别归 gap report P2，不在本 spec 范围。

### 3.7 证据保留

归一化只影响展示字段（`CpuInfo.vendor` / `GpuInfo.vendor`）。原始 vendor_id / model_name 通过 `Device.with_source(SourceEvidence)` 现有链路保留在 device 的 source 列表里，不需要额外字段。归一化前后不一致时**不发 warning**。

## 4. Monitor EDID

### 4.1 数据源架构

```
xrandr --query    -> 已有，保留   -> connector / current mode / connected 状态
xrandr --verbose  -> 新增          -> 每个 connector 内嵌 EDID hex block
/sys/class/drm/*/edid -> 新增文件源 -> headless / Wayland-only 场景的 EDID 二进制
```

### 4.2 合并优先级

按 connector 名（如 `HDMI-1`、`eDP-1`）为 key 逐 monitor 合并：

```
resolution / connector / connected 状态 : xrandr --query 单源

edid raw bytes : xrandr --verbose  →  /sys/class/drm/<name>/edid
                 （xrandr verbose 命中就用它；未命中或失败才读 sysfs）
```

sysfs connector 名归一化：kernel drm 目录名形如 `card0-HDMI-A-1`，需归一化到 xrandr 侧的 `HDMI-1`（丢 `card0-` 前缀、把 `HDMI-A-1` 归一化到 `HDMI-1`）。此映射是纯字符串处理，写在 monitor probe 里。

sysfs edid 文件的两种状态：
- 0 字节：monitor 未接，跳过。
- 128B 或 256B（带扩展块）：走 EDID parser。

### 4.3 EDID parser（`hw-parser/src/edid.rs`）

输入 `&[u8]`，长度必须 ≥ 128 且以 EDID 魔数 `00 FF FF FF FF FF FF 00` 开头。校验和不对时返回 `Err(EdidError::BadChecksum)`。

输出：

```rust
pub struct EdidRecord {
    pub manufacturer: Option<String>,       // 3 字母 PNP ID
    pub product_code: Option<u16>,
    pub serial:       Option<String>,       // Descriptor FF 段 ASCII serial
    pub name:         Option<String>,       // Descriptor FC 段 monitor name
    pub week:         Option<u8>,
    pub year:         Option<u16>,          // byte + 1990
    pub size_cm:      Option<(u8, u8)>,
    pub preferred_mode: Option<PreferredMode>,
}

pub struct PreferredMode {
    pub width: u16,
    pub height: u16,
    pub refresh_hz: u16,
}
```

字段来源（VESA EDID 1.4）：
- Manufacturer PNP ID：byte 8-9 的 16-bit big-endian，5-bit-per-letter，`A=0b00001`。
- Product code：byte 10-11 little-endian。
- Week/Year：byte 16-17。
- Size：byte 21-22（cm）。
- Preferred mode：byte 54 起第一个 Detailed Timing Descriptor。
- Descriptor 类型：byte offset 3 的 tag 字节，`0xFC = name`、`0xFF = serial number`。

### 4.4 xrandr --verbose 里 EDID 的抽取

parser 逐行扫描：`^\s+EDID:` 触发抓取模式，后续以 `^\s{8}` 开头的十六进制行拼成 hex string，遇到不匹配的行结束。hex→bytes 用一个小 helper。

实现放在 `hw-parser/src/monitor.rs`（现有 `parse_xrandr_query` 的姊妹函数 `parse_xrandr_verbose`）。

### 4.5 `MonitorInfo` 扩字段

现有字段保留。新增（全 `Option`，向后兼容）：

```rust
pub manufacturer: Option<String>,           // "AOC" 三字母
pub manufacturer_name: Option<String>,       // "AOC International" 若命中 PNP 表
pub product: Option<String>,                 // EDID name descriptor
pub product_code: Option<u16>,
pub serial: Option<String>,
pub manufactured_year: Option<u16>,
pub manufactured_week: Option<u8>,
pub size_cm: Option<(u8, u8)>,
pub preferred_width: Option<u16>,
pub preferred_height: Option<u16>,
pub preferred_refresh_hz: Option<u16>,
```

### 4.6 PNP ID 表

`hw-parser/src/normalize/pnp.rs` 静态表，约 30 条常见桌面 monitor 厂商。命中 → 填 `manufacturer_name`；未命中 → `manufacturer_name = None`，`manufacturer` 保留 3 字母。

最终清单（3 字母 PNP ID，全部合法）：

```
AOC AUS ACR ACI BNQ CMN CMO DEL EIZ FUS
GSM HPN HWP LEN LGD LPL MSI NEC PHL SAM SEC
SHP SNY VSC HSD BOE CPT LGP AMR CHI
```

未列出的品牌保留 3 字母原样，不阻塞。P1b 完成后如实际扫描发现遗漏，可追加。

### 4.7 权限与 headless 场景

`xrandr --verbose` 在无 DISPLAY 的 root/headless 会失败。此时依赖 `/sys/class/drm/*/edid` —— 它对普通用户可读，无需 root，也无需 X 会话。只要显示器接着且开机时枚举了 drm，就能拿到 EDID。这是 sysfs fallback 存在的核心理由。

### 4.8 Bad EDID 处理

`parse_edid` 遇到坏校验和/长度不够时返回 `Err`。probe 层：

```rust
match parse_edid(&edid_bytes) {
    Ok(edid) => monitor.merge_edid(edid),
    Err(e)   => probe_result.push_warning(format!(
        "EDID parse failed for {}: {}", connector, e)),
}
```

Monitor 记录仍生成（有 connector / resolution），只是没有 vendor/product/year 等 EDID 字段。

## 5. Fixtures 与测试策略

### 5.1 目录结构

```
crates/hw-testdata/fixtures/
├── cpu/
│   ├── lscpu-intel-x86_64.txt
│   ├── lscpu-amd-x86_64.txt
│   ├── lscpu-loongson-loongarch64.txt
│   ├── lscpu-phytium-arm64.txt
│   ├── lscpu-kunpeng-arm64.txt
│   ├── lscpu-hisilicon-kirin.txt
│   ├── lscpu-hygon.txt
│   ├── lscpu-zhaoxin.txt
│   ├── lscpu-model-name-missing.txt
│   ├── lscpu-no-vendor-id.txt
│   ├── lshw-intel-singlesocket.txt
│   ├── lshw-loongson.txt              # 触发 Loongson 保护
│   ├── lshw-product-null.txt          # 触发 "null → version" 回退
│   ├── lshw-product-armv.txt          # 触发 "ARMv → version" 回退
│   ├── lshw-missing.txt               # 空输出 / 命令缺失
│   ├── dmidecode-4-single-socket.txt
│   ├── dmidecode-4-dual-socket.txt
│   ├── dmidecode-4-phytium.txt        # 仅 Current Speed 可信
│   ├── dmidecode-4-permission-denied.txt
│   └── merge/
│       ├── intel-x86_64.expected.json
│       ├── amd-x86_64.expected.json
│       ├── loongson-loongarch64.expected.json
│       ├── phytium-arm64.expected.json
│       ├── kunpeng-arm64.expected.json
│       ├── hisilicon-kirin.expected.json
│       ├── hygon.expected.json
│       ├── zhaoxin.expected.json
│       ├── lscpu-only.expected.json
│       ├── lshw-only.expected.json
│       ├── dmi-only.expected.json
│       └── all-empty.expected.json
├── monitor/
│   ├── xrandr-verbose-single-display.txt
│   ├── xrandr-verbose-dual-display.txt
│   ├── xrandr-verbose-headless.txt
│   ├── xrandr-verbose-no-edid.txt
│   ├── sysfs-edid-hdmi-1.bin
│   ├── sysfs-edid-edp-1.bin
│   ├── sysfs-edid-empty.bin
│   ├── sysfs-edid-bad-checksum.bin
│   └── merge/
│       ├── xrandr-and-sysfs.expected.json
│       ├── sysfs-only.expected.json
│       └── xrandr-verbose-only.expected.json
├── gpu/
│   ├── lspci-nvidia.txt
│   ├── lspci-amd.txt
│   ├── lspci-intel.txt
│   ├── lspci-loongson.txt
│   ├── lspci-jingjia.txt
│   └── normalize/
│       └── vendor-alias.expected.json
└── normalize/
    ├── arch.cases.txt
    ├── cpu-vendor-id.cases.txt
    ├── cpu-name-inference.cases.txt
    └── gpu-vendor.cases.txt
```

### 5.2 测试层次

**1. Parser 单元测试**（`hw-parser/src/**/tests` 或 `#[cfg(test)]` 内联）

- `parse_lscpu` 扩展字段：flags/family/model/stepping/mhz/virtualization
- `parse_lshw_processor` 各变体（Loongson name、null/ARMv product、缺失字段）
- `parse_dmidecode_processor` 单/多 socket、permission denied 输出
- `parse_xrandr_verbose` EDID hex block 抽取
- `parse_edid` 每个字段独立测试

**2. 合并/归一化单元测试**（`hw-parser/src/cpu.rs`、`hw-parser/src/normalize/`）

- `merge_cpu_records` 每一条 §2.2 字段规则一个 case
- Loongson 保护：lshw.product=`Loongson 3A5000`、lscpu.model_name=`Loongson-3A5000` → 合并后 name 是 lscpu 值，dmi 不覆盖
- 计数修正：dmi_cores=64/dmi_threads=128 vs lscpu cores_per_socket=32 sockets=1 threads=32 → cores 修正为 64、threads 修正为 128
- 计数修正上界：dmi_cores=1000（超 512） → 不修正
- 特殊机型：lscpu 无 model_name/vendor_id + dmi.version=`Kunpeng 920` + dmi.manufacturer=`HiSilicon` → name/vendor 从 dmi 补齐
- 归一化：`normalize/*.cases.txt` 按 `输入\t期望` 逐行断言

**3. Probe 集成测试**（`hw-probe/tests/`）

- 用 `hw-source` 的 mock/stub 机制注入 fixture 内容
- CPU：三源全部成功、lshw 缺失、dmi permission denied、三源全部失败
- Monitor：xrandr verbose + sysfs 都存在、只有 sysfs、xrandr 报 headless、bad checksum EDID
- 断言 `Device` 结构完整（含 `SourceEvidence` 每条源的 status）

**4. 端到端快照测试**（`crates/hw-collect/tests` 或 CLI 快照）

- `merge/*.expected.json` 用 `insta` 或 `serde_json::to_string_pretty` 比较
- 每个关键平台一个快照
- 快照与 fixture 同目录，便于并排审查

### 5.3 版权与来源合规

所有 fixture **手工构造或从公开 samples 改编**，不复制 Deepin/Kylin 的 fixture。可能来源：

- qurbrix 维护者在真机上跑 `lscpu`/`lshw`/`dmidecode` 采集
- 公开 issue tracker 里贴出的样本
- 按输出格式手写的合成样本

每个 fixture 顶部加一行注释：

```
# source: <来源方式>, redacted: <是否清理过 serial/mac 等 PII>
```

### 5.4 测试运行成本

- Parser 和 merge 是纯函数，`cargo test` 秒回。
- Probe 集成测试用 runner mock，不 spawn 实际命令 —— CI 无 lshw/dmidecode/xrandr 也能跑通。
- 快照 `expected.json` 提交 git，回归时 diff 一目了然。

## 6. 分阶段验收标准

三阶段各自独立 PR、独立评审、独立可回退。

### 6.1 阶段 P0 — CPU 三源合并

**涉及模块**

- 新增：`hw-parser/src/cpu.rs` 中的 `LshwCpuRecord` / `DmidecodeCpuRecord` / `parse_lshw_processor` / `parse_dmidecode_processor` / `merge_cpu_records`
- 修改：`hw-parser/src/cpu.rs::parse_lscpu`（扩字段）、`hw-model/src/properties.rs::CpuInfo`（新增 `current_freq_mhz`）、`hw-probe/src/existing.rs::CpuProbe`（三源编排）
- 新增 fixtures：`crates/hw-testdata/fixtures/cpu/` 下 §5.1 列出的全部条目

**验收条件**

- 全部 §5.2 定义的 CPU parser/merge unit test 通过
- 端到端快照 `merge/*.expected.json` 提交且稳定
- lscpu 一切正常但 lshw/dmidecode 都缺失时，`Device.name`/`vendor`/`architecture`/`cores`/`threads` 不为 `None`（等价于"P0 至少不比现在差"）
- lscpu 缺失但 dmidecode 存在时，`name` 和 `vendor` 能从 dmidecode.version/manufacturer 拿到（override 链自然覆盖此场景：§2.2 步骤 3）；此时 `architecture` 为 `None`，warning 说明
- 三源全部失败时，`ProbeResult` 是 `source_failure`，warnings 列出三条失败原因
- CPU 类别之外的 device 在 CPU probe 变更前后**逐字节相同**（用现有 fixture 端到端 CLI 输出 diff 为空）
- `cargo build --release` 通过；`qurbrix-hw` 单 binary 可正常执行
- 手动验证：至少一台 x86_64（Intel 或 AMD）机器上真跑 `qurbrix-hw`，CPU 字段与 `lscpu` 显示一致

**非验收范围**

- 归一化 —— `GenuineIntel` 原样输出，P1a 才改
- GPU vendor 归一化 —— P1a
- Monitor EDID —— P1b

### 6.2 阶段 P1a — 归一化

**涉及模块**

- 新增：`hw-parser/src/normalize/{mod.rs, arch.rs, cpu_vendor.rs, gpu_vendor.rs}`
- 修改：
  - `hw-probe/src/existing.rs::CpuProbe`：合并后调用 `normalize_arch` / `normalize_cpu_vendor_id` / `infer_cpu_vendor_from_name`
  - `hw-probe/src/existing.rs::GpuProbe` / `hw-probe/src/pci.rs`（视 GPU 装配位置）：装配 `GpuInfo` 时调用 `normalize_gpu_vendor`
- 新增 fixtures：`crates/hw-testdata/fixtures/normalize/*.cases.txt`、`crates/hw-testdata/fixtures/gpu/lspci-*.txt` + `gpu/normalize/vendor-alias.expected.json`

**验收条件**

- `normalize/*.cases.txt` 里每一行输入都通过
- P0 阶段的所有 `merge/*.expected.json` 快照更新一遍：`GenuineIntel` → `Intel`、`AuthenticAMD` → `AMD`、arch 保留 kernel 名、Loongson/Phytium/Kunpeng 等国产 vendor 稳定规范化
- 未命中归一化表的 vendor 保留原字符串（不是 `None`、不是空串）；专项 case：手写 `"UnknownVendor Corp"` 的 lscpu fixture，断言输出仍是 `"UnknownVendor Corp"`
- GPU 归一化后 `GpuInfo.vendor` 变化，但 `Device` 的 `SourceEvidence` 里原始 lspci 行仍保留
- 手动验证：真机跑一遍 x86_64 上 `Intel` 展示；若有条件在国产平台跑一遍 `Loongson` / `Phytium` 展示

**非验收范围**

- PNP ID 表 —— P1b（只服务 monitor）

### 6.3 阶段 P1b — Monitor EDID

**涉及模块**

- 新增：`hw-parser/src/edid.rs`、`hw-parser/src/normalize/pnp.rs`、`hw-parser/src/monitor.rs::parse_xrandr_verbose`
- 修改：
  - `hw-model/src/properties.rs::MonitorInfo`：新增 §4.5 列出的 EDID 字段
  - `hw-probe/src/existing.rs::MonitorProbe`：改为 `xrandr --query` + `xrandr --verbose` + sysfs edid 三源
  - `hw-source::runner`：如现有能力不足以 glob `/sys/class/drm/*/edid`，扩展一个 `read_glob_bytes` helper（待实现阶段确认）
- 新增 fixtures：`crates/hw-testdata/fixtures/monitor/` 全套

**验收条件**

- EDID parser 单元测试全通（manufacturer/product_code/week/year/size/preferred_mode/name descriptor/serial descriptor/坏校验和 ≥ 8 个 case）
- 有 EDID 的 fixture：`MonitorInfo.manufacturer`/`product`/`manufactured_year`/`preferred_width`/`preferred_height` 全部填充
- 无 EDID 的 fixture（headless xrandr、空 sysfs edid）：`MonitorInfo.connector`/`resolution` 仍正确，EDID 字段全 `None`，`Device` 仍生成
- 坏校验和 EDID：warning 记录一条，monitor device 仍生成
- xrandr 命令缺失时，单靠 sysfs 也能生成 monitor device
- sysfs 权限不足时走 warning，不 panic
- PNP 表命中：`AOC` → `manufacturer_name = "AOC International"`；未命中：`manufacturer_name = None`、`manufacturer` 保留 3 字母
- 手动验证：接一台外接显示器跑一遍，看 EDID 字段展示；断电或拔线状态下再跑，确认无 EDID 分支不 panic

### 6.4 跨阶段守则

- `cargo test --workspace` 全绿
- `cargo clippy --workspace -- -D warnings` 通过（如仓库有此约定）
- 二进制体积增长每阶段不超过 +200KB
- 单次 `qurbrix-hw` 执行时长不劣化超过 15%（`lshw` 无 root 下可能慢一些，probe 层加 5-8 秒超时；`hw-source::runner` 已支持 timeout）
- 所有新增 fixture 顶部 `# source:` 注释合规
- 每阶段的 commit message 标注 `P0:` / `P1a:` / `P1b:` 前缀

## 7. 决策记录

| # | 决策点 | 采纳的方案 | 参考项目的做法 | 为什么偏离 |
|---|---|---|---|---|
| D1 | CPU 数据源组合 | `lscpu` + `lshw -class processor` + `dmidecode -t 4` | Deepin 三源；Kylin 走 `/proc/cpuinfo` | 严格按 Deepin |
| D2 | Device 装配粒度 | 全机聚合为单个 `cpu:0` device | Deepin 每个 lscpu 逻辑核一个 `DeviceCpu` | qurbrix 当前 `Device.id` 契约是单实例；多 socket 通过 `sockets` 字段表达 |
| D3 | Loongson 保护范围 | 只保护 `name`，`vendor` 走正常优先级链 | Deepin `setInfoFromLshw:311-323` 只 skip product | 与 Deepin 原意一致 |
| D4 | 计数修正上界 | `dmi_cores ≤ 512`、`dmi_threads < 1024` | Deepin `generatorCpuDevice:230-236` 同值 | 沿用经验值，防畸形 DMI 数据 |
| D5 | Kunpeng / Huawei / HiSilicon 归一化 | 三者全部映射到 `HiSilicon` | Kylin `huawei→Huawei`，`hisilicon` 另立 | 业界共识：鲲鹏属海思出品；vendor 语义统一优于品牌区分 |
| D6 | 归一化未命中语义 | 保留原始字符串 | Kylin 表外通常保留原值 | 未知 vendor 不应丢失字段 |
| D7 | 归一化 canonical 侧 | kernel 名（`x86_64` / `aarch64`） | Deepin 用 Debian 包名（`amd64` / `arm64`） | 与 lscpu 输出一致，减少二次转换 |
| D8 | `loongarch` vs `loongarch64` | 统一归一化为 `loongarch64` | Deepin 两者分立保留 | 32 位龙芯几乎不在桌面/服务器 |
| D9 | ARM Phytium 当前频率 | dmidecode `Current Speed` 补 `current_freq_mhz`（`CpuInfo` 新增字段） | Deepin `setInfoFromDmidecode:389` 同源 | qurbrix 侧新增字段 |
| D10 | 三源全空时的行为 | `ProbeResult::source_failure` + 三条 warnings | Deepin `lsCpu_num.size() <= 0` 直接 return | qurbrix 有结构化的失败通道，全记录利于诊断 |
| D11 | Monitor EDID 数据源优先级 | `xrandr --verbose` 优先，sysfs fallback | Kylin xrandr verbose + `edid-decode`；Deepin 走 hwinfo/xrandr | 保证 headless / Wayland 场景也能拿到 EDID |
| D12 | EDID 解析实现 | qurbrix 自写 128B parser | Kylin 调 `edid-decode` 外部命令 + 写 `/tmp/edid.dat` | 避免 `/tmp` 副作用、外部依赖、并发冲突 |
| D13 | Bad EDID 处理 | warning + monitor device 仍生成 | 参考项目一般丢字段 | 数据源问题不该让设备消失 |
| D14 | PNP 表规模 | ~30 条常见桌面 monitor 厂商 | Kylin 依赖 edid-decode 完整表 | 严格 P1 范围；未命中保留 3 字母 |
| D15 | GPU 归一化范围 | NVIDIA/AMD/Intel/Matrox/ASPEED/VMware/VirtIO/Loongson/Jingjia/Zhaoxin/Moore Threads/Innosilicon/WDE | Kylin 表更大，含虚拟机 | 虚拟机识别归 gap report P2 |
| D16 | Fixture 来源 | 手工构造或公开来源改编 | 不复制参考项目 fixture | 许可证 + gap report §11 要求 |
| D17 | fixture 顶部注释 | 每个 fixture 加 `# source: … redacted: …` | 无对应实践 | 便于审查来源合规和 PII 状态 |
| D18 | 三阶段独立 PR | P0 / P1a / P1b 分开合并 | 一次性大 patch | 缩小评审面 + 每阶段独立验收 |
| D19 | 输出契约变化 | `CpuInfo.current_freq_mhz` 新增；`MonitorInfo` 加 EDID 字段（全 `Option`）；顶层结构不变 | — | 加字段保持向后兼容 |
| D20 | 快照测试目录位置 | `fixtures/**/merge/*.expected.json` 与 fixture 同目录 | 常见做法是 `snapshots/` | 输入 + 期望输出并排，评审更直观 |

## 8. 明确暂缓项

不进本 spec，但记录于此，便于后续再开 spec：

- 存储 SMART / temperature / controller、`lsblk` 之外的 sysfs fallback（gap report P2）
- USB `lsusb -v` interface / hub 过滤（P2）
- 网卡 sysfs driver / wireless / virtual 过滤（P2）
- DMI `/sys/class/dmi/id` fallback（P2）
- `/proc/cpuinfo` fallback —— D1 选了 Deepin 三源，`/proc/cpuinfo` 暂不进；若实际运行发现 `lshw` 太慢或缺失率过高，这将是下一个 spec 的起点
- HiSilicon Kirin `/proc/hardware` 检测（Kylin `cpuinfo.py:62-71`）—— 桌面/服务器场景意义有限
- 虚拟机设备识别（QEMU/VMware/VirtualBox alias，P2）
- 输入设备 / 摄像头 / 电池的 vendor alias（P2）
- `glxinfo` / `hwinfo` 作为可选源（P3）

## 9. 附录：证据索引

- `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:22-70` — 当前 CpuProbe 单 lscpu 实现
- `qurbrix-hwinfo/crates/hw-parser/src/cpu.rs:13-30` — 当前 lscpu parser
- `qurbrix-hwinfo/crates/hw-model/src/properties.rs:54-65` — CpuInfo 现有字段
- `qurbrix-hwinfo/crates/hw-source/src/runner.rs:22-59` — 命令/文件错误分类
- `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceGenerator.cpp:173-261` — Deepin CPU 三源合并 + 计数修正
- `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:224-395` — Deepin 逐源字段合并规则、Loongson 保护、Phytium 频率
- `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceFactory.cpp:26-73` — Deepin 架构分流
- `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/commonfunction.cpp:25-33` — Deepin 架构 alias 表
- `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:704-745` — Kylin CPU vendor 归一化
- `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:1339-1411` — Kylin EDID 提取流程
- VESA EDID 1.4 规范 — EDID 128B 结构、Descriptor tag 定义
