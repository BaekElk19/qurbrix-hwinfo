# qurbrix-hwinfo Hardware Compatibility Gap Report

生成日期：2026-07-05

更新说明（2026-07-06）：本报告保留为初始差距基线。当前实现已经吸收部分当时缺口，最新状态以
`docs/hardware-compatibility-reference-audit.md` 为准。已完成的 CPU 关键改进包括：
`lscpu` + `lshw -class processor` + `dmidecode -t 4` 多源合并、`/proc/cpuinfo`
`Hardware`/`Processor` fallback、`/proc/hardware` Kirin fallback、DMI 当前频率/count 修正、CPU vendor/arch 归一化；USB 已在 `lsusb` 不可用时读取 `/sys/bus/usb/devices/*` 基础 device 字段；Bluetooth 已在 `hciconfig -a` 不可用时读取 `/sys/class/bluetooth/hci*` 基础 controller 字段。

## 1. Executive Summary

1. `qurbrix-hwinfo` 的硬件类别覆盖已经比较宽：CPU、PCI、USB、内存、BIOS/主板、GPU、显示器、存储、网络、音频、蓝牙、输入、摄像头、电池、打印机、光驱都在统一 probe orchestration 中注册；证据：`qurbrix-hwinfo/crates/hw-collect/src/collector.rs:21-39`。
2. 与 Deepin/Kylin 相比，最大差距不是“完全没有硬件类别”，而是“特殊硬件兼容规则、字段 fallback、厂商/型号归一化、非 x86 字段差异处理”不足。
3. CPU 曾是最明显短板：初始审计时只执行 `lscpu`，只解析 `Architecture`、`CPU(s)`、`Model name`、`Vendor ID`、`Core(s) per socket`、`Socket(s)` 六类字段；当前已改为多源合并并增加 `/proc/cpuinfo` fallback，详见最新审计文档。
4. Deepin 会融合 `lscpu`、`lshw_cpu`、`dmidecode -t 4`，并用 DMI 修正 core/thread、CPU name/vendor 空缺；证据：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceGenerator.cpp:173-259`。
5. Kylin 明确做了国产 CPU vendor 归一化和 `/proc/cpuinfo` 的 `Hardware` fallback；证据：`ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:704-745`，`ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py:220-228`。
6. 国产 CPU、ARM64、LoongArch 仍有 P1 风险：`qurbrix-hwinfo` 已有 `/proc/cpuinfo` parser、`Hardware`/`Processor` fallback、`/proc/hardware` Kirin fallback 和 locale 强制，但更广泛真机 fixture 仍待补齐。
7. Deepin 已有架构 alias 和架构分流，覆盖 `aarch64`、`sw_64`、`loongarch`、`loongarch64`；证据：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/commonfunction.cpp:25-33`，`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceFactory.cpp:29-69`。
8. `qurbrix-hwinfo` 的错误、超时、命令缺失、权限不足处理比参考项目更结构化，值得保留；证据：`qurbrix-hwinfo/crates/hw-source/src/runner.rs:22-59`。
9. 优先补齐顺序建议：P0 CPU 多源 fallback；P1 国产 CPU/vendor/arch 规则和 fixtures；P1 显示 EDID/sysfs、音频 SoC/sysfs；P2 USB/network 过滤和分类增强。

## 2. Scope and Method

扫描目录：

| 项目 | 路径 |
| --- | --- |
| qurbrix-hwinfo | `qurbrix-hwinfo` |
| Deepin Device Manager | `ReferenceProject/deepin-devicemanager-6.0.67` |
| Kylin OS Manager | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2` |

已按要求先检查仓库说明和工作区状态。审计过程中未修改源码、未格式化、未回滚、未提交 commit。外层目录不是 git 仓库；`qurbrix-hwinfo` 是实际 git 仓库。保存本报告前，`git -C qurbrix-hwinfo status --short` 无输出。

系统扫描重点包括 `dmidecode`、`lscpu`、`lspci`、`lsusb`、`lshw`、`hwinfo`、`udev`、`/sys`、`/proc`、`DMI`、`SMBIOS`、`PCI`、`USB`、`driver`、`vendor`、`product`、CPU/架构/国产厂商、GPU/显示、设备分类、`fallback`、`normalize`、`fix`、`workaround` 等关键词。

本报告只提炼参考项目的行为、兼容策略、字段来源和判断逻辑，不复制 Deepin/Kylin 实现代码。

## 3. Current qurbrix-hwinfo Capability Map

| 类别 | 当前来源/实现 | 当前状态 | 证据 |
| --- | --- | --- | --- |
| Orchestration | 顺序执行 probe，汇总 devices/warnings/consumed | 结构清晰，类别覆盖广 | `qurbrix-hwinfo/crates/hw-collect/src/collector.rs:21-63` |
| CPU | `lscpu`、`lshw -class processor`、`dmidecode -t 4`、`/proc/cpuinfo`、`/proc/hardware` | 多源扫描；已补 `Hardware`/`Processor` fallback、Kirin fallback、DMI fallback、locale 强制和主要 vendor alias；仍缺更广泛真机 fixture | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`；`qurbrix-hwinfo/crates/hw-parser/src/cpu.rs` |
| CPU model | `CpuInfo` 有 name/vendor/arch/core/thread/socket/frequency/flags | model 有字段，但 parser 没填频率和 flags | `qurbrix-hwinfo/crates/hw-model/src/properties.rs:54-65` |
| PCI | `lspci -nn -k` | 能解析 class/vendor/device/driver/modules | `qurbrix-hwinfo/crates/hw-probe/src/pci.rs:22-83` |
| USB | `lsusb` + `/sys/bus/usb/devices/*` fallback | 能解析基础 USB 字段；无 `lsusb` 时可从 sysfs 读取 bus/dev、VID/PID、device class/subclass/protocol、manufacturer/product/serial/speed；仍无 `lsusb -v`、maxpower、详细 interface descriptor、跨类别 consumed dedup | `qurbrix-hwinfo/crates/hw-probe/src/usb.rs` |
| Memory | `dmidecode -t memory` | 可识别 DIMM size/vendor/type/speed/slot/serial/part | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:168-229` |
| BIOS / Motherboard | `dmidecode -t 0,1,2,3` | 可识别 BIOS vendor/version/date 和 board manufacturer/product/serial | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:232-295` |
| Storage | `lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN` + `/sys/block/*` fallback | 正常路径取 disk；fallback 路径补 node/model/serial/size/rotational；缺少 SMART、temperature、controller/driver | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| GPU | `lspci -nn -k`，GPU parser | 能识别 PCI GPU 和 driver；缺少国产 GPU vendor alias/glxinfo/drm sysfs | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:309-354` |
| Monitor | `xrandr --query` | 只取 connector/resolution；缺少 EDID/vendor/product/week/year/size | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:368-399` |
| Network | `ip -j link` + `/sys/class/net/*` fallback | 正常路径取 interface/MAC/operstate，fallback 路径补 speed/duplex；过滤 loopback/常见虚拟网卡；缺少 lshw/lspci driver、无线/以太分类 | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| Error handling | command/read/glob 抽象，区分 Missing/PermissionDenied/Timeout/Failed | 优于多数脚本式参考实现，建议保留 | `qurbrix-hwinfo/crates/hw-source/src/runner.rs:22-59` |
| Dedup | 仅按 `Device.id` 合并 sources/warnings/capabilities | 有基础去重；缺少基于 bus/class/vendor/serial 的语义合并 | `qurbrix-hwinfo/crates/hw-collect/src/merge.rs:4-20` |
| Tests/fixtures | 有 PCI/USB/蓝牙/打印机/电源/音频/输入/摄像头等 fixtures | 缺少 CPU 架构/国产平台 fixtures | `qurbrix-hwinfo/crates/hw-testdata/fixtures/pci/lspci-nn-k.txt`；`qurbrix-hwinfo/crates/hw-testdata/fixtures/usb/lsusb.txt` |

判断：`qurbrix-hwinfo` 不是单纯空壳，已经是“通用硬件扫描 + 结构化 model + 统一 source runner”。但特殊硬件兼容还处于早期，主要采集原始字段，语义归一化、fallback、国产 CPU/GPU/SoC 规则不足。

## 4. Deepin Reference Capability Map

| 类别 | Reference 行为 | 参考价值 | 证据 |
| --- | --- | --- | --- |
| 数据源池 | 收集 `lshw`、多种 `dmidecode`、`hwinfo`、`upower`、`lscpu`、`lsblk`、`xrandr`、`dmesg`、`hciconfig`、`/proc/boardinfo`、`/proc/asound`、`/proc/gpuinfo_0` 等 | 提供多源 fallback 思路 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/GetInfoPool.cpp:85-122` |
| 架构分流 | 根据 `x86_64`、`mips64`、`aarch64` 和 ARM board vendor 选择 generator | 说明非 x86 需要字段和来源分流 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceFactory.cpp:29-69` |
| 架构 alias | `aarch64 -> arm64`、`x86_64 -> amd64`、`sw_64`、`loongarch`、`loongarch64` | 可转化为 qurbrix 的 arch normalization | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/commonfunction.cpp:25-33` |
| CPU 多源合并 | `lscpu` + `lshw_cpu` + `dmidecode4`，DMI 修正 core/thread/socket/name/vendor | qurbrix CPU P0/P1 参考 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceGenerator.cpp:173-259` |
| CPU 字段解析 | 解析 model/vendor/thread/bogomips/architecture/family/frequency/model/stepping/flags/virtualization | qurbrix 已有 model 字段但 parser 未填全 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:224-289` |
| Loongson CPU | 对 Loongson 避免被 lshw/dmidecode 覆盖型号 | 特殊 CPU name 保护规则 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:306-323`；`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:365-390` |
| Phytium/ARM 频率 | 注释说明飞腾无法通过 lscpu 获取当前频率，使用 dmidecode Current Speed | ARM/国产 CPU 频率 fallback | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:384-390` |
| Monitor | 使用 `xrandr`/verbose/EDID 处理显示器信息 | qurbrix monitor 目前只取 query，缺 EDID | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/GetInfoPool.cpp:109-110` |

Not Applicable：Deepin 的 GUI 文案加载进度、发行版专属 UI 结构不适合直接进入 `qurbrix-hwinfo`。qurbrix 应保留 CLI/library 输出模型，不引入 GUI service 依赖。

## 5. Kylin Reference Capability Map

| 类别 | Reference 行为 | 参考价值 | 证据 |
| --- | --- | --- | --- |
| CPU vendor 归一化 | 从 `lscpu` Model name 推断 Phytium/Huawei/Hygon/Zhaoxin/Loongson/Intel/D2000 | 国产 CPU alias 规则 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:704-745` |
| `/proc/cpuinfo` fallback | `model name` 不存在时使用 `Hardware` | ARM SoC/非 x86 必需 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py:220-228` |
| Kirin SoC | 读取 `/proc/hardware` 识别 HUAWEI Kirin 990/9006C | HiSilicon/Kirin SoC 兼容参考 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:62-71` |
| 厂商表 | 覆盖 CPU、GPU、显示器、整机、虚拟机、网卡、硬盘、声卡、摄像头、输入、电池、BIOS 等 vendor alias | qurbrix 可建立小型、可测、许可证无关的归一化表 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:409-565` |
| 国产 GPU alias | 包含 `JINGJIA`/`JJM`、`Wuhan Digital Engineering` 等 | GPU vendor 归一化参考 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:415-425` |
| Monitor EDID | 从 xrandr verbose 提取 EDID，用 `edid-decode` 解析 manufacturer/product/week/year/size/gamma/maxmode | qurbrix monitor P1/P2 增强 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:1339-1411` |
| Audio/SoC | 对 Loongson/HiSilicon 声卡 vendor 有 alias | 国产平台外设展示增强 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:479-483` |
| VirtualBox | 识别 `INNOTEK`/`VBOX`/`VIRTUALBOX` | 虚拟机设备识别和标注参考 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:452-455` |

Not Applicable：Kylin 代码中有大量 `/tmp/youker-assistant-*` 临时文件和发行版服务流程；这不适合 qurbrix 的 library/CLI 架构。可借鉴数据源和规则，不建议引入临时文件协议。

## 6. CPU Special Handling Gap

| CPU/架构 | Reference 处理 | qurbrix-hwinfo 当前处理 | 缺口 | 建议 | 证据 |
| --- | --- | --- | --- | --- | --- |
| 通用 x86_64 | Deepin 解析 model/vendor/family/model/stepping/frequency/flags/virtualization；Kylin 用 lscpu 计算 core | 已扩展 lscpu/procfs/DMI 主要字段 | family/model/stepping/bogomips/virtualization 尚未暴露到 `CpuInfo` | 后续按模型字段需求扩展 `CpuInfo` | Deepin: `.../DeviceCpu.cpp:224-289`；qurbrix: `qurbrix-hwinfo/crates/hw-parser/src/cpu.rs` |
| Intel | Kylin 将 model name 中 Intel 归一化为 Intel | qurbrix 直接保存 `Vendor ID` | 展示 alias 和 vendor fallback 不足 | 建立 vendor alias：`GenuineIntel`/`Intel` -> `Intel` | Kylin: `.../cpuinfo.py:742-744`；qurbrix: `qurbrix-hwinfo/crates/hw-parser/src/cpu.rs:23-24` |
| AMD | Kylin vendor 表包含 AMD | qurbrix 直接保存 `AuthenticAMD` 或 lscpu 原值 | 缺少 alias | `AuthenticAMD`/`AMD` -> `AMD` | Kylin: `.../cpuinfo.py:411-414`；qurbrix: `qurbrix-hwinfo/crates/hw-parser/src/cpu.rs:23-24` |
| Hygon / 海光 | Kylin 从 model name 识别 `hygon` | qurbrix 无特殊处理 | 国产 x86 vendor 可能不统一 | `HygonGenuine`/`Hygon`/`海光` alias | Kylin: `.../cpuinfo.py:734-735` |
| Zhaoxin / 兆芯 | Kylin 从 model name 识别 `zhaoxin` | qurbrix 无特殊处理 | 国产 x86 vendor 可能不统一 | `CentaurHauls`/`Shanghai`/`Zhaoxin`/`兆芯` alias | Kylin: `.../cpuinfo.py:736-737` |
| Loongson / 龙芯 | Kylin 识别 Loongson 并补 CPU 频率；Deepin 防止 Loongson name 被 lshw/dmidecode 覆盖 | 已有 Loongson name 保护、vendor alias、DMI/procfs 频率 fallback | LoongArch 真机样本仍不足 | 增加更多 `/proc/cpuinfo` fixture 和真机验证 | Kylin: `.../cpuinfo.py:738-741`；Deepin: `.../DeviceCpu.cpp:306-323` |
| Phytium / 飞腾 | Kylin 识别 Phytium/D2000；Deepin 用 DMI Current Speed 补飞腾频率 | qurbrix 无特殊处理 | ARM64 频率和 vendor 风险 | `phytium`/`D2000`/`飞腾` alias，DMI Current Speed fallback | Kylin: `.../cpuinfo.py:730-731`，`.../cpuinfo.py:744-745`；Deepin: `.../DeviceCpu.cpp:384-390` |
| Kunpeng / 鲲鹏 | Kylin 将 `huawei` model name 归一化为 Huawei；另有 Kirin `/proc/hardware` | 已有 `/proc/cpuinfo` Hardware、Kirin `/proc/hardware` 和 HiSilicon/Kunpeng alias | ARM implementer/part 细分仍不足 | 增加更多真机 fixture | Kylin: `.../cpuinfo.py:732-733`；`.../cpuinfo.py:62-71` |
| HiSilicon / 海思 | Kylin vendor 表包含 HISILICON，Kirin 走 `/proc/hardware` | 已有 HiSilicon/Kirin vendor 推断和 `/proc/hardware` Kirin fallback | SoC 声卡/GPU 等跨类别 vendor 识别仍可增强 | 后续在 audio/GPU 类别补 alias/enrichment | Kylin: `.../cpuinfo.py:479-483`；`.../cpuinfo.py:62-71` |
| Sunway / 申威 | Deepin arch map 包含 `sw_64` | qurbrix 没有 arch alias | 申威架构显示和字段选择风险 | arch normalization 加 `sw_64`，CPU parser 接受非 x86 字段缺失 | Deepin: `.../commonfunction.cpp:25-33` |
| ARM64 SoC | Deepin aarch64 走 board vendor generator；Kylin `/proc/cpuinfo Hardware` fallback | 已读取 `/proc/cpuinfo` 的 `Hardware`/`Processor`，并支持 `/proc/hardware` Kirin fallback | 真机样本仍不足 | 补更多 ARM64/SoC fixture | Deepin: `.../DeviceFactory.cpp:40-65`；Kylin: `.../sysinfo/__init__.py:220-228` |
| LoongArch | Deepin arch alias 覆盖 `loongarch`/`loongarch64` | qurbrix 不归一化 arch | 架构展示和后续规则风险 | arch normalization + Loongson fixtures | Deepin: `.../commonfunction.cpp:30-32` |

最小实现建议：

1. 在 `hw-parser` 增加原创 `parse_proc_cpuinfo`，支持多 record、key 大小写差异和 `model name`/`Hardware`/`Processor`/`vendor_id`/`CPU implementer`/`cpu MHz`/`flags`。
2. 在 `CpuProbe` 中保留 `lscpu` 为 primary，失败或字段为空时读取 `/proc/cpuinfo`，可选读取 `dmidecode -t processor` 补 socket/core/thread/current speed/max speed。
3. 新增 `normalize_cpu_vendor` 和 `normalize_architecture`，仅保存小而可测的 alias，不复制参考项目表。
4. 新增 fixtures：Intel、AMD、Hygon、Zhaoxin、Loongson LoongArch、Phytium ARM64、Kunpeng ARM64、HiSilicon/Kirin、model name 空但 Hardware 存在、vendor_id 不存在。

## 7. Domestic Hardware and Non-x86 Gap

| 平台/厂商 | Reference 项目中的特殊逻辑 | qurbrix-hwinfo 风险 | 建议动作 | 证据 |
| --- | --- | --- | --- | --- |
| LoongArch / 龙芯 | Deepin arch alias 覆盖 LoongArch，CPU name 避免被 lshw/DMI 覆盖；Kylin vendor 表包含 Loongson | CPU 名称、架构、vendor、频率可能缺失或被错误覆盖 | 增加 LoongArch fixtures，优先使用可信 model name，补 `/proc/cpuinfo` | Deepin: `.../commonfunction.cpp:25-33`，`.../DeviceCpu.cpp:306-323`；Kylin: `.../cpuinfo.py:554-558` |
| ARM64 / 飞腾 | Deepin aarch64 分 generator，飞腾频率从 DMI Current Speed 补；Kylin 识别 Phytium/D2000 | `lscpu` 不足时频率/vendor/name 缺失 | `/proc/cpuinfo` + DMI fallback + Phytium alias | Deepin: `.../DeviceFactory.cpp:40-65`，`.../DeviceCpu.cpp:384-390`；Kylin: `.../cpuinfo.py:730-731` |
| 鲲鹏 / 华为 / 海思 | Kylin 从 `huawei` 推断 Huawei，读取 `/proc/hardware` 识别 Kirin | ARM SoC 可能只有 Hardware 字段 | Huawei/HiSilicon/Kunpeng/Kirin alias，`/proc/hardware` 可作为 Linux optional source | Kylin: `.../cpuinfo.py:62-71`，`.../cpuinfo.py:732-733` |
| 兆芯 / 海光 | Kylin 从 model name 识别 Zhaoxin/Hygon | vendor_id 可能非标准，展示不统一 | CPU vendor alias 表，测试 vendor_id 缺失/非标准 | Kylin: `.../cpuinfo.py:734-737` |
| 申威 | Deepin arch alias 包含 `sw_64` | 架构显示和字段选择风险 | 加 `sw_64` arch normalization；若无样本先标 unknown with evidence | Deepin: `.../commonfunction.cpp:30-31` |
| 国产 GPU | Kylin vendor 表包含 Jingjia/JJM/Wuhan Digital Engineering | qurbrix 只依赖 lspci 原始 vendor/product，缺 alias 和国产厂商识别 | GPU vendor alias 小表，保留 PCI IDs | Kylin: `.../cpuinfo.py:415-425`；qurbrix: `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:309-354` |
| 国产声卡/SoC 音频 | Kylin 表包含 Loongson/HiSilicon 声卡 vendor | qurbrix 若只看通用 PCI/ALSA，SoC 音频可能弱 | audio parser 增加 `/proc/asound`/sysfs SoC fallback 和 vendor alias | Kylin: `.../cpuinfo.py:479-483` |

## 8. Hardware Data Source Gap

| 数据源 | Deepin 使用情况 | Kylin 使用情况 | qurbrix-hwinfo 使用情况 | 差距 | 建议 |
| --- | --- | --- | --- | --- | --- |
| `/proc/cpuinfo` | 间接经 lscpu/lshw，另有 `/proc/boardinfo` | 直接读取，支持 `Hardware` fallback | CPU probe 已作为 optional procfs fallback 使用 | 已吸收 | 继续补更多国产 CPU fixtures |
| `/proc/meminfo` | 非核心证据 | 有注释/部分系统信息逻辑 | memory probe 已在 dmidecode 失败时读取 MemTotal 总量 | 已吸收部分 | 后续如需 DIMM 级信息仍需 DMI/lshw/sysfs |
| `/sys/class/dmi/id` | 主要走 dmidecode | 可作为系统信息来源 | BIOS/board probe 已作为 dmidecode fallback 使用 | 已吸收 | 后续可扩 chassis/system/language/memory-array |
| `/sys/class/drm` | 参考项目使用 xrandr/EDID 类能力 | 通过 xrandr verbose/EDID | 未使用 | P1/P2 | headless/Wayland 下补 sysfs drm/edid |
| `/sys/class/net` | Deepin 有网络 sysfs/MAC 过滤逻辑 | 结合 lshw/lspci/driver | Network probe 已在 `ip` 失败时读取 MAC/operstate/speed/duplex | 已吸收部分 | 后续补 driver、type、wireless |
| `/sys/class/power_supply` | Deepin 使用 upower/dmesg 类电源源 | Kylin 有电源厂商 alias | Battery probe 已在 UPower 失败时读取 BAT* sysfs 字段 | 已吸收部分 | 后续可补温度和厂商归一化 |
| `/sys/block` | Deepin 用 lsblk/sg | Kylin 磁盘逻辑复杂 | Storage probe 已在 `lsblk` 失败时读取 size/model/serial/rotational | 已吸收部分 | 后续补 vendor/wwn/controller/SMART |
| `/sys/bus/pci` | 参考项目重视 PCI/driver | lspci/lshw/driver | 当前 `lspci -nn -k` | P2 | 无 lspci 时读 sysfs modalias/vendor/device/class/driver |
| `/sys/bus/usb` | Deepin USB 过滤/去重 | Kylin `lsusb -v` | USB probe 已在 `lsusb` 失败时读取基础 sysfs device 字段 | 已吸收部分 | 后续补 `lsusb -v`、maxpower、interface descriptor、跨类别 dedup |
| `lscpu` | CPU 主来源之一 | CPU 主来源之一 | CPU primary source，另有 lshw/DMI/procfs fallback | 已吸收主要 fallback，并强制英文 locale | 继续补真机样本 |
| `lspci` | PCI/GPU/driver 来源 | 网络/GPU/声卡等来源 | PCI/GPU 来源 | P2 | 继续使用，补分类和 alias |
| `lsusb` | USB 来源并带过滤/去重 | 使用更详细输出 | USB primary 基础来源，缺失时有 sysfs fallback | P2 | 加 `lsusb -v` optional source |
| `dmidecode` | BIOS/board/memory/CPU 修正 | 系统硬件常用来源 | CPU、BIOS/board/memory 来源；BIOS/board 权限不足走 sysfs；memory 权限不足可走 `/proc/meminfo` 总量 fallback | 已吸收部分 | memory 仍可补 sysfs/lshw DIMM 级 fallback |
| `lshw` | CPU/audio/network 等多类 fallback | 网络/硬件详情 | 未使用 | P2 | 可选，不作为硬依赖 |
| `hwinfo` | monitor/general 来源 | 依赖或脚本中存在 | 未使用 | P3 | Not Applicable by default，避免重依赖 |
| `udevadm` | 搜索范围内需关注 | 参考项目可通过 udev 类源 | 未见核心使用 | P2 | 可用于 USB/PCI/input 属性补充 |
| `xrandr` | query/verbose | verbose + EDID | 只用 `xrandr --query` | P1/P2 | 加 verbose/EDID 或 sysfs drm |
| `glxinfo` | GPU 展示增强类 | GPU 可能使用 | 未使用 | P3 | Not Applicable for core scan；可选显示 renderer |

## 9. Device Category Gap Matrix

| 类别 | Reference 行为 | qurbrix-hwinfo 当前状态 | 差距 | 严重程度 | 是否建议实现 | 证据 |
| --- | --- | --- | --- | --- | --- | --- |
| CPU | 多源合并、DMI fallback、国产 vendor/arch 处理 | 已实现多源合并、`/proc/cpuinfo` fallback、`/proc/hardware` Kirin fallback 和 locale 强制 | 真机样本覆盖仍不足 | P1/P2 | 是 | Deepin `.../DeviceGenerator.cpp:173-259`；qurbrix `crates/hw-probe/src/existing.rs` |
| 主板/BIOS/DMI | 多个 dmidecode type | dmidecode `0,1,2,3` + `/sys/class/dmi/id` fallback | chassis/system/language/memory-array 未建模 | P2 | 部分 | qurbrix `crates/hw-probe/src/existing.rs` |
| 内存 | dmidecode type 17 等 | dmidecode memory，dmidecode 失败时用 `/proc/meminfo` 产出总量 fallback | 缺 sysfs/lshw DIMM 级 fallback | P2 | 是 | qurbrix `crates/hw-probe/src/existing.rs` |
| 硬盘/分区/控制器 | lsblk/sg/lshw/厂商修正 | lsblk disk，失败时 fallback 到 `/sys/block/*` | 缺 SMART/temperature/controller/driver/vendor/wwn | P2 | 是 | qurbrix `crates/hw-probe/src/existing.rs` |
| 显卡/显示控制器 | lspci、nvidia、国产 GPU alias | lspci GPU/driver | 缺国产 GPU alias、glxinfo/drm | P1/P2 | 是 | Kylin `.../cpuinfo.py:415-425`；qurbrix `crates/hw-probe/src/existing.rs:309-354` |
| 显示器 | xrandr verbose/EDID/product/year/size | xrandr query connector/resolution | EDID 缺失 | P1/P2 | 是 | Kylin `.../cpuinfo.py:1339-1411`；qurbrix `crates/hw-probe/src/existing.rs:368-399` |
| 网卡/Wi-Fi/蓝牙 | sysfs/MAC/filter、lshw/lspci/driver | network 使用 `ip -j link`，失败时 fallback 到 `/sys/class/net/*`；bluetooth 使用 `hciconfig -a`，失败时 fallback 到 `/sys/class/bluetooth/hci*` 基础 controller/rfkill 字段 | 网卡分类/driver/wireless 标注不足；Bluetooth 仍缺 lshw/hwinfo/BlueZ DBus enrichment 和 controller address fallback | P1/P2 | 是 | qurbrix `crates/hw-probe/src/existing.rs`；`crates/hw-probe/src/bluetooth.rs` |
| 声卡 | lshw、`/proc/asound`、SoC vendor | qurbrix 有音频类别和 fixtures | SoC/国产声卡规则不足 | P1/P2 | 是 | Deepin `.../GetInfoPool.cpp:88,119`；Kylin `.../cpuinfo.py:479-483` |
| USB 设备 | 过滤 hub/重复/无效设备，详细描述符 | `lsusb` 基础字段；`lsusb` 缺失时读 sysfs 基础字段并过滤 hub/host controller/interface entries | 无 `lsusb -v`、maxpower、详细 interface descriptor、跨类别 consumed dedup | P2 | 是 | qurbrix `crates/hw-probe/src/usb.rs` |
| PCI 设备 | PCI 分类、驱动识别 | lspci class/vendor/device/driver | 分类消费只对部分类别；alias 不足 | P2 | 是 | qurbrix `crates/hw-probe/src/pci.rs:22-83` |
| 摄像头 | USB/vendor 表、video source | qurbrix 有摄像头 fixtures | 厂商 alias 和 sysfs 属性可能不足 | P2 | 是 | Kylin `.../cpuinfo.py:484-491` |
| 打印机/扫描仪 | printer source | qurbrix 有 printer fixtures | 扫描仪未明确 | P3 | 部分 | Deepin `.../GetInfoPool.cpp:89` |
| 电池/电源 | upower/dmesg/vendor alias | qurbrix 有 UPower 和 `/sys/class/power_supply/BAT*` fallback fixtures | 温度 fallback/厂商归一化待增强 | P2 | 是 | Deepin `.../GetInfoPool.cpp:105,111`；Kylin `.../cpuinfo.py:524-525` |
| 输入设备 | `/proc/bus/input/devices` | qurbrix 有 input fixtures | 分类和 vendor alias 可增强 | P2 | 是 | Deepin `.../GetInfoPool.cpp:118`；Kylin `.../cpuinfo.py:492-503` |
| 虚拟机设备 | Kylin VirtualBox alias | qurbrix 无明确虚拟机分类/标注 | 虚拟设备未过滤/未标注 | P2 | 是 | Kylin `.../cpuinfo.py:452-455` |
| 国产平台/非 x86 | Deepin arch map/generator，Kylin Hardware fallback/vendor alias | 无专门规则 | ARM64/LoongArch/SW64 风险 | P1 | 是 | Deepin `.../commonfunction.cpp:25-33`；Kylin `.../sysinfo/__init__.py:220-228` |

## 10. Test Fixture Gap

| 测试样本 | 覆盖风险 | 当前是否已有 | 建议 fixture 文件名 | 预期断言 |
| --- | --- | --- | --- | --- |
| Intel x86_64 `/proc/cpuinfo` | vendor/model/flags/frequency 基线 | 否 | `crates/hw-testdata/fixtures/cpu/proc-cpuinfo-intel-x86_64.txt` | vendor `Intel`，arch fallback 不破坏 lscpu |
| AMD x86_64 `/proc/cpuinfo` | `AuthenticAMD` alias | 否 | `.../proc-cpuinfo-amd-x86_64.txt` | vendor `AMD` |
| Hygon `/proc/cpuinfo` | 国产 x86 vendor | 否 | `.../proc-cpuinfo-hygon.txt` | vendor `Hygon` |
| Zhaoxin `/proc/cpuinfo` | 兆芯 vendor_id/model 差异 | 否 | `.../proc-cpuinfo-zhaoxin.txt` | vendor `Zhaoxin` |
| Loongson LoongArch `/proc/cpuinfo` | `model name`/Hardware/arch 差异 | 否 | `.../proc-cpuinfo-loongson-loongarch64.txt` | vendor `Loongson`，arch `loongarch64` |
| Phytium ARM64 `/proc/cpuinfo` | ARM64 Hardware fallback | 否 | `.../proc-cpuinfo-phytium-arm64.txt` | name 来自 Hardware/Processor，vendor `Phytium` |
| Kunpeng ARM64 `/proc/cpuinfo` | Huawei/Kunpeng alias | 否 | `.../proc-cpuinfo-kunpeng-arm64.txt` | vendor `Huawei` 或 `Kunpeng` normalization 按设计固定 |
| HiSilicon ARM64 `/proc/cpuinfo` | Kirin/HiSilicon SoC | 否 | `.../proc-cpuinfo-hisilicon-kirin.txt` | vendor `HiSilicon`，name 不为空 |
| `model name` 为空但 `Hardware` 存在 | ARM SoC 常见 | 已有 inline parser/probe 测试 | `crates/hw-parser/tests/cpu_sources.rs`、`crates/hw-probe/tests/existing_category_probes.rs` | CPU name 不退化为 `CPU` |
| `vendor_id` 不存在 | 非 x86 常见 | 否 | `.../proc-cpuinfo-no-vendor-id.txt` | vendor 从 model/hardware alias 推断或保持 None |
| lscpu 与 `/proc/cpuinfo` 不一致 | 多源优先级 | 否 | `.../lscpu-proc-disagree/` | 明确 primary/fallback 策略 |
| dmidecode 不可用或权限不足 | DMI fallback | 部分 runner 可模拟 | `.../dmidecode-permission-denied.txt` | warning，仍输出 CPU/board 基础信息 |
| lspci 不存在 | PCI/GPU fallback | 否 | `.../pci/lspci-missing.expected.json` | warning，不 panic |
| lsusb 不存在 | USB fallback | 已有 inline probe 测试 | `crates/hw-probe/tests/base_probes.rs` | warning，不 panic，并从 sysfs 输出基础 USB 设备 |
| sysfs 字段为空 | robust parser | 否 | `.../sysfs-empty-fields/` | 空字段不生成伪 vendor/product |
| QEMU/VMware/VirtualBox | 虚拟设备标注/过滤 | 否 | `.../virtualization/qemu-vmware-virtualbox.txt` | 标注 virtual 或过滤策略稳定 |

当前 fixtures 可见 PCI/USB/蓝牙/打印机/电源/音频/输入/摄像头等，但缺少 CPU fixture；证据：`qurbrix-hwinfo/crates/hw-testdata/fixtures/pci/lspci-nn-k.txt`，`qurbrix-hwinfo/crates/hw-testdata/fixtures/usb/lsusb.txt`，`qurbrix-hwinfo/crates/hw-testdata/fixtures/proc/asound-cards.txt`，`qurbrix-hwinfo/crates/hw-testdata/fixtures/proc/bus-input-devices.txt`。

## 11. Recommended Implementation Plan

### P0

| 项 | 内容 |
| --- | --- |
| 目标 | CPU 在 `lscpu` 失败、字段缺失、非 x86 字段差异时仍能输出正确 name/vendor/arch/core/thread/frequency |
| 涉及模块 | `crates/hw-probe/src/existing.rs`、`crates/hw-parser/src/cpu.rs`、`crates/hw-model/src/properties.rs`、`crates/hw-testdata/fixtures/cpu/` |
| 推荐实现方式 | 已原创实现 `parse_proc_cpuinfo`；`CpuProbe` 先跑 `lscpu`，再读取 `/proc/cpuinfo` 补空，并用 `dmidecode -t 4` 补 DMI speed/core/thread |
| 不建议照抄 reference 的原因 | Deepin/Kylin 绑定 Qt/Python/发行版临时文件和 UI 模型；qurbrix 应保持 Rust parser + source runner 抽象 |
| 建议测试 | Intel/AMD/Hygon/Zhaoxin/Loongson/Phytium/Kunpeng/HiSilicon fixtures |
| 验收标准 | 所有 fixtures 输出 name/vendor/arch 不为空；命令缺失/权限不足只产生 warning，不中断完整报告 |

### P1

| 项 | 内容 |
| --- | --- |
| 目标 | 国产 CPU/GPU/SoC 和非 x86 架构归一化 |
| 涉及模块 | `hw-parser` normalization helper、CPU/GPU/audio/network parsers |
| 推荐实现方式 | 小型 alias 表：Intel/AMD/Hygon/Zhaoxin/Loongson/Phytium/Huawei/Kunpeng/HiSilicon/Sunway；arch alias：x86_64/aarch64/arm64/loongarch/loongarch64/sw_64 |
| 不建议照抄 reference 的原因 | Kylin vendor 表覆盖 UI 展示和旧硬件，范围过大且包含发行版语义 |
| 建议测试 | vendor alias unit tests；国产 GPU lspci 样本；Kirin/HiSilicon `/proc/hardware` optional sample |
| 验收标准 | 国产平台样本 vendor 稳定归一化，原始 source evidence 保留 |

| 项 | 内容 |
| --- | --- |
| 目标 | 显示器 EDID 和 DRM/sysfs fallback |
| 涉及模块 | `MonitorProbe`、monitor parser、fixtures |
| 推荐实现方式 | `xrandr --verbose` 或 `/sys/class/drm/*/edid` optional source，解析 manufacturer/product/week/year/size |
| 不建议照抄 reference 的原因 | Kylin 写 `/tmp/edid.dat` 并调用 `edid-decode`，不适合 library 并发和无副作用要求 |
| 建议测试 | 内屏无 EDID name、外接显示器 EDID、headless xrandr 失败 |
| 验收标准 | 有 EDID 时 vendor/product/size/date 填充；无 EDID 不影响 resolution |

### P2

| 项 | 内容 |
| --- | --- |
| 目标 | 网络、USB、存储、DMI 的 fallback 和过滤 |
| 涉及模块 | `NetworkProbe`、`UsbProbe`、`StorageProbe`、`BiosProbe`、parser tests |
| 推荐实现方式 | network 加 `/sys/class/net` driver/type/wireless/virtual 标注；USB 已补无 `lsusb` 时的 `/sys/bus/usb/devices/*` fallback，后续加 `lsusb -v` optional interface/maxpower；DMI 加 `/sys/class/dmi/id` fallback；storage 加 `/sys/block` rotational/wwn |
| 不建议照抄 reference 的原因 | 参考项目大量展示逻辑、发行版服务和临时文件协议不适合 qurbrix |
| 建议测试 | virtual NIC、USB hub、空 serial、dmidecode permission denied |
| 验收标准 | 关键字段补齐，虚拟/无效设备不会污染核心类别 |

### P3

| 项 | 内容 |
| --- | --- |
| 目标 | 展示增强和可选 heavy source |
| 涉及模块 | output views、optional source configuration |
| 推荐实现方式 | `glxinfo` renderer、`hwinfo`/`lshw` 作为可选 feature，不作为默认硬依赖 |
| 不建议照抄 reference 的原因 | qurbrix 的 CLI contract 应稳定、快速、少依赖 |
| 建议测试 | command missing、timeout、large output |
| 验收标准 | 默认扫描不显著变慢；可选 source 缺失时 warning 可解释 |

## 12. Evidence Appendix

* `qurbrix-hwinfo/crates/hw-collect/src/collector.rs:21-39`：证明 qurbrix 注册的硬件类别和 probe orchestration。
* `qurbrix-hwinfo/crates/hw-collect/src/collector.rs:63-80`：证明 qurbrix 有 consumed PCI 过滤和基础 dedup 后处理。
* `qurbrix-hwinfo/crates/hw-collect/src/merge.rs:4-20`：证明当前去重只按 `Device.id` 合并。
* `qurbrix-hwinfo/crates/hw-source/src/runner.rs:22-59`：证明命令执行、文件读取有 timeout/missing/permission/failed 分类。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`：当前 CPU probe 调用 `lscpu`、`lshw -class processor`、`dmidecode -t 4` 并读取 `/proc/cpuinfo` fallback。
* `qurbrix-hwinfo/crates/hw-parser/src/cpu.rs`：当前 CPU parser 覆盖扩展 lscpu、DMI type 4 和 `/proc/cpuinfo` `Hardware`/`Processor` fallback 字段。
* `qurbrix-hwinfo/crates/hw-model/src/properties.rs`：CPU model 有 frequency/flags 字段，当前由多源 parser 填充主要字段。
* `qurbrix-hwinfo/crates/hw-probe/src/pci.rs:22-83`：证明 PCI 当前来源和 driver 解析路径。
* `qurbrix-hwinfo/crates/hw-probe/src/usb.rs`：证明 USB 当前使用基础 `lsusb`，并在 `lsusb` 不可用时 fallback 到 `/sys/bus/usb/devices/*`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`：证明 storage 当前用 `lsblk`，并在 `lsblk` 失败时 fallback 到 `/sys/block/*`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:168-229`：证明 memory 当前依赖 `dmidecode -t memory`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:242-295`：证明 BIOS/board 当前依赖 `dmidecode -t 0,1,2,3`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:309-354`：证明 GPU 当前基于 `lspci -nn -k`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:368-399`：证明 monitor 当前只用 `xrandr --query`。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/GetInfoPool.cpp:85-122`：证明 Deepin 收集多种硬件数据源。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceFactory.cpp:29-69`：证明 Deepin 按架构/board vendor 选择 generator。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/commonfunction.cpp:25-33`：证明 Deepin 架构 alias 覆盖 arm64、amd64、sw_64、loongarch。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceGenerator.cpp:173-259`：证明 Deepin CPU 合并 `lscpu`、`lshw_cpu`、`dmidecode4` 并做 DMI fallback。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:224-289`：证明 Deepin CPU 字段解析更完整。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:306-323`：证明 Deepin 对 Loongson/lshw 覆盖有特殊处理。
* `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:365-390`：证明 Deepin 对 Loongson/DMI/飞腾当前频率有特殊处理。
* `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:62-71`：证明 Kylin 读取 `/proc/hardware` 判断 Kirin 990/9006C。
* `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py:220-228`：证明 Kylin `/proc/cpuinfo` 使用 `Hardware` fallback。
* `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:409-565`：证明 Kylin 有跨类别 vendor alias 表，包含虚拟机、国产 GPU、Loongson/HiSilicon 声卡等。
* `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:704-745`：证明 Kylin CPU vendor 归一化覆盖 Phytium/Huawei/Hygon/Zhaoxin/Loongson/Intel/D2000。
* `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:1339-1411`：证明 Kylin 通过 xrandr verbose/EDID/edid-decode 提取显示器属性。
