# Qurbrix HWInfo 硬件快照能力提升计划

- 创建时间：2026-07-22 22:33:55 CST
- 最近更新：2026-07-23 00:45:36 CST
- 主题：基于 `quick_probe()`、`full_scan()`、`ensure_snapshot()` 和 SQLite 的硬件快照管理
- 当前状态：已收口为 GPT 可无人值守执行的阶段级 runbook，尚未开始实现

## 1. 背景

当前项目已经具备完整的 Linux 硬件采集能力：

- `hw-probe` 负责各类硬件探测；
- `hw-collect::collect_scan_report()` 编排完整扫描；
- `hw-model::ScanReport` 表达扫描结果；
- `hw-output` 提供稳定的外部输出格式。

目前每次调用都会重新采集并直接返回 `ScanReport`，项目尚未提供持久化缓存、历史快照和统一的快照 ID。目标是在现有采集层之上增加一层按需运行的硬件清单管理能力，使上层调用方只需确保存在有效快照，并通过 `snapshot_id` 读取一致的数据。

`qurbrix-hwinfo` 定位为硬件清单和历史快照工具，不是常驻监控服务。快照表示“最近一次调用时观察到的硬件状态”，不承诺在两次调用之间随系统变化实时更新。

## 2. 目标

新增以下能力：

1. `quick_probe()`：低成本获取能够反映硬件变化的规范化摘要。
2. `full_scan()`：复用现有完整采集器，生成可持久化的完整报告。
3. `ensure_snapshot()`：判断是否复用已有快照或创建新快照，最终返回 `snapshot_id`。
4. `qurbrix_hwinfo.db`：使用 SQLite 保存当前状态、不可变快照、设备明细和探测历史。
5. 提供按 `snapshot_id` 查询完整快照和设备列表的 API。
6. 保持当前 CLI 和 `collect_scan_report()` 的行为兼容。
7. 保持纯按需调用模式，不引入热插拔事件循环或常驻后台服务。
8. 使用 bindid 标识机器身份，主要物理部件变化后形成新的机器身份和测试结论域。
9. 将完整 `ScanReport` 保存为磁盘上的不可变 JSON artifact，SQLite 只保存结构化查询投影和 artifact 元数据。
10. CLI 与 Rust API 同时作为正式接口：Rust API 提供类型安全集成，CLI 提供稳定的跨语言 JSON 合约。

## 3. 非目标

本阶段不包含：

- 远程上传或云端同步；
- 多台机器共用一个 SQLite 数据库；
- 基于快照的授权或计费逻辑；
- 删除现有 JSON、JSONL、summary 和 table 输出；
- 大范围重写当前 probe、parser 或设备模型；
- 自动无限期保留所有历史快照；
- 监听 udev、netlink、USB、PCI、蓝牙或网络热插拔事件；
- 采集温度、利用率、实时频率、功耗、流量和风扇转速等热数据；
- 存储监控时间序列、告警状态或实时设备事件；
- 依赖、调用或通知 `qurbrix-monitor`；
- 接收来自 `qurbrix-monitor` 的快照失效通知。

### 3.1 与 `qurbrix-monitor` 的边界

两个项目保持零依赖，各自独立运行：

```text
qurbrix-hwinfo                         qurbrix-monitor
----------------                      ----------------
按需执行                               常驻或定时执行
硬件身份、型号、容量和拓扑             温度、负载、功耗和流量
完整设备清单                           USB/PCI/网络等设备事件
不可变历史快照                         指标时间序列和告警
qurbrix_hwinfo.db                      独立的 monitor 数据存储
```

边界规则：

- 温度变化、性能变化和热插拔事件统一归 `qurbrix-monitor`；
- `qurbrix-hwinfo` 不注册 udev listener，不维护事件循环，也不启动守护进程；
- `qurbrix-monitor` 不读写 `qurbrix_hwinfo.db`，不创建或更新硬件快照；
- `qurbrix-hwinfo` 不读取 monitor 的事件库或时间序列库；
- 两者即使只有一个运行，也必须能够完成各自职责；
- 上层业务如需同时展示清单和实时指标，分别调用两个项目并在业务层组合结果。

“USB 设备属于某次硬件清单”和“USB 发生了插入或拔出事件”是两个概念。`qurbrix-hwinfo` 可以在被调用时枚举当前 USB 设备并写入快照，但不负责监听 USB 何时发生变化；事件及发生时间由 `qurbrix-monitor` 负责。

### 3.2 Deepin Device Manager 的参考边界

`deepin-devicemanager` 首先作为架构研究样本；如果经过代码质量、行为契约、依赖和许可证评审，也可以作为候选实现来源。复用时必须把代码放入正确的项目边界：热插拔监听和热数据实现归 `qurbrix-monitor`，硬件快照实现归 `qurbrix-hwinfo`。

适合定向研究的内容：

- 首次完整加载与后续选择性刷新的冷热数据分层；
- 硬件命令的并行调度；
- 热插拔事件的延迟和去抖，但相关经验归 `qurbrix-monitor` 使用；
- 常驻服务、DBus 和 Polkit 架构，仅供未来其他系统服务参考。

不采用的内容：

- Deepin 的 `enable.db` 主要保存设备禁用、授权和唤醒配置，不是硬件快照数据库；
- 不照搬其运行时建表方式，`qurbrix_hwinfo.db` 必须使用 migration、事务、主外键和索引；
- 不采用缓存原始命令文本作为核心数据模型，继续以 typed `ScanReport` 和 `Device` 为准；
- 不将 Deepin 的 udev 热插拔监听代码引入 `qurbrix-hwinfo`。

候选代码复用前必须完成：

- 阅读并确认原实现的测试覆盖、错误处理、并发行为和安全边界；
- 确认实现符合本项目的 Rust API、数据模型、可测试性和模块边界；
- 保留原始版权声明、SPDX 标识、许可证文本和来源记录；
- 明确复用代码是直接复制、改编移植还是仅参考行为，分别记录审查结论；
- 为复用后的代码补充本项目的单元测试和集成测试，不以“上游能编译”作为质量证明。

许可证约束：`deepin-devicemanager` 使用 GPL-3.0-or-later，而本项目当前声明 MIT OR Apache-2.0。GPT 有权自主决定参考、改编或直接复用合适代码，不需要等待人工确认。如果直接复制或形成衍生作品，GPT 必须把整个仓库统一转换为 GPL-3.0-or-later，更新所有 Cargo manifest、许可证文件、README、SPDX 标识、版权和来源记录，并保证完整对应源代码可用；不得留下 MIT/Apache 与 GPL 派生代码混合但声明不一致的状态。仅研究行为或独立重写时继续保留 MIT OR Apache-2.0。许可证判断和采取的动作必须记录在阶段执行日志中，并由自动化文本检查确认仓库声明一致。

## 4. 建议架构

在现有 crate 之上新增独立的快照管理模块，避免把数据库逻辑放入 `hw-probe` 或 `hw-collect`。整个流程只由调用方主动触发，不存在后台刷新路径。

```text
业务调用方
    |
    v
ensure_snapshot()
    |
    +-- quick_probe() ----------------------+
    |                                      |
    |                        指纹未变化且缓存有效
    |                                      |
    |                                      v
    |                             返回当前 snapshot_id
    |
    +-- 首次运行、指纹变化、缓存过期或强制刷新
                                           |
                                           v
                                      full_scan()
                                           |
                                           v
                         事务写入 snapshot 和 devices
                                           |
                                           v
                            更新 inventory_state 当前指针
                                           |
                                           v
                                  返回新 snapshot_id
```

建议新增 crate：

```text
crates/hw-inventory/
├── src/lib.rs
├── src/error.rs
├── src/probe.rs
├── src/service.rs
├── src/store.rs
├── src/model.rs
└── migrations/
    └── 0001_initial.sql
```

职责划分：

- `probe.rs`：实现 quick probe、规范化和指纹计算；
- `service.rs`：实现 `ensure_snapshot()` 的业务编排；
- `store.rs`：封装 SQLite 事务和查询；
- `model.rs`：定义 `SnapshotId`、探测类型和持久化 DTO；
- `migrations`：维护显式、可测试的数据库 schema。

## 5. API 草案

以下签名仅用于确定职责，实施时根据异步 SQLite 驱动调整：

```rust
pub struct SnapshotId(pub uuid::Uuid);

pub enum PartialPolicy {
    PublishIfCoreComplete,
    Reject,
}

pub struct EnsureSnapshotOptions {
    pub force_full_scan: bool,
    pub max_snapshot_age: Option<Duration>,
    pub partial_policy: PartialPolicy,
}

pub async fn quick_probe(config: QuickProbeConfig) -> Result<QuickProbeReport>;

pub async fn full_scan(config: ScanConfig) -> Result<ScanReport>;

pub async fn ensure_snapshot(
    store: &InventoryStore,
    options: EnsureSnapshotOptions,
) -> Result<SnapshotId>;

pub async fn load_snapshot(
    store: &InventoryStore,
    snapshot_id: SnapshotId,
) -> Result<Option<StoredSnapshot>>;

pub async fn load_scan_report(
    store: &InventoryStore,
    snapshot_id: SnapshotId,
) -> Result<Option<ScanReport>>;
```

`full_scan()` 应当是对现有 `collect_scan_report()` 的薄封装，不建立第二套完整扫描逻辑。

### 5.1 数据合约的确定方法

数据合约不从表结构反推，而按以下顺序确定：

1. 列出调用方场景：首次建库、无变化复用、硬件变化、强制刷新、扫描失败、读取历史快照；
2. 写出不可变不变量：已发布快照不可修改、当前指针只能指向完整可读快照、设备只属于一个快照；
3. 明确每个失败场景的返回语义，尤其是“旧快照存在但新扫描失败”时不能只返回一个含义不明的 ID；
4. 为每个字段规定来源、空值含义、单位、稳定性和是否参与指纹；
5. 给外部 JSON 和 SQLite 记录增加显式 schema/fingerprint 版本；
6. 用固定 fixture 生成 golden JSON 和状态转移测试；
7. 阶段 A 的合约 fixture、状态转移测试和一致性检查全部通过后再写 migration，禁止先建表再用代码行为倒推合约；是否满足条件由 GPT 根据本 runbook 自主判定。

### 5.2 推荐的 V1 合约

V1 使用 UUIDv7 作为 `SnapshotId`，在应用层生成并以规范的小写连字符字符串对外序列化。SQLite 使用 `TEXT PRIMARY KEY` 保存该值。UUIDv7 能够跨机器唯一且大致按生成时间排序，适合后续上传服务器；数据库内部仍可使用 SQLite 隐式 `rowid` 优化本地访问，但它不进入外部合约。

`QuickProbeReport` 至少包含：

- `fingerprint_version`；
- `bindid_algorithm` 和 `machine_bind_id`；
- `configuration_fingerprint`；
- `canonical_payload` 或其可审计摘要；
- `observed_at`；
- 被纳入指纹的稳定设备身份集合；
- source warning 和权限/缺失信息。

`StoredSnapshot` 至少包含：

- `snapshot_id`；
- `machine_bind_id` 和 `bindid_algorithm`；
- `schema_version` 和 `scanner_version`；
- `created_at`；
- `scan_status`（只允许 `complete` 或经策略批准的 `partial` 发布）；
- `configuration_fingerprint`；
- 关系化的 snapshot/device/property/warning/source 查询投影；
- 完整 `ScanReport` artifact 的路径、SHA-256、大小和 schema 元数据；
- 设备数量、warning 数量和耗时。

`ensure_snapshot()` 的 V1 语义：

- 成功复用或发布时只返回一个有效 `snapshot_id`；
- quick probe 失败会尝试 full scan；
- full scan 失败返回错误，不返回旧 ID 冒充最新结果，但旧快照继续可读；
- 新快照发布前旧快照保持当前；
- `partial` 默认按 `PublishIfCoreComplete` 处理：核心硬件合约完整时允许发布，缺少核心身份时拒绝发布；调用方可显式使用 `Reject`；
- 指纹由同一套 canonicalizer 计算，quick 和 full 的结果使用同一版本字段。

这些规则应先写成 `EnsureSnapshotOutcome` 内部状态，再由公共 API 压缩为 `SnapshotId` 或明确错误，避免数据库状态、日志和 API 各自解释一套语义。

### 5.3 机器身份、配置身份和快照身份

三个 ID 分工如下：

```text
machine_bind_id            机器是谁
configuration_fingerprint  这台机器当前的软硬件配置是什么
snapshot_id                哪一次已发布快照
```

现有项目已经实现 `qurbrix.hw.bindid.v1` / `qurbrix-hw-bindid-sha1-hex16-v1`。它使用排序后的 component keys 计算 16 位 SHA-1 十六进制前缀，必需类别是 system、motherboard、memory、storage、network，GPU 为可选；CPU、BIOS/固件、驱动和内核不参与。v1 必须保留用于兼容，但不直接作为未来跨机器汇聚的最终算法。

新增版本化的 bindid v2：

- 算法名建议为 `qurbrix-hw-bindid-sha256-v2`；
- 使用完整 SHA-256 十六进制值，不截断为 64 bit；
- 必需核心类别为 system/motherboard、CPU、memory、storage、physical network；
- GPU 在可稳定识别时参与，仍允许在无 GPU 身份时生成结果；
- 使用 system UUID、主板 serial/product、CPU vendor/model/socket identity、内存 serial/part/locator、存储 WWN/serial/model、物理网卡永久 MAC、GPU PCI identity 等稳定字段；
- 不包含 BIOS/UEFI 版本、设备固件、内核、驱动、温度、性能和网络状态；
- 数据库和上传 DTO 必须同时携带 `bindid_algorithm` 与 `machine_bind_id`，禁止只传裸值。

主要物理部件变化导致 bindid v2 变化，并按新的机器身份处理。固件、内核或驱动变化不改变 bindid，但会改变 `configuration_fingerprint`，在同一个 machine bindid 下生成新 snapshot。当前 bindid v1 不包含主板固件，因此刷 BIOS 不会改变 v1；v2 也有意保持这个规则。

### 5.4 通用默认策略

- `PublishIfCoreComplete` 作为 partial 的默认发布策略；
- `qurbrix_hwinfo.db` 默认位于 `/var/lib/qurbrix-hwinfo/`；
- 完整报告默认位于 `/var/lib/qurbrix-hwinfo/reports/<snapshot_id>.json`；
- 默认 snapshot TTL 为 24 小时，测试工作流可以使用 `force_full_scan`；
- 驱动、内核、BIOS/UEFI 和核心设备固件版本变化产生新 snapshot，但不产生新 machine bindid；
- 默认外部命令并发上限为 4，可配置并通过基准测试调整；
- 同时提供 Rust API 和 CLI，CLI 是其他语言使用的稳定边界；
- 默认保留当前快照、所有未上传/被固定的快照，以及每个 machine bindid 最近 30 个快照；已上传且未固定、超过 90 天并且不在最近 30 个内的快照可清理。

### 5.5 核心完整性矩阵

`PublishIfCoreComplete` 和 bindid v2 不绑定单一厂商数据源。每个必需身份组至少需要形成一个过滤占位符后的稳定 component key：

| 身份组 | 优先字段 | 通用 fallback | 是否必需 |
|---|---|---|---|
| platform | system UUID、主板 serial | device tree serial、manufacturer + product + board product | 是 |
| CPU | vendor/implementer + model/part | architecture + socket/topology + stable processor identity | 是 |
| memory | module serial | part number + locator、总容量 + 稳定插槽拓扑 | 是 |
| storage | WWN、serial | model + 稳定控制器/总线路径 | 是 |
| physical network | permanent MAC | 稳定硬件地址；排除 loopback、虚拟和随机 MAC | 是 |
| GPU | PCI vendor/device/subsystem ID | 稳定 model/name，过滤软件 renderer | 否 |

某个优先字段缺失不直接导致失败；只有整个必需身份组无法产生稳定 key 才认为核心合约不完整。算法必须保存已覆盖和缺失的身份组，便于解释为什么不能发布 snapshot。

“必需”表示必须可信地完成该组枚举，不表示机器必须物理存在该类设备。成功枚举后确认无物理网卡、GPU 或本地存储时，使用版本化的 canonical absence marker；数据源权限不足、超时或失败不能伪装成“设备不存在”。这样既支持无 GPU、无网卡或无盘系统，又能在设备实际增加/移除时稳定改变 bindid。

### 5.6 实现技术默认值

- SQLite 驱动使用 `rusqlite`，数据库调用放入专用 blocking store/worker，不阻塞 Tokio executor；
- 使用 `rusqlite` 的 bundled SQLite 构建保证发行版和跨架构行为一致，发布前验证目标体积和交叉编译；
- migration 使用随 crate 编译的顺序 SQL 文件和独立 schema version 表；
- UUID 使用支持 RFC 9562 UUIDv7 的成熟 crate；
- bindid v2、configuration fingerprint 和 artifact 校验使用 `sha2::Sha256`；
- JSON 使用 `serde_json`，V1 保存普通 UTF-8 `.json`，暂不压缩以保证可检查性；
- 默认状态目录权限为 `0700`、数据库和 artifact 权限为 `0600`，调用方可显式配置受控的 group 访问。

## 6. 数据库设计草案

数据库文件名确定为 `qurbrix_hwinfo.db`，默认路径为 `/var/lib/qurbrix-hwinfo/qurbrix_hwinfo.db`。库调用方可以显式覆盖状态目录，CLI 使用该系统级默认值。数据库只保存清单快照和采集历史，不保存监控指标或热插拔事件。

### 6.1 `inventory_state`

保存最近一次已发布快照的指针和最近一次 quick probe 状态。单机数据库原则上只有一条记录。这里的“当前”表示最近一次成功调用得到的状态，不表示后台持续同步的实时状态。

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `id` | INTEGER PRIMARY KEY | 固定为 1，约束单例状态 |
| `current_snapshot_id` | TEXT NULL | 当前有效 UUIDv7 快照 |
| `current_machine_bind_id` | TEXT NULL | 当前 bindid v2 机器身份 |
| `bindid_algorithm` | TEXT NULL | bindid 算法版本 |
| `last_configuration_fingerprint` | TEXT NULL | 最近成功并用于比较的配置指纹 |
| `core_identity_count` | INTEGER NULL | 本次指纹包含的核心身份记录数 |
| `fingerprint_version` | INTEGER NULL | 指纹算法版本 |
| `last_quick_probe_at` | TEXT NULL | 最近 quick probe 时间 |
| `updated_at` | TEXT NOT NULL | 状态更新时间 |

### 6.2 `hardware_snapshot`

保存一次完整扫描的头信息。快照发布后不可原地修改。

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `snapshot_id` | TEXT PRIMARY KEY | 应用生成的 UUIDv7 快照 ID |
| `created_at` | TEXT NOT NULL | 创建时间，UTC RFC 3339 |
| `scan_status` | TEXT NOT NULL | 已发布快照只能是 `complete` 或策略允许的 `partial`；失败记录进入 `probe_history` |
| `schema_version` | TEXT NOT NULL | ScanReport schema 版本 |
| `scanner_version` | TEXT NULL | qurbrix-hw 版本 |
| `machine_bind_id` | TEXT NOT NULL | 该快照所属的 bindid v2 机器身份 |
| `bindid_algorithm` | TEXT NOT NULL | bindid 算法版本 |
| `configuration_fingerprint` | TEXT NOT NULL | 该快照的规范化软硬件配置指纹 |
| `device_count` | INTEGER NOT NULL | 设备数量 |
| `warning_count` | INTEGER NOT NULL | warning 数量 |
| `duration_ms` | INTEGER NULL | 完整扫描耗时 |

### 6.3 `snapshot_device`

保存快照中的设备明细，用于按类别、设备 ID 等条件查询。

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `snapshot_id` | TEXT NOT NULL | 所属快照 |
| `device_id` | TEXT NOT NULL | 当前 `Device.id` |
| `kind` | TEXT NOT NULL | 设备类别 |
| `name` | TEXT NOT NULL | 展示名称 |
| `vendor` | TEXT NULL | 厂商 |
| `model` | TEXT NULL | 型号 |
| `serial` | TEXT NULL | 序列号，日志中需注意脱敏 |
| `bus_kind` | TEXT NULL | 总线类型 |
| `bus_address` | TEXT NULL | 可查询的总线地址 |
| `driver_name` | TEXT NULL | 当前驱动名称 |
| `driver_status` | TEXT NULL | 驱动状态 |
| `parent_device_id` | TEXT NULL | 父设备 ID |

建议主键为 `(snapshot_id, device_id)`，并建立 `(snapshot_id, kind)` 索引。外键删除策略使用 `ON DELETE CASCADE`。

不保存 `report_json` 或 `device_json`。关系化存储由以下附属表组成：

### 6.4 `snapshot_device_identifier`

保存一个设备的多个稳定身份，例如 MAC、WWN、PCI vendor/device ID、USB serial。字段建议为 `(snapshot_id, device_id, identifier_kind, identifier_value)`，并按 `identifier_kind/identifier_value` 建索引。

### 6.5 `snapshot_device_property`

保存无法全部提升为固定列的属性，采用带类型的 key/value，而不是 JSON：

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `snapshot_id` | TEXT NOT NULL | 所属快照 |
| `device_id` | TEXT NOT NULL | 设备 ID |
| `property_key` | TEXT NOT NULL | 版本化属性名，例如 `memory.speed_mtps` |
| `value_type` | TEXT NOT NULL | `text`、`integer`、`real`、`boolean` |
| `text_value` | TEXT NULL | 字符串值 |
| `integer_value` | INTEGER NULL | 整数值 |
| `real_value` | REAL NULL | 浮点值 |
| `boolean_value` | INTEGER NULL | 0 或 1 |
| `unit` | TEXT NULL | 单位，例如 `bytes`、`mhz` |
| `ordinal` | INTEGER NOT NULL DEFAULT 0 | 数组属性顺序 |

每一行只能填充与 `value_type` 对应的值列。属性名和类型属于持久化 schema，升级时通过 migration 处理。

建议主键为 `(snapshot_id, device_id, property_key, ordinal)`，并分别为 `(property_key, text_value)`、`(property_key, integer_value)` 和 `(property_key, real_value)` 建索引。高频查询且跨设备通用的字段应提升到 `snapshot_device` 固定列，长尾属性才进入 typed property 表。

温度、实时频率、利用率、功耗、流量、风扇转速、SMART 动态计数和电池实时状态等热属性不得写入 `snapshot_device_property`。当前 `DeviceProperties` 中已经存在的热字段需要建立审计清单：V1 持久化映射明确忽略它们，后续通过外部 schema 版本升级决定是否从 hwinfo 输出模型移除并迁移到 `qurbrix-monitor`。

为保持现有 `ScanReport v2` 兼容，V1 JSON artifact 可以暂时包含其中已有的热字段，但这些字段不参与 bindid、configuration fingerprint、关系化投影或默认上传 DTO，也不视为 snapshot 的权威属性。后续外部 schema 升级后从 hwinfo 报告移除，实时采集由 `qurbrix-monitor` 接管。

### 6.6 `snapshot_device_relation`

保存 parent/child 和 backing device 关系，避免把设备树塞进 JSON。建议主键为 `(snapshot_id, parent_device_id, child_device_id, relation_kind)`。

### 6.7 `snapshot_warning`

保存快照 warning，字段对应 `ScanWarning` 的 `code`、`message`、`source` 和 `device_id`，便于按严重性和设备查询。

### 6.8 `snapshot_source`

保存 source evidence 的结构化记录，包括 source、kind、status、summary。原始命令输出不进入数据库。

### 6.9 `probe_history`

记录每次 quick/full probe 尝试，包括失败记录。

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `probe_id` | INTEGER PRIMARY KEY | 探测记录 ID |
| `probe_type` | TEXT NOT NULL | `quick` 或 `full` |
| `started_at` | TEXT NOT NULL | 开始时间 |
| `finished_at` | TEXT NULL | 结束时间 |
| `status` | TEXT NOT NULL | `running`、`succeeded`、`partial`、`failed` |
| `snapshot_id` | TEXT NULL | 成功产生或复用的快照 |
| `previous_snapshot_id` | TEXT NULL | 执行前的当前快照 |
| `machine_bind_id` | TEXT NULL | 本次观察到的机器身份 |
| `configuration_fingerprint` | TEXT NULL | 本次观察到的配置指纹 |
| `duration_ms` | INTEGER NULL | 耗时 |
| `warning_count` | INTEGER NULL | warning 数量 |
| `error_code` | TEXT NULL | 稳定错误代码 |
| `error_message` | TEXT NULL | 诊断信息，避免包含敏感原始数据 |

### 6.10 `snapshot_artifact`

完整 `ScanReport` 不存入 SQLite，而是保存为磁盘上的不可变 JSON 文件。数据库只保存 artifact 元数据：

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `snapshot_id` | TEXT PRIMARY KEY | 所属快照 |
| `artifact_kind` | TEXT NOT NULL | V1 固定为 `scan_report_json` |
| `relative_path` | TEXT NOT NULL | 相对状态目录的安全路径 |
| `sha256` | TEXT NOT NULL | 文件内容校验值 |
| `size_bytes` | INTEGER NOT NULL | 文件大小 |
| `schema_version` | TEXT NOT NULL | artifact 内容 schema |
| `created_at` | TEXT NOT NULL | 文件创建时间 |

artifact 发布协议：

1. 使用目标 `snapshot_id` 在同一文件系统写入临时文件；
2. 完成 JSON 序列化、flush、fsync 和 SHA-256 计算；
3. 原子 rename 为 `reports/<snapshot_id>.json`；
4. 在 SQLite 事务内写入关系化投影、artifact 元数据和 current pointer；
5. 数据库事务失败时删除新 artifact；进程在 rename 后崩溃产生的孤儿文件由下次启动清理；
6. 读取完整报告时验证路径边界、大小、SHA-256 和 schema version；
7. 删除历史快照时，在数据库事务提交后删除对应 artifact，并对失败删除进行可恢复记录。

SQLite 无法和文件系统形成单一 ACID 事务，因此不得先提交数据库再写 artifact，也不得在 artifact 缺失或校验失败时返回成功快照。

### 6.11 `snapshot_lifecycle`

快照内容保持不可变，上传和保留状态单独存放：

| 字段 | 建议类型 | 说明 |
|---|---|---|
| `snapshot_id` | TEXT PRIMARY KEY | 所属快照 |
| `pinned` | INTEGER NOT NULL DEFAULT 0 | 是否禁止自动清理 |
| `uploaded_at` | TEXT NULL | 外部上传器确认成功的时间 |
| `delete_pending` | INTEGER NOT NULL DEFAULT 0 | artifact 删除失败后的重试标记 |
| `updated_at` | TEXT NOT NULL | lifecycle 更新时间 |

### 6.12 查询和上传原则

关系化表是设备查询、过滤、比较和上传投影的权威索引；完整 JSON artifact 是原始 `ScanReport` 的不可变归档和回放来源。两者必须通过 `snapshot_id`、schema version 和 SHA-256 保持可验证的一致性。常见查询必须有直接 SQL 和索引支持，例如：

- 按 `snapshot_id` 查询全部设备；
- 按 `snapshot_id + kind` 查询某类设备；
- 按 MAC、WWN、serial、PCI ID 查询设备；
- 按属性名和值查询容量、核心数、内存速度等字段；
- 比较两个 snapshot 的新增、移除和属性变化。

上传服务器时，由 repository/service 从关系表分页读取并组装独立、版本化的上传 DTO；需要原始报告时也可以读取并校验 artifact。不得直接上传 SQLite 文件，也不得让服务端 API 依赖本地表结构。上传幂等键使用 `(bindid_algorithm, machine_bind_id, snapshot_id)`。

## 7. `quick_probe()` 设计

### 7.1 数据源选择原则

quick probe 必须满足：

- 明显快于完整扫描；
- 不依赖高延迟的可选命令；
- 采集结果能够稳定排序和规范化；
- 设备重启后尽量保持稳定；
- 能发现业务关心的增加、移除和替换。

quick probe 是一次普通的按需读取。它可以枚举调用时的 `/sys`、`/proc` 或轻量命令输出，但不得订阅 udev/netlink 事件，不得创建监听线程，也不得在函数返回后继续运行。

第一版 quick probe 确定只覆盖核心硬件：

- system/motherboard 的 UUID、serial 和产品身份；
- CPU 的稳定型号、厂商和 socket/topology 身份，不包含实时频率、温度或 governor；
- 内存模块的 locator、serial、part number 和容量；
- 存储 WWN、序列号或稳定替代标识；
- 物理网卡永久 MAC 和稳定总线身份，排除 loopback、虚拟接口、IP 和链路状态；
- GPU 的 PCI 地址、vendor/device/subsystem ID。

此外 quick probe 采集少量会影响测试结论的配置身份字段：

- kernel release；
- BIOS/UEFI vendor、version 和 release date；
- 绑定到核心设备的驱动名称和可获得的模块版本；
- 可低成本读取的核心设备 firmware 版本。

quick probe 同时生成 `machine_bind_id` 和 `configuration_fingerprint`。前者只使用稳定物理身份；后者以 bindid v2、上述配置字段和 fingerprint version 为输入。无法低成本读取的设备固件由 TTL 或 `force_full_scan` 在 full scan 中补充并重新计算配置指纹。

USB、显示器、音频、蓝牙、输入、摄像头、打印机、CD-ROM、电池以及其他可热插拔外设不参与 quick fingerprint。它们仍可以在调用 full scan 时进入该次完整硬件快照，但不会单独触发 quick probe 判定核心硬件已变化；其插拔事件归 `qurbrix-monitor`。

因此，只有核心硬件变化、快照 TTL 到期、调用方强制刷新或直接 full scan 才会创建包含最新外设清单的新快照。单纯外设插拔不会主动或通过 core quick fingerprint 创建新快照。

不建议直接把以下易变字段放入硬件变化指纹：

- IP 地址；
- 网络链路状态和速率；
- 温度、容量使用率和电量；
- CPU/GPU 利用率、实时频率和功耗；
- 磁盘 I/O、网络吞吐量和风扇转速；
- USB 临时 bus/device 编号；
- `/dev/sdX` 等可能重排的节点名；
- 用户态软件包版本、桌面配置和与硬件测试无关的操作系统状态。

### 7.2 指纹生成

1. 将 bindid 物理身份字段和 configuration 配置字段分别转成明确的版本化结构；
2. 清理空白、统一大小写和标识格式；
3. 对类别和记录进行确定性排序；
4. 使用稳定 JSON 或其他规范编码序列化；
5. 对序列化结果计算 SHA-256；
6. 分别输出 `machine_bind_id` 和 `configuration_fingerprint`，并保存各自算法/版本。

不得直接对 `HashMap` 的非确定性序列化结果计算指纹。

## 8. `ensure_snapshot()` 状态机

建议执行流程：

1. 打开数据库并执行 migration；
2. 创建一条 `probe_history(quick, running)`；
3. 执行 quick probe；
4. 读取 `inventory_state`；
5. 若当前快照存在、machine bindid 与 configuration fingerprint 均一致、未过期且没有强制刷新：
   - 将 quick history 标记为成功；
   - 记录复用的 `snapshot_id`；
   - 返回现有 `snapshot_id`；
6. 否则创建 `probe_history(full, running)` 并执行完整扫描；
7. 验证完整扫描结果是否满足发布策略；
8. 生成 UUIDv7 snapshot ID，写入、fsync、rename 并校验完整 JSON artifact；
9. 在一个数据库事务内：
   - 插入 `hardware_snapshot`；
   - 批量插入 `snapshot_device`；
   - 插入 `snapshot_artifact` 和初始 `snapshot_lifecycle`；
   - 更新 `inventory_state.current_snapshot_id`、machine bindid 和 configuration fingerprint；
   - 更新 full probe history；
10. 提交事务后返回新 `snapshot_id`；
11. 任一步失败时记录错误、清理或标记孤儿 artifact，并继续保留原来的当前快照。

核心约束：调用方只能看到旧快照或完整的新快照，不能看到写入一半的数据。

新鲜度约束：`ensure_snapshot()` 只在被调用时检查硬件状态。两次调用之间发生的热插拔不会主动改写 `qurbrix_hwinfo.db`，下一次调用通过 quick probe 发现核心设备集合变化后再决定是否创建新快照。

状态转移和不变量：

```text
Start(current_snapshot_id?)
    -> QuickProbing
       -> 当前快照存在、machine bindid 与 configuration fingerprint 相同且未过期
          -> Reused(snapshot_id)
       -> 首次运行、指纹变化、过期、强制刷新或 quick 失败
          -> FullScanning(reason)
             -> 结果满足发布策略
                -> ArtifactWriting
                   -> Publishing
                      -> Published(new_snapshot_id)
             -> 失败或结果不满足发布策略
                -> Failed(old_snapshot_id remains current)
```

- `QuickProbing` 和 `FullScanning` 是一次调用的临时状态，不能被当作可读快照；
- `ArtifactWriting` 完成前不能创建可见数据库快照；
- `Publishing` 只在一个 SQLite 事务内存在，事务失败时 artifact 必须被清理或留待孤儿回收；
- `Published` 的快照行和所有 `snapshot_device` 行提交后不可更新；
- `Failed` 不改变 `inventory_state.current_snapshot_id`，首次扫描失败时没有可返回的 ID；
- quick 身份或配置指纹变化但 full scan 失败时，不覆盖旧的 `current_machine_bind_id` 和 `last_configuration_fingerprint`，否则下次调用可能错误复用旧快照；
- 恢复任务只处理超时的 `probe_history`/租约，不自动把半成品标记为成功。

## 9. 并发和事务

必须覆盖多个进程同时调用 `ensure_snapshot()` 的情况：

- 开启 SQLite WAL 模式和合理的 busy timeout；
- 使用数据库级互斥租约、锁表或 `BEGIN IMMEDIATE` 串行化快照发布；
- 获得写锁后再次检查当前指纹，避免等待期间其他进程已经创建相同快照；
- `hardware_snapshot`、`snapshot_device`、`inventory_state` 必须在同一事务中更新；
- 扫描耗时较长，不应在持有 SQLite 写事务期间执行完整硬件扫描；
- 需要设计短期扫描租约，处理进程崩溃后遗留的 `running` 状态。

### 9.1 完整扫描的并发提升

当前 `hw-collect` 在 `crates/hw-collect/src/collector.rs:47` 按列表顺序串行执行 probe。并发优化必须以这个行为和耗时为基线，不能直接无脑并行所有 probe，因为部分 probe 会共享 source、产生 consumed 引用或参与最终 PCI 去重。

目标方案：

1. 先为每个 probe 和 source 记录耗时、调用次数、超时和输出设备数，建立真实基线；
2. 给 probe/source 声明依赖和副作用，区分可并行、需要共享缓存和必须串行的阶段；
3. 在一次 full scan 内增加只读 `SourceCache`，按规范化的 command/path/config 去重，避免多个 probe 重复执行 `lspci`、`lshw` 等命令；
4. 对无依赖 probe 使用 `FuturesUnordered` 或等价机制并行执行，并通过 `Semaphore` 限制同时运行的外部命令数量；
5. 并发度可配置，默认值通过基准测试确定，不把 CPU 核数直接当作外部命令并发上限；
6. 按固定 probe 顺序合并 devices、warnings、consumed 和 sources，保证并发前后 JSON、去重结果和 warning 顺序稳定；
7. SQLite 发布事务仍保持串行且短，不在事务内执行硬件扫描；
8. 对单独的 `kinds`/`exclude_kinds` 扫描沿用同一依赖图，不能因并行绕过过滤规则；
9. 设置全局扫描截止时间和单 source 超时，取消后记录未完成 probe，不留下不可见后台任务。

并发优化的顺序固定为：基线测量 -> source 去重 -> 有界 probe 并发 -> 确定性合并 -> 数据库批量写入优化。每一步都必须通过现有 fixture 和状态机测试后再进入下一步。

性能门槛：

- 并发前后设备、warning、source 和状态结果等价；
- 并发度达到上限时不会启动超过配置数量的外部命令；
- 在当前机器和延迟 fixture 上，full scan P95 相比串行基线至少降低 25%；如果受硬件、权限或测量噪声限制，GPT 根据至少 10 轮样本自主确定可复现的新目标，并在执行日志中保存原始数据、判断依据和回归上限；
- 在单核/资源受限环境中，耗时和失败率不能比串行基线恶化超过 10%；
- quick probe 不得因为 full scan 的并发改造引入后台任务或热插拔依赖。

## 10. 错误和降级策略

扫描状态定义：

- `complete`：请求范围内的必需数据源和核心字段完整，没有影响清单可信度的重要 warning；
- `partial`：已经生成可用清单，但至少一个数据源缺失、权限不足、超时或失败，部分字段通过 fallback 获得或保持为空；
- `failed`：无法形成满足最低合约的硬件清单。

例如，`dmidecode` 因权限失败，但 `/sys` fallback 仍拿到了机器身份和主要设备，这通常是 `partial`；如果主板、存储等核心身份不足，无法形成可信清单，则按 `failed` 或不可发布的 `partial` 处理。

当前实现的实际判定比较宽：设备数为 0 是 `failed`，设备数大于 0 且没有 warning 是 `complete`，设备数大于 0 但存在任意 warning 就是 `partial`。快照发布层需要在这个扫描状态之上增加“核心合约是否完整”的判断。

V1 默认策略确定为 `PublishIfCoreComplete`：partial 但核心硬件合约完整时允许发布，并保存 warning；缺少任何必需核心身份时不发布新快照。`Reject` 策略可供要求所有数据源完整的调用方使用。

- quick probe 失败：默认尝试 full scan，而不是错误地复用可能过期的快照；
- full scan 失败：不更新当前快照，返回明确错误；
- full scan 为 `partial`：按 `PublishIfCoreComplete` 或显式 `Reject` 策略决定是否发布；
- 数据库损坏或 migration 失败：停止发布，不静默重建并丢失历史；
- 旧快照读取失败：视为不可复用并触发 full scan；
- JSON 序列化失败：事务回滚；
- 磁盘空间不足：事务回滚并保留旧快照。

## 11. 隐私和权限

硬件报告可能包含主机名、MAC、设备序列号等稳定标识：

- 数据库权限默认限制为当前用户或 root 可读写；
- 日志和 `probe_history.error_message` 不输出完整序列号和原始报告；
- 不将数据库放入 Git 仓库；
- 在 `.gitignore` 中忽略运行时数据库及 SQLite 的 `-wal`、`-shm` 文件；
- 文档明确数据位置、所有者和清理方法。

## 12. 分阶段实施

### 阶段 A：确定合约

- 用调用场景、不变量、失败语义和 golden fixture 确定 V1 数据合约；
- 固化 UUIDv7 `SnapshotId`、`QuickProbeReport`、`StoredSnapshot` 和错误类型；
- 固化 bindid v1 兼容边界和 SHA-256 bindid v2 机器身份合约；
- 固化 machine bindid、configuration fingerprint、snapshot ID 三层身份模型；
- 固化状态转移表以及每个转移允许修改的数据库字段；
- 固化 quick probe 只覆盖 system/motherboard、CPU、memory、storage、physical network 和 GPU 核心身份；
- 固化主要物理部件变化产生新 machine bindid，固件/内核/驱动变化只产生新 snapshot；
- 固化按需调用、非实时、无热插拔监听的项目边界；
- 固化 `qurbrix-hwinfo` 与 `qurbrix-monitor` 零依赖的约束；
- 固化 `PublishIfCoreComplete` 作为 partial 默认发布策略；
- 固化默认 TTL 为 24 小时；
- 固化 `/var/lib/qurbrix-hwinfo/` 默认路径和 30 个/90 天保留策略；
- 固化完整 ScanReport JSON artifact 与关系化数据库投影的一致性合约；
- 固化 API 和 schema 草案。

完成标准：关键策略没有未决歧义，ADR、字段映射、golden vectors 和状态转移测试齐全，阶段 A 自动化门禁全部通过；完成与否由 GPT 自主判定。

### 阶段 B：持久化基础

- 创建 `hw-inventory` crate；
- 引入 SQLite 依赖；
- 添加 migration 和 schema 版本管理；
- 实现 snapshot、device、state 和 history 的读写；
- 实现 identifier、typed property、relation、warning 和 source 的关系化映射，不保存完整 report/device JSON；
- 实现 `snapshot_artifact`、原子 JSON 文件发布、SHA-256 校验和孤儿文件恢复；
- 实现基于关系表的分页查询和上传 DTO 投影；
- 添加事务回滚、外键和索引测试。

完成标准：能够把构造的 `ScanReport` 写成校验通过的 JSON artifact，并将快照合约投影原子写入关系表；两种读取路径都通过一致性测试。

### 阶段 C：quick probe

- 实现版本化的 `QuickProbeReport`；
- 实现并测试 bindid v2，同时保持 bindid v1 输出兼容；
- 复用已有 source/probe 能力，避免重复解析器；
- 实现 machine bindid 和 configuration fingerprint 的规范化、排序和 SHA-256；
- 针对设备顺序变化、字段空白和易变字段添加测试；
- 测量常见机器上的耗时。
- 检查依赖树和运行行为，确保未引入 udev 监听或后台任务。

完成标准：相同机器和配置重复运行产生相同结果；主要物理部件变化改变 bindid；固件、内核或驱动变化保持 bindid 但改变 configuration fingerprint。

### 阶段 D：完整扫描性能与有界并发

- 记录串行扫描中每个 probe/source 的 P50/P95、调用次数和超时情况；
- 建立 probe/source 依赖图和串行白名单；
- 实现单次扫描内的 source 请求合并与结果缓存；
- 实现默认上限为 4、可配置且受 semaphore 限制的 probe/source 并发；
- 保持设备、warning、source 和 consumed 的确定性合并；
- 增加全局扫描截止时间、取消和子进程清理；
- 优化 snapshot_device 的 prepared statement 批量写入。

完成标准：并发结果与串行基线等价，满足 9.1 节的并发上限和性能门槛。

### 阶段 E：编排服务

- 实现 `full_scan()` 薄封装；
- 实现 `ensure_snapshot()` 状态机；
- 实现 force、24 小时默认 TTL 和 partial 发布策略；
- 实现失败保留旧快照；
- 实现并发去重和崩溃恢复。

完成标准：首次调用创建快照，未变化时复用 ID，变化后产生新 ID，失败时旧 ID 保持有效。

### 阶段 F：对外接口

- 从顶层 facade 导出稳定 API；
- 增加 CLI 子命令，例如 `snapshot ensure/show/list/diff/export`；
- 同时导出类型安全的 Rust snapshot/store API；
- 更新中英文 README；
- 明确数据库路径、退出码和 JSON 合约。

完成标准：Rust 调用方和 CLI 调用方都能基于 `snapshot_id` 使用快照。

### 阶段 G：维护能力

- 增加历史快照保留数量或保留天数配置；
- 实现当前、未上传、固定、最近 30 个和 90 天的默认保留规则；
- 删除旧快照时保持当前快照不受影响；
- 支持按两个 `snapshot_id` 输出设备变化；
- 增加数据库健康检查和可观测指标。

完成标准：数据库能够长期运行，不会无上限增长，并能解释快照产生原因。

## 13. 测试计划

### 单元测试

- quick probe 规范化和确定性排序；
- bindid v1 兼容向量和 bindid v2 算法版本；
- machine bindid 与 configuration fingerprint 字段隔离；
- ScanReport JSON artifact 序列化、SHA-256 校验和读取往返；
- ScanReport 快照合约字段与数据库投影映射；
- UUIDv7 snapshot ID 的生成、解析、排序和跨进程唯一性；
- Device 的快照合约字段与关系化表之间完整映射，不依赖 JSON blob；
- 状态机各分支；
- partial/failed 策略；
- migration 幂等性；
- 同一输入在不同任务完成顺序下得到完全一致的合并结果；
- source cache 对相同请求只执行一次；
- semaphore 的实际并发数不超过配置上限；
- 全局取消后不遗留运行中的子进程或后台任务。

### 集成测试

- 空数据库首次创建快照；
- 确认数据库文件名为 `qurbrix_hwinfo.db`；
- 首次创建同时产生关系化记录和 `reports/<snapshot_id>.json`；
- 指纹不变时复用相同 ID；
- 主要物理部件变化时 bindid 和 snapshot ID 都变化；
- BIOS/固件、内核或驱动变化时 bindid 不变但 snapshot ID 变化；
- full scan 失败后当前 ID 不变；
- 写入中途失败时事务完全回滚；
- artifact 写入、rename、数据库提交各崩溃点均可恢复，不返回缺失 artifact 的成功快照；
- JSON 文件被截断或篡改时 SHA-256 校验失败；
- 两个并发调用只发布一个等价快照；
- 多个 probe 并发完成时，最终输出与串行基线等价；
- 进程崩溃后的 `running` history 恢复；
- 老版本数据库升级；
- 两次调用之间改变模拟设备集合，确认数据库不会被后台修改，下一次调用时才生成新快照；
- 在不安装、不启动 `qurbrix-monitor` 的环境中完成全部 hwinfo 测试；
- 从关系表分页组装上传 DTO，并验证与预期设备清单一致。

### 性能验证

- quick probe P50/P95 耗时；
- full scan 串行基线与有界并发版本的 P50/P95；
- 每个 probe/source 的耗时、调用次数和缓存命中率；
- 不同并发度下的耗时、CPU、内存和外部进程峰值；
- 单快照和多快照数据库体积；
- 1000 个设备记录的事务写入和查询耗时；
- WAL 文件在持续运行下能否正常 checkpoint。

## 14. 验收标准

1. 首次 `ensure_snapshot()` 成功返回一个存在的 `snapshot_id`。
2. 硬件未变化时重复调用返回同一个 ID，且不执行 full scan。
3. 目标硬件变化后返回新 ID，旧快照仍可查询。
4. 新快照在所有设备写完之前不可见。
5. full scan 失败不会覆盖当前有效快照。
6. 并发调用不会发布重复的等价快照。
7. 数据库升级有自动化 migration 测试。
8. 当前 `collect_scan_report()`、CLI 输出和退出码保持兼容。
9. `cargo test --workspace` 全部通过。
10. 中英文文档包含 API、数据库位置、权限和清理说明。
11. 项目不注册热插拔监听器，不产生温度、性能或事件时间序列数据。
12. `qurbrix-hwinfo` 在没有 `qurbrix-monitor` 的环境中可独立构建、测试和运行。
13. V1 数据合约具有 golden fixture、schema 版本和完整状态转移测试。
14. 并发扫描与串行基线结果等价，外部命令并发数始终受配置上限约束。
15. full scan 达到 9.1 节约定的 P95 提升目标；受环境限制时，由 GPT 根据可复现样本确定新基线，并留下原始数据、自动判定结果和原因。
16. 所有 `snapshot_id` 均为有效 UUIDv7，来自不同机器的快照可直接汇聚而不重新编号。
17. bindid v1 保持兼容；bindid v2 使用完整 SHA-256，主要物理部件变化产生新的 machine bindid。
18. 固件、内核和驱动变化不改变 machine bindid，但产生新的 configuration fingerprint 和 snapshot ID。
19. `qurbrix_hwinfo.db` 不包含完整 report/device JSON，常见设备和属性查询都有关系表与索引支持。
20. 每个成功快照都有 SHA-256 校验通过的完整 ScanReport JSON artifact，数据库与文件崩溃恢复测试通过。
21. 能从关系表生成版本化上传 DTO，也能按需读取完整 JSON artifact。
22. CLI 与 Rust API 都能 ensure、查询和导出同一份 snapshot 数据。

## 15. 已确定的设计决策

- 热插拔属于热数据，由 `qurbrix-monitor` 管理；
- 温度和性能变化由 `qurbrix-monitor` 管理；
- `qurbrix-hwinfo` 不监听事件，仅在调用时读取当前硬件状态；
- `qurbrix-hwinfo` 与 `qurbrix-monitor` 互不依赖；
- quick probe 只覆盖核心硬件，USB 等热插拔外设不参与 fingerprint；
- 使用 bindid 表达机器身份，主要物理部件变化视为新的机器和测试结论域；
- 保留 bindid v1 兼容输出，新增完整 SHA-256 bindid v2 作为 machine bindid；
- CPU 纳入 bindid v2；BIOS/固件、内核和驱动不纳入 bindid；
- BIOS/固件、内核和驱动变化会改变 configuration fingerprint 并生成新 snapshot；
- `snapshot_id` 使用跨机器唯一的 UUIDv7；
- 数据库文件名为 `qurbrix_hwinfo.db`；
- 默认状态目录为 `/var/lib/qurbrix-hwinfo/`；
- 数据库不保存完整 report/device JSON，以关系表作为查询和上传投影；
- 完整 ScanReport 保存为 `reports/<snapshot_id>.json` 不可变 artifact，并由数据库记录 SHA-256；
- `PublishIfCoreComplete` 是 partial 默认发布策略；
- 默认 TTL 为 24 小时，测试前可强制 full scan；
- full scan 默认外部命令并发上限为 4，并保持可配置；
- 默认保留当前、未上传、固定和每个 bindid 最近 30 个快照；其他已上传快照超过 90 天可清理；
- CLI 与 Rust API 同时提供；
- `qurbrix_hwinfo.db` 表示最近一次扫描状态和历史快照，不表示实时状态。

## 16. 推荐的第一步

设计决策已经收口。实现首先完成阶段 A 的正式 ADR、字段映射表、bindid v2 golden vectors、configuration fingerprint vectors 和状态转移测试；同时记录当前串行 full scan 的 probe/source 基线。随后按阶段 B 使用既定的 `rusqlite` 方案实现 migration、关系化投影和 JSON artifact 原子发布。

## 17. GPT 无人值守执行协议

本节是实施时的最高优先级 runbook。GPT 从阶段 A 连续执行到阶段 G，不等待人工确认，也不得把“需要评审”“需要选择”作为暂停理由。遇到实现分歧时，按本节的自主决策规则选择并记录；只有客观环境故障导致代码无法构建或测试连续失败时才允许停止。

### 17.1 执行授权和边界

- 允许创建和修改本仓库内完成 A-G 所需的源码、测试、fixture、migration、文档、许可证和 Cargo 依赖；
- 允许访问网络查询依赖、标准和上游许可证，允许下载 Cargo 依赖；
- 允许创建本地分支、本地 commit 和本地 tag，但不得 push、创建远程 PR、发布 crate 或触发远程 release；
- 不使用 `sudo`，不修改真实 `/var/lib/qurbrix-hwinfo/`；所有测试和 smoke test 使用 `tempdir` 或仓库 `target/` 下的隔离目录；
- 不修改、删除或提交与本计划无关的用户改动。当前已知未跟踪的本计划文件属于本任务，应纳入基线 commit；
- 不引入 `qurbrix-monitor` 依赖、后台守护进程、热插拔监听或监控时间序列；
- 不为了通过测试删除、忽略或弱化既有测试，不使用宽泛 `allow` 隐藏 lint，不以跳过测试代替修复；
- 不要求人工做许可证决定。发生 GPL 派生复用时，按第 3.2 节自动完成全仓库 GPL-3.0-or-later 转换；否则保持 MIT OR Apache-2.0。

### 17.2 启动步骤

1. 读取 `AGENTS.md`、`CONTRIBUTING.md`、workspace manifests、CI workflow 和与当前阶段相邻的实现及测试；若没有 `AGENTS.md`，继续执行。
2. 运行 `git status --short --branch`，记录 HEAD、分支和已有改动。除本计划文件外的已有改动一律保留并排除在本任务 commit 之外。
3. 如果当前不在专用分支，创建 `codex/hardware-snapshot-v0.2.0`；如果分支已存在则继续使用，不重置历史。
4. 将本计划作为不可变执行基线提交：`docs(plan): add autonomous hardware snapshot runbook`。后续如需修正计划，必须单独提交并在执行日志说明原因。
5. 创建 `docs/hardware-snapshot-execution-log.md`，记录起始 commit、环境、Rust 版本、阶段状态、测试结果、性能数据、自主决策、许可证判断和最终 commit。
6. 运行基线门禁并把结果写入执行日志。既有基线失败时先判断是否与任务相关：相关则在阶段 A 修复；无关则记录精确失败并继续，但最终阶段 G 必须使全部门禁通过。

基线和每阶段统一门禁为：

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### 17.3 自主决策优先级

出现文档未覆盖的实现选择时，GPT 按以下顺序决策，无需等待用户：

1. 安全、数据不可丢失、已发布快照不可变和旧接口兼容；
2. 本计划已经确定的 API、状态机、数据库和许可证约束；
3. 仓库现有架构、命名、错误类型、fixture 和测试风格；
4. Rust stable 上维护活跃、许可证兼容且依赖较少的成熟 crate；
5. 最小实现复杂度和最小变更范围；
6. 如果仍有多个等价方案，选择更容易用确定性 fixture 完整验证的方案。

所有会影响公共 API、持久化格式、许可证或性能目标的自主选择都写入执行日志。不得只记录结论，必须同时记录候选方案和选择理由。

### 17.4 阶段、目标、commit 和版本路线

每个阶段必须完成专属目标、专属门禁、统一门禁、Git commit 和 workspace 版本提升后，才可进入下一阶段。版本号必须同时更新根 `Cargo.toml` 和 `Cargo.lock` 中所有 workspace package 条目。每阶段可以包含多个原子工作 commit，但最后必须有且只有一个版本 checkpoint commit。

| 阶段 | 独立交付目标 | 阶段版本 | 必需的阶段收尾 commit |
|---|---|---:|---|
| A | ADR、字段映射、状态转移表、bindid/configuration golden vectors、串行性能基线 | `0.2.0-alpha.1` | `chore(release): bump workspace version to 0.2.0-alpha.1` |
| B | `hw-inventory`、migration、关系投影、artifact 原子发布和存储层测试 | `0.2.0-alpha.2` | `chore(release): bump workspace version to 0.2.0-alpha.2` |
| C | quick probe、bindid v2、configuration fingerprint 和确定性测试 | `0.2.0-alpha.3` | `chore(release): bump workspace version to 0.2.0-alpha.3` |
| D | source cache、有界并发、取消清理、确定性合并和性能报告 | `0.2.0-alpha.4` | `chore(release): bump workspace version to 0.2.0-alpha.4` |
| E | full scan 薄封装、ensure 状态机、租约、去重和崩溃恢复 | `0.2.0-beta.1` | `chore(release): bump workspace version to 0.2.0-beta.1` |
| F | facade API、CLI 全套命令、稳定 JSON 合约及中英文文档 | `0.2.0-rc.1` | `chore(release): bump workspace version to 0.2.0-rc.1` |
| G | retention、diff、health、全量回归、性能验收和发布收口 | `0.2.0` | `chore(release): bump workspace version to 0.2.0` |

版本 checkpoint commit 之前必须确认：

1. 当前阶段专属门禁通过；
2. 四条统一门禁全部通过；
3. `git diff --check` 无错误；
4. README、schema 和 migration 与实现一致；
5. 执行日志包含本阶段测试命令、结果和自主决策；
6. `git status --short` 中没有意外文件；
7. 先提交实现、测试和文档，再修改版本并创建独立 checkpoint commit。

### 17.5 阶段级执行清单

#### 阶段 A：合约和基线

- 输出 ADR，逐字段定义来源、空值、单位、稳定性、敏感性、是否进入 bindid/configuration fingerprint/数据库投影；
- 输出完整状态转移表，覆盖首次、复用、TTL、force、quick 失败、full 失败、partial、并发、崩溃和 artifact 损坏；
- 为 bindid v1 兼容、bindid v2 和 configuration fingerprint 添加 golden vectors；
- 用 fixture 固化对外 DTO 和错误语义；
- 测量当前串行 full scan，至少保存 10 轮 wall time；真实硬件命令缺失时同时使用延迟 fixture 建立可复现基线；
- 专属门禁：ADR 中无 `TBD`/`TODO`/“待确认”，golden 和状态转移测试通过，性能基线文件可由测试或脚本复算；
- 提交建议：`docs(inventory): define snapshot v1 contract`、`test(inventory): add identity and state transition vectors`，然后提升至 `0.2.0-alpha.1`。

#### 阶段 B：持久化基础

- 创建 crate、migration runner、schema version 表和第 6 节全部必要关系表、约束与索引；
- 用 repository/store API 隔离 rusqlite blocking worker，不阻塞 Tokio executor；
- 完成 JSON artifact 临时写入、fsync、原子 rename、SHA-256、数据库发布和孤儿恢复；
- 完成关系投影往返、分页查询和上传 DTO，但不提前实现阶段 E 的业务状态机；
- 专属门禁：migration 幂等、旧版本升级、外键、索引、事务注入失败、各崩溃点恢复、路径穿越和 artifact 篡改测试全部通过；
- 提交建议：按 migration、store、artifact、query/tests 分为原子 commit，然后提升至 `0.2.0-alpha.2`。

#### 阶段 C：quick probe

- 复用已有 probe/source/parser 构建最低成本核心枚举，不复制完整扫描逻辑；
- 实现同一 canonicalizer 下的 bindid v2 和 configuration fingerprint；
- 排除易变字段、虚拟网卡、随机 MAC、软件 renderer 和占位符；
- 记录覆盖及缺失的核心身份组，区分可信 absence 与读取失败；
- 专属门禁：乱序、空白、重复、缺失、权限失败、物理部件变化、固件/内核/驱动变化 fixture 全部通过；重复输入字节级确定；无后台任务；
- 提交建议：按 model/canonicalizer、probe、fixtures/tests 分为原子 commit，然后提升至 `0.2.0-alpha.3`。

#### 阶段 D：完整扫描性能与有界并发

- 严格按“测量 -> source cache -> semaphore -> 确定性合并 -> 批量写入”顺序实现，每一步单独测试和 commit；
- 为 source 请求定义可比较的规范 key、共享结果语义和不得缓存的副作用白名单；
- 为外部命令设置单 source timeout、全局 deadline、取消传播和子进程 reap；
- 专属门禁：串行与并发 golden 完全等价；峰值外部进程不超过配置；取消后无残留任务/子进程；至少 10 轮样本满足第 9.1 节门槛或按 17.3 自主确定并记录新门槛；
- 性能达不到目标时先 profile 并修复最多三轮，不得牺牲正确性、确定性或兼容性换取数字；
- 提交建议：cache、bounded concurrency、cancellation、batching/perf 分开 commit，然后提升至 `0.2.0-alpha.4`。

#### 阶段 E：编排服务

- 实现 `full_scan()` 薄封装和 `ensure_snapshot()` 的完整状态机；
- 实现 TTL、force、partial policy、扫描租约、并发去重、旧快照保留及 crash recovery；
- 确保扫描期间不持有 SQLite 写事务，发布事务短且原子；
- 专属门禁：第 8 节每条状态转移均有测试；首次、复用、变化、失败、并发、租约过期和进程崩溃集成测试全部通过；
- 提交建议：service model、state machine、lease/recovery、integration tests 分开 commit，然后提升至 `0.2.0-beta.1`。

#### 阶段 F：对外接口

- 从 facade 导出稳定 Rust API，实现 `snapshot ensure/show/list/diff/export` CLI；
- 固化 stdout JSON、stderr、退出码、排序、分页和路径参数合约；
- 更新英文和中文 README，覆盖权限、默认路径、临时目录覆盖、清理和示例；
- 保持旧 CLI、`collect_scan_report()` 和 schema 行为兼容；
- 专属门禁：Rust facade contract、CLI snapshot contract、旧 CLI regression、golden JSON 和 README 命令 smoke test 通过；
- 提交建议：facade、CLI、contract tests、docs 分开 commit，然后提升至 `0.2.0-rc.1`。

#### 阶段 G：维护和发布收口

- 实现 retention、pin/upload 状态保护、两快照 diff、health check、WAL checkpoint 和可观测数据；
- 验证清理顺序、当前快照保护、artifact 删除失败重试及数据库长期增长边界；
- 执行第 13、14 节完整测试与验收清单，修复全部回归；
- 根据实际是否使用 GPL 派生代码执行全仓库许可证一致性检查；
- 专属门禁：22 条验收标准全部在执行日志标记为 PASS 并附证据；统一门禁全绿；工作树除执行日志最终更新外干净；
- 提交建议：retention、diff/health、release validation 分开 commit，然后提升至 `0.2.0`；创建本地 annotated tag `v0.2.0`，不得 push。

### 17.6 失败处理和继续规则

每条门禁失败时执行以下闭环，不询问人工：

1. 保存失败命令、退出码和关键错误到执行日志；
2. 定位根因，优先增加或收紧能够复现问题的最小测试；
3. 修复后重新运行失败测试，再运行当前阶段专属门禁和四条统一门禁；
4. 对同一根因最多进行三轮不同修复尝试。格式、网络瞬断或依赖下载等瞬时错误可额外重试三次；
5. 仍失败时不得提交版本 checkpoint、不得进入下一阶段，也不得伪造 PASS。保留可诊断工作树，在执行日志标记 `BLOCKED`，写明已尝试方案和下一步；
6. 某个可选性能优化失败但正确性门禁通过时，可以回退该优化并继续；回退只能针对本阶段由 GPT 创建的改动，必须保留测试并记录原因；
7. 不得因真实硬件缺失而阻塞 fixture 可验证的功能。真实硬件验证标记为环境限制，以 deterministic fixture 和当前机器可获得的观测完成自主判定。

### 17.7 最终交付条件

只有同时满足以下条件，整个计划才算完成：

- A-G 每阶段都有实现/测试 commit 和独立版本 checkpoint commit；
- workspace 版本为 `0.2.0`，`Cargo.lock` 一致，本地 tag 为 `v0.2.0`；
- 四条统一门禁和第 14 节 22 条验收标准全部通过；
- `docs/hardware-snapshot-execution-log.md` 能追溯每阶段输入、决策、测试、性能、许可证和 commit；
- `git status --short` 干净，不包含数据库、WAL、SHM、临时 artifact 或未提交源码；
- 未 push、未发布、未修改仓库外系统状态；
- 最终报告列出阶段版本、commit、测试结果、性能变化、许可证状态及仍存在的非阻塞风险。
