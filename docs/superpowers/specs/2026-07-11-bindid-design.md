# qurbrix-hw BindId 设计方案

日期：2026-07-11

## 1. 背景

当前 `qurbrix-hw scan` 是全量硬件扫描，目标是生成完整硬件资产报告。后续如果把它作为高频业务绑定判断的输入，会过重，也容易把“完整机器指纹”和“业务绑定 ID”混在一起。

原始 `GAutoTestTool` 中的 `bindid` 不是完整机器指纹。它的目的更窄：防止关键零部件变化后继续沿用旧测试/任务记录。原始实现实际没有把 CPU 和显示器纳入 bindid，即 CPU 或显示器变化不会触发 bindid 变化。

因此本方案继续使用 `bindid` 命名，不使用 `fingerprint` 命名，避免误导为完整机器指纹，也为未来真正的 `fingerprint` 能力保留语义空间。

## 2. 目标

1. 新增轻量业务绑定命令 `qurbrix-hw bindid`。
2. `bindid` 只基于关键零部件组合，不复用全量 `ScanReport`。
3. `bindid` 输出专用 `BindIdReport` JSON。
4. 采集类命令启动前统一要求 root 权限。
5. 保持现有 `scan`、`summary`、`table`、`schema`、`list-kinds` 语义不变。

## 3. 非目标

1. 不做完整机器指纹。
2. 不把 CPU 纳入 bindid。
3. 不把显示器纳入 bindid。
4. 不改变 `scan --format json` 的输出契约。
5. 不做非 root 下的降级采集。
6. 不实现缓存落盘；缓存可在后续方案中基于 bindid 使用。

## 4. CLI 设计

新增命令：

```bash
qurbrix-hw bindid
qurbrix-hw bindid --pretty
```

行为：

- 默认输出 compact JSON。
- `--pretty` 只改变 JSON 格式化，不改变字段内容。
- 不提供 `--kind` 或 `--exclude-kind`，因为 bindid 的参与范围固定。
- 不把 bindid 做成 `scan --profile bindid`，避免混淆完整扫描和业务绑定 ID。

现有命令保持原语义：

```bash
qurbrix-hw scan --format json
qurbrix-hw scan --format typed-json
qurbrix-hw summary
qurbrix-hw table
qurbrix-hw schema
qurbrix-hw list-kinds
```

## 5. 权限门禁

采集类命令统一要求 `euid == 0`：

```text
scan
summary
table
bindid
```

未来如果加入会触发真实硬件采集的缓存刷新命令，也应纳入权限门禁。

不触发真实硬件采集的命令不需要 root：

```text
schema
list-kinds
未来的 cache show / cache path
```

规则：

1. CLI 参数解析完成后、进入采集逻辑前检查 `euid == 0`。
2. 非 root 直接失败，不进入 probe、source runner、DMI 或 bindid 计算。
3. stdout 不输出半成品 JSON。
4. stderr 输出明确错误。
5. 退出码使用现有 `4 Permission failure prevents core scan`。
6. 第一版不识别 capability、group 或单文件访问权限。

## 6. BindIdReport

`bindid` 输出专用 JSON，不复用 `ScanReport`：

```json
{
  "schema_version": "qurbrix.hw.bindid.v1",
  "algorithm": "qurbrix-hw-bindid-sha1-hex16-v1",
  "status": "complete",
  "value": "0123456789abcdef",
  "required_kinds": ["system", "motherboard", "memory", "storage", "network"],
  "optional_kinds": ["gpu"],
  "covered_kinds": ["system", "motherboard", "memory", "storage", "network"],
  "missing_required_kinds": [],
  "missing_optional_kinds": ["gpu"],
  "component_keys": [
    "system:manufacturer=GEIT|product=UT6619-FC2",
    "motherboard:product=GG-D3000-AIO-WB1|serial=xxx",
    "memory:product=DDR4-xxx|serial=xxx",
    "storage:model=xxx|serial=xxx",
    "network:mac=xx:xx:xx:xx:xx:xx"
  ],
  "warnings": []
}
```

字段说明：

- `schema_version`：bindid 输出契约版本。
- `algorithm`：bindid 算法版本。
- `status`：`complete` 或 `failed`。
- `value`：最终 bindid。失败时固定输出 `null`。
- `required_kinds`：必须至少生成一条有效 key 的种类。
- `optional_kinds`：缺失也不导致失败的种类。
- `covered_kinds`：本次实际参与 hash 的种类。
- `missing_required_kinds`：缺失后导致失败的必需种类。
- `missing_optional_kinds`：缺失但不导致失败的可选种类。
- `component_keys`：参与 hash 的规范化 key，用于审计和排错。
- `warnings`：非权限类警告。

## 7. 参与范围

必需种类：

```text
system
motherboard
memory
storage
network
```

可选种类：

```text
gpu
```

不参与 bindid：

```text
cpu
monitor
audio
bluetooth
input
camera
battery
printer
cdrom
usb
pci fallback
```

理由：

- CPU 和显示器在最原始 bindid 中没有参与，继续保持不触发 bindid 变化。
- 内存、硬盘、网卡通常一定存在，数量变化或关键值变化应触发 bindid 变化。
- GPU 不设为必需，因为很多机器没有独立 GPU，缺失 GPU 不应导致 bindid 命令失败。

## 8. Key 生成规则

每个组件生成一条规范化 component key。

### 8.1 system

字段：

```text
manufacturer
product
```

格式：

```text
system:manufacturer=<system manufacturer>|product=<system product name>
```

### 8.2 motherboard

字段：

```text
serial
product
```

格式：

```text
motherboard:product=<board product name>|serial=<board serial>
```

### 8.3 memory

每条内存生成一条 key。

字段：

```text
serial
product
```

`product` 优先使用内存条 part number、product 或当前模型中的等价型号字段。

格式：

```text
memory:product=<memory product>|serial=<memory serial>
```

### 8.4 storage

每块物理硬盘生成一条 key。不包含分区。

字段：

```text
serial
model
```

格式：

```text
storage:model=<disk model>|serial=<disk serial>
```

### 8.5 network

每张物理网卡生成一条 key。

字段：

```text
mac
```

格式：

```text
network:mac=<mac>
```

不参与字段：

```text
interface name
network type
driver
speed
ip
link state
duplex
```

规则：

- loopback 不参与。
- 空 MAC 不参与。
- `00:00:00:00:00:00` 不参与。
- MAC 统一小写。
- 多张网卡分别生成 key，排序后参与 hash。

### 8.6 gpu

每个 GPU 生成一条 key。GPU 是可选种类。

字段：

```text
name
model
```

格式：

```text
gpu:model=<gpu model>|name=<gpu name>
```

不参与字段：

```text
显存
当前分辨率
最小/最大分辨率
显示器信息
驱动状态
```

## 9. 规范化规则

所有参与 key 的字符串统一处理：

1. trim 首尾空白。
2. 连续空白折叠为单个空格。
3. MAC 和十六进制 ID 转小写。
4. 空字符串不参与。
5. 以下占位值不参与：
   - `None`
   - `N/A`
   - `Not Specified`
   - `No Asset Tag`
   - `To Be Filled By O.E.M.`
   - `System Serial Number`
   - `Default string`
   - `Unknown`
6. 同一个 component key 内字段按字段名排序。
7. 多个 component key 全局排序。

## 10. Hash 算法

算法名：

```text
qurbrix-hw-bindid-sha1-hex16-v1
```

步骤：

1. 收集所有有效 component key。
2. 校验必需种类是否都有至少一条 key。
3. 对 component key 全局排序。
4. 使用 `||` 拼接。
5. 对拼接字符串做 SHA1。
6. 取前 16 位 hex 作为 `value`。

## 11. 失败语义

以下情况直接失败：

- 非 root 执行采集类命令。
- `system` 无法生成有效 key。
- `motherboard` 无法生成有效 key。
- `memory` 没有任何有效 key。
- `storage` 没有任何有效 key。
- `network` 没有任何有效 key。
- 序列化输出失败。

以下情况不失败：

- GPU 缺失。
- CPU 信息变化。
- 显示器缺失或变化。
- 网卡接口名变化。
- 网络 IP、速率、link 状态变化。

## 12. 模块边界

建议新增独立 bindid 模块或 crate：

```text
hw-bindid
```

职责：

- 定义 `BindIdReport`。
- 定义 `BindIdStatus`。
- 定义 component key 类型和规范化逻辑。
- 负责 key 采集、覆盖率判断、排序和 hash。

其他模块职责：

- `hw-cli`：新增 `bindid` 子命令、权限门禁、JSON 输出。
- `hw-source`：复用命令执行和文件读取能力。
- `hw-parser` / `hw-probe`：可复用现有解析逻辑，但不要让 `bindid` 依赖完整 `ScanReport`。
- `hw-output`：现有 scan 输出不变；是否承载 bindid JSON helper 可在实现计划中决定。

## 13. 测试策略

需要覆盖：

1. CLI：
   - `qurbrix-hw bindid` 输出 compact JSON。
   - `qurbrix-hw bindid --pretty` 输出 pretty JSON。
   - 非 root 执行 `bindid` 返回退出码 4，stdout 为空。
   - `schema`、`list-kinds` 不触发权限门禁。

2. key 生成：
   - system key 字段正确。
   - motherboard key 字段正确。
   - 多条 memory key 全部参与。
   - 多块 storage key 全部参与。
   - 多张 network key 全部参与，且只使用 MAC。
   - GPU 缺失时不失败。

3. hash 稳定性：
   - component key 输入顺序变化，bindid 不变。
   - 同类多设备顺序变化，bindid 不变。
   - CPU 字段变化，bindid 不变。
   - 显示器字段变化，bindid 不变。
   - network interface name / type / driver / speed / IP 变化，bindid 不变。
   - memory、storage、network 的 key 变化，bindid 改变。

4. 失败规则：
   - system 缺失失败。
   - motherboard 缺失失败。
   - memory 无有效 key 失败。
   - storage 无有效 key 失败。
   - network 无有效 MAC 失败。

## 14. 推荐结论

采用独立 `qurbrix-hw bindid` 命令。

该命令不是完整机器指纹，而是有限范围的业务绑定 ID。它继承最原始 bindid 的核心意图：关键零部件变化时让旧任务/测试记录失效；CPU 和显示器变化不触发误判。

第一版最小闭环：

```text
命令：qurbrix-hw bindid [--pretty]
权限：采集类命令要求 euid=0
必需：system、motherboard、memory、storage、network
可选：gpu
算法：component key 排序后 "||" 拼接，SHA1 前 16 位
输出：BindIdReport JSON
```
