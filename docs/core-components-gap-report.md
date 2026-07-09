# qurbrix-hwinfo 六类核心零部件差距评估报告

生成日期：2026-07-09
评估目标：`qurbrix-hwinfo` 对照 `ReferenceProject/deepin-devicemanager-6.0.67`（主）与
`ReferenceProject/kylin-os-manager-build-2.0.0-76update2`（辅），在 CPU、主板/BIOS、内存、SSD/存储、
显卡、显示器六类零部件上的能力差距与完成度。

策略基线：Deepin 为主要拉齐目标；在识别到 Kylin 系统运行时可切换到 Kylin 专属策略作为补充。

评估方法：以只读方式扫描三个代码库，逐类别形成"数据源 + 字段 + 归一化 + 特殊机型识别"四维差距表，按项打分
（Yes=1、Partial=0.5、No=0），再按数据源/核心字段/扩展字段/UI 归一的权重汇总，得出对齐比例区间。

关键文件坐标（只读证据）：
- Deepin：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/**`、
  `deepin-devicemanager-server/{customgpuinfo,deepin-deviceinfo,deepin-devicecontrol}/**`
- Kylin：`ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/**`、
  `debian/patches/0049-73.patch`、`0110-92.patch`
- qurbrix：`crates/hw-{model,parser,probe,source,collect}/**`、`docs/hardware-compatibility-*.md`

---

## 0. 总览

| 零部件 | vs Deepin 完成度 | vs Kylin 完成度 | 主要缺口摘要 |
|---|---|---|---|
| CPU | 约 82% | 约 95% | per-logical-CPU 明细树、PANGU M900 特判、`isHwPlatform` 语义、Overview 文案；Phytium 1500a `/sys/phytium1500a_info` 兜底 |
| 主板 / BIOS | 约 100% | 约 105%（已超） | 已补 SMBIOS Version、BIOS ROM/Runtime/Address/Characteristics/Revision、Chipset Family、国产 vendor 归一化、Board Features/Type/Chassis Lock；保留 devicetree/fwupd/mokutil 等超参考增强为后续项 |
| 内存 | 约 98% | 约 98% | 已达到 1.0 试用口径；剩余为真机 fixture 覆盖和保守匹配尾部风险，不阻塞上线 |
| SSD / 存储 | 约 100% | 约 140%（已超） | 按本地 Deepin/Kylin 对齐口径已补 UFS `spec_version`、SATA Speed / RotationRate 文本、`/proc/bootdevice/cid` + `unique_number` 兜底、VID_PID / PhysID / Modalias、Capabilities、国产品牌前缀、容量展示归一、USB 桥接盘 `smartctl -d sat` 重试、PCI controller 父子链接 |
| 显卡 | 约 60-65% | 约 130%（已超） | `/sys/kernel/debug/gc/total_mem`（Vivante GC / 飞腾）、景嘉微 JJW dmesg 正则、显示接口 connector 归并、位宽/时钟/IRQ/Capabilities/IOPort/MemAddress、EGL/GLSL、xrandr 分辨率写回 GpuInfo |
| 显示器 | 约 65-70% | 约 110%（已超） | 多 DTD 尺寸解析 + base-vs-DTD 交叉校验、大端 EDID、Interface 分类字段、Aspect Ratio、全支持模式 @Hz 列表、0xFE Alphanumeric Name |

综合基线（简单平均）：**vs Deepin ≈ 84%，vs Kylin ≈ 113%**。qurbrix 已经普遍超越 Kylin
的采集广度（Kylin 本身仅走 lscpu/dmidecode/lsblk/lshw display 等基础通道），主拉齐工作
集中在 Deepin 侧：国产 SoC 特判、per-logical/per-DIMM 细粒度模型、UI 展示字段（Overview、
Aspect Ratio、Interface 分类）与国产厂商归一化。

---

## 1. CPU

### 1.1 数据源对比

| 数据源 | Deepin | Kylin | qurbrix |
|---|---|---|---|
| `lscpu` | Y | Y | Y (`crates/hw-parser/src/cpu.rs:95-210`) |
| `/proc/cpuinfo`（全解析成三层树） | Y | Y（部分） | Y（聚合，无三层树；`cpu.rs:212-319`） |
| `lshw -C processor` | Y | N | Y (`cpu.rs:334-349`) |
| `dmidecode -t 4` | Y | Y | Y (`cpu.rs:351-391`) |
| sysfs topology (`physical_package_id`/`core_id`/`thread_siblings_list`) | Y | N | Y (`existing.rs:646-703`) |
| sysfs cache `index*` | Y | N | Y (`existing.rs:725-756`) |
| cpufreq `scaling_max/min/cur_freq` | Y（每核） | 部分（`scaling_max_freq`） | Y（含全体平均，`existing.rs:880-936`） |
| `/proc/hardware` 特判 | Y（PANGU M900、HW990） | Y（Kirin 990/9006C） | 部分（Kirin 990/9006C，`cpu.rs:321-332`） |
| `Common::isHwPlatform()` → DMI Version 覆盖 name | Y | N | 部分（merge 顺序恒定，无条件化开关） |
| `/sys/phytium1500a_info` 兜底 | N | Y | **N** |
| DBus `lscpu_num` 三计数复核 | Y | N/A | N/A（qurbrix 直连命令） |

### 1.2 字段差距

Deepin 展示字段（`DeviceCpu.h:145-169`）：PhysicalID / CoreID / ThreadNum / Frequency(min-max range)
/ CurFrequency / MaxFrequency / BogoMIPS / Architecture / Family / Model / Step / L1d/L1i/L2/L3/L4 /
Extensions（MMX/SSE 系列/AMD64/EM64T）/ Flags / HardwareVirtual / LogicalCPUNum / CPUCoreNum /
ARM CPU implementer|architecture|variant|part|revision / Overview 文案 `Model (X Core(s) / Y Processor)`。

qurbrix `CpuInfo` 已覆盖 34 字段（`crates/hw-model/src/properties.rs:87-123`），核心字段基本对齐，
但明显缺口：

- 无 per-logical-CPU 明细数组，只有单个聚合 `CpuInfo`
- 无 per-logical current freq 数组，只有全体平均
- 无 frequency-range 展示语义（`is_range` / `frequency_display: {Range|Single}`）
- 无 Overview 文案生成

### 1.3 归一化与特判

- qurbrix Vendor 归一化：`crates/hw-parser/src/normalize/cpu_vendor.rs:1-13`（Intel/AMD/Hygon/Zhaoxin/Loongson/Phytium/HiSilicon/Sunway），比 Kylin 更宽 + `infer_cpu_vendor_from_name` 覆盖 Kunpeng/Kirin/D2000
- Arch 归一化：`normalize/arch.rs`（x86_64/i386/aarch64/loongarch64/sw_64/mips64/riscv64）
- Loongson 名称保护、Kunpeng cores 从 DMI 上修已实现（`cpu.rs:414-419, 610-614, 988-1007`）
- 缺 PANGU M900 关键字扩展与 `isHwPlatform` 条件化 DMI-name 覆盖

### 1.4 backlog

- **P0**：per-logical-CPU 模型（PhysicalCpu → CoreCpu → LogicalCpu）或至少 `Vec<LogicalCpuFreq>`；
  Frequency 展示语义（`is_range`）；Overview 文案生成；`/proc/hardware` 加 PANGU M900/Hygon/常见板载 SoC
- **P1**：`is_hw_platform` runtime 判断 + DMI Version 覆盖 name；Phytium 1500a `/sys/phytium1500a_info`；
  Loongson 3A5000/3A6000/3C5000 名字清洗
- **P2**：Extensions 扩展 AVX/AVX2/AVX512/AES/SHA/VMX/SVM；Cache 结构化 `CacheEntry`；
  三源（DMI/lscpu/sysfs）计数不一致的 `ScanWarning`

---

## 2. 主板 / BIOS

### 2.1 数据源对比

| 数据源 | Deepin | Kylin | qurbrix |
|---|---|---|---|
| `dmidecode -t 0`（BIOS） | Y | Y | Y (`existing.rs:2756-2811` + `dmi.rs:303-399`) |
| `dmidecode -t 1`（System） | Y | Y | Y (`SystemProbe`, `existing.rs:337-386`) |
| `dmidecode -t 2`（Base Board） | Y | Y | Y |
| `dmidecode -t 3`（Chassis） | Y | N | Y |
| `dmidecode -t 13`（BIOS Language） | Y | N | Y (`enrich_dmi_bios_language`, `existing.rs:2814-2848`) |
| `dmidecode -t 16`（Physical Memory Array） | Y | N | Y（归到 BIOS 设备，`existing.rs:2850-2892`） |
| `lspci` ISA Subsystem / host bridge → Chipset Family | Y (`CmdTool.cpp:916-932`) | N | Y（优先 ISA/LPC bridge `Subsystem:`，缺失时回退 host bridge 描述） |
| `SMBIOS x.y.z present` 版本正则 | Y | N | Y |
| `/sys/class/dmi/id` sysfs 回退 | N | N | Y (`existing.rs:2955-2998`) |
| `/sys/firmware/efi` 判 UEFI + SecureBoot | N | N | Y (`existing.rs:2910-2953`) |

### 2.2 字段差距

- 已补：**SMBIOS Version、BIOS ROM Size / Runtime Size / Address / Characteristics / BIOS Revision /
  Firmware Revision**（Deepin `DeviceBios.cpp:227-231`）、**Chipset Family**（Deepin lspci ISA bridge `Subsystem:`，
  qurbrix 同时兼容 host bridge 回退）、**Board Features / Board Type**、**Chassis Lock**、
  **国产平台厂商归一化**（Loongson/Phytium/Hygon/Zhaoxin/Kunpeng/Sunway/BIOSTAR/Colorful 等）。
- 已修复风险：主命令 `dmidecode` 成功但 stdout 为空时，若 `/sys/class/dmi/id` 有可用数据，会主动回退到 sysfs DMI，
  不再直接返回空设备。
- 保守项：Chassis 后半部字段（boot_up/power_supply/thermal/security/oem/height/power_cords/
  contained_elements/sku_number）在 sysfs fallback 路径下仍只能采集内核实际暴露的有限字段；这些字段本身没有 sysfs 标准来源，
  无 root/无 dmidecode 时保持空值而不伪造。
- 优势项：UEFI/SecureBoot 采集在两个参考项目中都缺，是 qurbrix 独占

### 2.3 backlog

- **P0/P1**：本地 Deepin/Kylin 对齐主项已完成：SMBIOS Version、BIOS ROM/Runtime/Address/Characteristics/
  BIOS Revision/Firmware Revision、Chipset Family、国产芯片/主板厂商归一化字典、Chassis Lock、
  Board Features / Board Type、empty dmidecode sysfs fallback。
- **P2**：architecture 分支 fallback（`/proc/device-tree/model`、`/sys/firmware/devicetree`）；
  mokutil / fwupd 交叉校验；DMI type 11/12 采集

### 2.4 风险

- `BiosProbe` 与 `SystemProbe` 各自跑 `dmidecode`，后续可做命令缓存或一次 `-t 0,1,2,3,13,16` 采集来减少开销；
  这属于性能/结构优化，不影响本地 Deepin/Kylin 字段对齐。

---

## 3. 内存

### 3.1 数据源对比

| 数据源 | Deepin | Kylin | qurbrix |
|---|---|---|---|
| `dmidecode -t 17`（主） | Y | Y | Y (`existing.rs:2440-2470`, `dmi.rs:74-131`) |
| `dmidecode -t 16`（Physical Memory Array） | filter key（不显示到 memory 类别） | N | Y（归到 BIOS 设备） |
| `lshw -class memory` | Y（主） | N | Y（DMI 成功路径按 locator/serial 融合；DMI 不可用时兜底） |
| `decode-dimms` SPD 文本 | N | N | Y (`dmi.rs:174-212`) |
| Raw DDR4 SPD EEPROM sysfs | N | N | Y（size/speed/vendor/serial/PN） |
| Raw DDR5 SPD EEPROM sysfs | N | N | 部分（仅 identity；Deepin/Kylin 均未实现 DDR5 SPD size/speed 深解析，不作为拉齐目标） |
| EDAC sysfs `mc*/dimm*` | N | N | Y (`existing.rs:2616-2657`) |
| `/proc/meminfo` 总量兜底 | N | Y（sysinfo） | Y (`existing.rs:2536-2555`) |
| SPD manufacturer ID → 友好名映射 | N | N（原样 uppercase） | Y（常见厂商覆盖，未知保留 JEP106 原始码；全量友好名表不作为对齐目标） |
| `/proc/device-tree/memory@*/`（aarch64） | 部分 | Y（`cpuinfo.py:899-963`） | Y |
| FT1500a 固件长度 sanitize | N | Y | Y |

### 3.2 字段差距

qurbrix `MemoryInfo` 已覆盖 size_bytes/vendor/memory_type/speed_mtps/configured_speed_mtps/
total_width_bits/data_width_bits/min|max|configured_voltage_v/locator/bank_locator/serial/part_number、
Rank、Form Factor、Type Detail、Asset Tag、Module/SubSystem ID、Memory Technology、Operating Mode、
Firmware Version、NVDIMM size 系列字段，并生成 Deepin-style overview 与 Kylin-style MemInfo。
当前同时保留 `<OUT OF SPEC>` 过滤、`No Module Installed` 跳过、Kylin-style `Serial == "0"` 过滤、
`Bank Locator` 兜底 `Locator`、`Manufacturer ID` 兜底 vendor、`Configured Memory Speed` 兜底 `Speed`。

阶段结论（2026-07-09）：内存部分按“拉齐本地 Deepin/Kylin”口径已阶段性完成，可进入 1.0 试用验证。
后续主要通过真实机器 fixture 补尾部兼容，不再因 SPD 表规模或展示层粗兜底阻塞上线。

### 3.3 backlog

- **P0/P1**：已完成本地 Deepin/Kylin 对齐口径；不再新增内存阻塞项。
- **P2（可选增强）**：继续补真实机器 fixture，尤其是 DMI/lshw 字段缺失、locator/serial 均不可用、
  国产平台固件异常值等样本；如产品明确要求“超越 Deepin/Kylin”，再单独立项扩展 SPD 表或更深 SPD 解码。

方向决策（2026-07-09）：本地 Deepin 与 Kylin 参考实现均没有 DDR5 raw SPD size/speed 深解析。当前目标是拉齐本地 Deepin/Kylin 行为，因此 DDR5 SPD 深解析不计入完成度缺口，也不作为后续必做 backlog；除非另有超越参考实现的新需求，只保留 DDR5 identity 解析与 `spd_partial` warning。

方向决策（2026-07-09）：本地 Deepin 没有 SPD/JEP106 全量友好名称表；Kylin 仅有少量厂商字符串归一化，内存路径主要保留 `Manufacturer ID` uppercase。qurbrix 已覆盖常见 SPD manufacturer ID 并保留未知 JEP106 原始码，因此“完整的 SPD manufacturer ID 友好名称表”不作为本地 Deepin/Kylin 对齐缺口，也不阻塞 1.0 试用。

方向决策（2026-07-09）：Kylin/Deepin 的粗展示兜底（如默认 `DDR3`/`DDR4`、`64bit`、`$` 占位）不进入 qurbrix 结构化核心模型。核心字段未知时保持空值并保留 source/warning；只允许展示层做温和文案兜底，避免把未知伪装成已知，影响下游判断和后续多源融合。

### 3.4 风险

- 真实机器 fixture 仍需继续积累；这属于上线试用后的兼容性样本建设，不是 1.0 阻塞项
- DMI 与 lshw 都缺 locator/serial 时保持保守，不强行融合，避免误匹配
- SPD EEPROM 路径需要 root + `at24`/`ee1004`/`spd5118` 等内核支持；不可用时通过 warning/source 透明暴露

---

## 4. SSD / 存储

### 4.1 数据源对比

| 数据源 | Deepin | Kylin | qurbrix |
|---|---|---|---|
| `lsblk -J -b` | Y (`DeviceGenerator.cpp:756`) | Y (`cpuinfo.py:get_disk`) | Y (`existing.rs:2292-2389`) |
| `smartctl --all` | Y（每盘） | N | Y (`-a -j`，NVMe health 完整) |
| `lshw -C disk` | Y (`DeviceStorage.cpp:338`) | N | Y (兜底 disk identity) |
| `lshw -C storage` | Y (`:376`，NVMe controller) | N | Y (controller vendor/model/driver) |
| `hwinfo --disk` | Y（主） | N | Y (兜底) |
| `hdparm -i` | 部分 | N | Y (Kylin-style) |
| sysfs `/sys/block/*/device/*` | 部分 | Y | Y (`storage_devices_from_sysfs`, `existing.rs:1393-1470`) |
| `/sys/class/nvme/*/device/uevent` PCI ancestor | 部分 | N | Y (`existing.rs:1613-1704`) |
| `/proc/bootdevice/cid` + `/sys/.../unique_number` | Y (`getSerialID`, `DeviceStorage.cpp:276-326`) | N | Y |
| `/sys/block/*/device/spec_version`（UFS） | Y (`:252-261`) | N | Y |
| `/sys/block/*/device/queue/rotational` | 部分 | N | Y |

### 4.2 字段差距

qurbrix `StorageInfo`（`properties.rs`）已覆盖 device_node/size_bytes/size_display/media_type/controller_vendor/
controller_model/controller_driver/firmware/wwn/speed/rotation_rate/ufs_spec_version/vid_pid/phys_id/modalias/
capabilities/smart_status/temperature_celsius/power_on_hours/
power_cycle_count/available_spare_percent/percentage_used/data_units_read/data_units_written/
media_errors/error_log_entries——在 SMART/NVMe health 维度**已超过 Deepin**。

阶段性已补：

- **UFS 接口检测**（`spec_version`）并将 media_type 标为 `ufs`
- **SATA Version → Speed 字段**（Deepin lshw `configuration.speed`）
- **RotationRate 显式字段**（`queue/rotational=0` → `Solid State Device`，`1` → `Rotating Media`）
- **Capabilities / VID_PID / PhysID / Modalias**（Deepin 字段语义，PhysID 跟随 VID_PID）
- **嵌入式 Serial 兜底**：`/proc/bootdevice/cid` 优先，`/sys/.../unique_number` 兜底
- **`checkDiskSize` 定制机型容量归一**（`DeviceStorage.cpp:529-559`，511-513GB→"512 GB"）
- **NVMe controller 与 disk 显式父子链接**：storage device `parent_id` 指向 `pci:<address>`，并通过 `consumed` 避免 PCI controller 重复展示
- **USB 桥接盘 `smartctl -d sat` 重试**：primary `smartctl -a -j` 读身份失败时按 SAT 重试

### 4.3 国产品牌前缀

qurbrix `storage_vendor_from_model_prefix`（`existing.rs`）已含：HGST HUS / WDC / HITACHI / HTS /
IC / FUJITSU / MP / TOSHIBA / MK / MAXTOR / PIONEER / PHILIPS / QUANTUM / FIREBALL / **FORESEE** / IBM /
**RS→Longsys** / **ST[0-9]→Seagate** / **YMTC / ZHITAI / ZTC / YEESTOR / MAXIO / GLOWAY /
KingSpec / KINGSTON / SanDisk / SAMSUNG / MICRON / CT (Crucial) / SKHynix / HYNIX / NETAC /
RAMAXEL / BIWIN / CXMT / TIGO / Colorful / Asgard / LEXAR**。

Kylin 补丁 `debian/patches/0049-73.patch:10382-10404` 已引入 `gloway/changxin → 长鑫`、`ymtc → 长江存储`、`st* → 希捷`，
`0110-92.patch` 引入 `RS → Longsys`（二进制补丁编码）。qurbrix 侧同步这些补丁语义即可。

### 4.4 backlog

- **P0/P1/P2**：本地 Deepin/Kylin 对齐主项已完成：vendor 前缀表、UFS `spec_version`、RotationRate / Speed、
  `/proc/bootdevice/cid` + `unique_number` serial 兜底、VID_PID / PhysID / Modalias、Capabilities 文本、
  `checkDiskSize` 容量展示归一、NVMe controller 与 disk 父子链接、USB 桥接盘 `smartctl -d sat` 重试。
- **后续非阻塞验证**：继续补真实机器 fixture，尤其是 UFS/eMMC、USB-SATA 桥、国产 SSD 型号、PCI vendor 与 model prefix 冲突样本。

### 4.5 风险

- 前缀表 first-match-wins 已对 `ST` 收紧为 `ST[0-9]`；仍建议用真实机样本持续校验国产/贴牌 SSD 前缀
- `apply_storage_smartctl` 在 JSON 解析失败时已写入设备级 `parse_failed` warning；真实机上仍需观察 smartctl 非标准输出样本
- `device_id` 优先级 `wwn > serial > node`，与 Deepin 复合 key 不对齐；USB 盘 serial 缺失时跨扫描去重会抖动
- FORESEE vs PCI vendor Longsys 冲突：当前仍保留“显式 vendor 优先，model prefix 仅兜底”的规则；需用真实机样本确认展示口径

---

## 5. 显卡

### 5.1 数据源对比

| 数据源 | Deepin | Kylin | qurbrix |
|---|---|---|---|
| `lspci -nn -k`（class 03） | Y | N | Y (`gpu.rs:31-43`) |
| sysfs PCI class 0x03 fallback | N | N | Y (`existing.rs:3977-4070`) |
| `lshw -class display` | Y | Y (`cpuinfo.py:1846`) | Y (部分字段，`gpu.rs:143-149`) |
| `/sys/class/drm/*/device/mem_info_vram_total` | 未直接用 | N | Y |
| `/sys/bus/pci/devices/*/gpu-info`（Deepin `VRAM total size` 16 进制） | Y | N | Y (`parse_depin_gpu_info_vram_total`) |
| `/proc/gpuinfo_0`（景嘉微 JJW） | Y | N | Y (`parse_proc_gpuinfo_memory_size`) |
| `dmesg` 通用 VRAM 正则 | Y | N | Y |
| `dmesg` JJW 专用正则 `VRAM Size N M` | Y (`CmdTool.cpp:513`) | N | **N** |
| `nvidia-smi` CSV | Y | N | Y (`existing.rs:3816-3843`) |
| `nvidia-settings -q VideoRam` | Y | N | Y (仅唯一 NVIDIA 场景) |
| `nvidia-settings -q GPUMemoryInterface`（位宽） | Y (`CmdTool.cpp:740`) | N | **N** |
| `glxinfo -B` renderer/vendor/version | Y | N | Y (`apply_gpu_glxinfo_enrichment`) |
| `/sys/kernel/debug/gc/total_mem`（Vivante GC / FT-DTM） | Y (`customgpuinfo/main.cpp:52-96`) | N | **N** |
| root privileged DBus helper (customgpuinfo) | Y | N | 架构差异 |
| Kylin `Judgment_HW990` + `gpuinfo` | N | Y (`cpuinfo.py:1881-1901`) | **N**（未捕获 GPU vendor / GPU type 键） |

### 5.2 字段差距

qurbrix `GpuInfo`（`properties.rs:165-174`）：vendor/renderer/opengl_vendor/opengl_version/memory_bytes/
current_resolution/max_resolution。

关键缺口（对齐 Deepin `DeviceGpu.h:117-140`）：

- **显示接口聚合**：DP/HDMI/VGA/eDP/DVI/DigitalOutput/DisplayOutput 七种接口（Deepin xrandr 归并）
- **Width（位宽）**：`GPUMemoryInterface` / lshw `width`
- **Clock / IRQ / Capabilities / IOPort / MemAddress**：lshw 全采集，qurbrix `LshwDisplayRecord` 只取 4 字段
- **EGL / GLSL version**：Deepin 独立字段（`DeviceInfo.cpp:1263,1282,1283`），qurbrix 仅 opengl_version
- **GDDR capacity 独立字段**：Deepin 单独展示（合并到 memory_bytes 无法分列）
- **PhysID / Modalias / VID_PID**：Deepin `DeviceGpu::setHwinfoInfo` 全部记录
- **Current/Min/Max Resolution 挂到 GPU**：模型有字段但 GpuProbe 无写回逻辑（xrandr 结果只落到 Monitor）

### 5.3 国产厂商识别

qurbrix `normalize_gpu_vendor{,_id}`（`hw-parser/src/normalize/gpu_vendor.rs:1-52`）：VID `0731=Jingjia`；
关键字 NVIDIA / AMD / Intel / Matrox / ASPEED / VMware / VirtIO / Loongson / Jingjia / Zhaoxin /
Moore Threads / Innosilicon / WDE。

未覆盖：**摩尔线程 VID `1ed5`**、**沐曦 MXN VID `1eb1`**、**壁仞 Biren**、**海光 Hygon DCU VID `1d17`**、
**华为 Ascend VID `19e5` 相关**、**龙芯完整型号**、**ARM Mali**（Kylin HW990 分支）。

### 5.4 backlog

- **P0**：`/sys/kernel/debug/gc/total_mem` Vivante GC 显存解析（飞腾 D2000 + FT-DTM / 华为 990 板必备）；
  dmesg JJW 专用正则 `VRAM Size N M`；vendor 白名单展开（VID `1ed5` / `1eb1` / `1d17` / `19e5`，
  `hygon`/`mxn`/`biren`/`ascend`/`kunpeng` 关键字）
- **P1**：`GpuInfo` 新增 `memory_bus_width_bits` / `irq` / `clock_mhz` / `capabilities` / `io_port` /
  `mem_address` / `connectors: Vec<Connector>`；`LshwDisplayRecord` 增补 width/clock/irq/capabilities/ioport/memory 键；
  glxinfo 扩展 EGL / EGL client APIs / GLSL；xrandr 分辨率写回 `GpuInfo`；`/sys/class/drm/card*-<CONN>/status`
  按 GPU 归并 connector
- **P2**：Modalias / PhysID 输出；Kylin HW990 `gpuinfo` 兼容（ARM Mali GPU vendor / GPU type）；
  `gddr_capacity` 与 `memory_bytes` 双份显存字段

### 5.5 风险

- `/sys/kernel/debug/gc/total_mem` 需要 CAP_SYS_ADMIN，Deepin 通过 root helper + DBus 走。qurbrix 建议放到 `sudo_required` 通道并优雅降级
- Deepin `setDmesgInfo` 用 `HwinfoToLshw`(busID 后缀) 匹配 VRAM 归属；qurbrix 用完整 PCI 地址粒度更细，
  但 dmesg 中景嘉微行无 domain:bus 前缀时需右对齐匹配
- glxinfo 只在"唯一 GPU"或"vendor 匹配唯一"时应用，多卡异构（Intel 核显 + NVIDIA）会漏（Deepin 也没做）
- `normalize_gpu_vendor` 关键字 `wuhan digital engineering → WDE` 存疑，同时对 Hygon（海光 DCU）完全缺失

---

## 6. 显示器

### 6.1 数据源对比

| 数据源 | Deepin | Kylin | qurbrix |
|---|---|---|---|
| `hwinfo --monitor` | Y (`DeviceMonitor.cpp:98-146`) | apt 依赖 | Y (`existing.rs:4339-4423`, `monitor.rs:126-231`) |
| `xrandr --query` | Y | Y (`monitor_ball.py:1207-1268`) | Y (`existing.rs:4089`) |
| `xrandr --verbose`（EDID hex） | Y (`:225-320,430-499`) | Y | Y (`existing.rs:4093`) |
| `/sys/class/drm/*/edid` | N | Y (通过 `/tmp/edid.dat`) | Y (`existing.rs:4100-4129`) |
| EDID Rust 内嵌解析 | 自研 C++（`EDIDParser.cpp`，525 行） | 外部 `edid-decode` 命令 | Y (`crates/hw-parser/src/edid.rs`) |
| DBus `CurResolution` | Y (`:205-212`) | N | N |
| TOML 配置覆盖 | Y (`:148-178`) | N | N |
| `ddcutil` | N | N | N |

### 6.2 字段差距

Deepin `DeviceMonitor` 字段：Model / DisplayInput / VGA/HDMI/DVI 开关 / Interface / RawInterface /
ScreenSize (含 inch 格式化) / **AspectRatio（GCD 计算+21:9 兜底）** / MainScreen / CurrentResolution /
SerialNumber / ProductionWeek / **SupportResolution（xrandr 全模式列表带 @Hz）** / RefreshRate / Width / Height。

qurbrix `MonitorInfo`（`properties.rs:176-195`）：connector / resolution / max_resolution / size_mm /
production_date / manufacturer_name / product / product_code / serial / manufactured_year / manufactured_week /
size_cm / diagonal_inches / gamma / preferred_width/height/refresh_hz。

关键缺口：

- **多 DTD 尺寸解析**：Deepin `parseDTDs` 遍历 4 个 DTD 块（`EDIDParser.cpp:304-397`），
  qurbrix `edid.rs:62` 仅解析 base 0x15/0x16
- **DTD 尺寸与 base 交叉校验**：|diff|<10mm 用 DTD 值，否则 base（`EDIDParser.cpp:216-222`）
- **大端 EDID 支持**：Deepin `:56-70`；qurbrix 只处理小端
- **Monitor name 0xFE Alphanumeric Data String** 兼容（Deepin `:266`）
- **Interface 分类**：HDMI/VGA/DP/eDP 正则动态提取（Deepin `:437`）；qurbrix 只有原始 connector 字符串
- **Aspect Ratio**：GCD + 21:9/32:9 兜底（Deepin `:501-553`）
- **全支持模式列表**（每模式带 @Hz）：Deepin `:618-676`；qurbrix 仅 max_resolution 首模式

优势项（qurbrix 独占）：

- Rust 内嵌解析，无 `/tmp/edid.dat` 写文件依赖（vs Kylin 的 shell out `edid-decode`）
- 重复 connector 归属策略（`status=connected` / `enabled=enabled` / preferred-mode 匹配 xrandr 当前分辨率），
  Deepin 未显式处理（`existing.rs:4422-4498`）

### 6.3 backlog

- **P0**：EDID 多 DTD 遍历（0x36/0x48/0x5A/0x6C）+ pixel_clock=0 跳过；base vs DTD 尺寸交叉校验；
  Interface 分类字段（从 connector 前缀派生）
- **P1**：Aspect Ratio（GCD + 21:9/32:9 兜底）；完整支持模式列表（解析每 mode 行的 `Rate1 Rate2 *+`）；
  Monitor name 0xFE 兼容；`production_date` 正规化为 `YYYY-MM`
- **P2**：EDID 大小端模式检测；CEA-861 扩展块解析；ddcutil 集成读取亮度/输入源；EDID 三元组指纹去重
  兜底残留的多可读 connector

### 6.4 风险

- qurbrix EDID 解析器严格只处理 128 字节 base block，扩展块字节完全丢弃
- `MonitorInfo.manufacturer_name` 需确认是否内置 PNP → 长厂商名映射表，否则与 hwinfo `Vendor` 冲突
- 重复 connector 策略是净优势，补齐 P0/P1 时勿改弱
- 不建议引入 Kylin 的 `edid-decode` shell out（`/tmp` 写文件的多用户/权限风险）

---

## 7. 综合优先级 backlog

按跨类别影响面重排（P0 影响多类别或高频真机场景优先）：

### P0（Deepin 平价 + 国产 SoC 必需）

1. **CPU per-logical-CPU 明细树 + 每核当前频率**（对应 Deepin `LoadCpuInfoThread` 三层结构）
2. **显卡 `/sys/kernel/debug/gc/total_mem` Vivante GC 显存**（飞腾 D2000 + FT-DTM / 华为 990 板核心指标）
3. **显卡国产 vendor 白名单扩展**（摩尔线程 VID `1ed5` / 沐曦 `1eb1` / 海光 `1d17` / 华为 Ascend `19e5` / 壁仞 Biren）
4. **存储 vendor 前缀表扩展**（YMTC / ZhiTai / Gloway / KingSpec / KINGSTON / SanDisk / SAMSUNG / MICRON / SK Hynix / NETAC / RAMAXEL / BIWIN / CXMT）
5. **存储 UFS 检测**（`/sys/block/*/device/spec_version`）+ RotationRate / SATA Speed 字段
6. **已完成：主板 SMBIOS Version + BIOS ROM/Runtime/Address/Characteristics/Revision + Chipset Family**（Deepin 主 BIOS 面板全字段）
7. **显示器 EDID 多 DTD 尺寸解析 + base-vs-DTD 交叉校验**（Deepin `parseDTDs` 全面对齐）
8. **显示器 Interface 分类**（HDMI/VGA/DP/eDP 从 connector 派生）
9. **内存 DIMM Rank / Form Factor / Type Detail / Asset Tag + Module/Subsystem ID 独立字段**

### P1（国产兼容 + Deepin 常用展示字段）

10. **已完成主板侧：主板/CPU 国产厂商归一化字典**（Loongson / Phytium / Hygon / Zhaoxin / Kunpeng / Sunway / BIOSTAR；CPU 侧此前已覆盖）
11. **CPU `isHwPlatform` runtime 判断 + DMI Version 覆盖 name**（Kunpeng 环境 name 展示）
12. **CPU Phytium 1500a `/sys/phytium1500a_info` 兜底**
13. **CPU frequency-range 展示语义 + Overview 文案生成**
14. **存储嵌入式 Serial 兜底**（`/proc/bootdevice/cid` + `/sys/.../unique_number`）+ VID_PID / Modalias / Capabilities
15. **显卡 `GpuInfo` 新增 memory_bus_width_bits / irq / clock_mhz / capabilities / io_port / mem_address / connectors**
16. **显卡 xrandr 分辨率写回 GpuInfo**；`/sys/class/drm/card*-<CONN>/status` 按 GPU 归并 connector
17. **显卡 glxinfo 扩展 EGL / GLSL**
18. **显示器 Aspect Ratio（GCD）+ 全支持模式列表（每模式 @Hz）+ Monitor name 0xFE 兼容**
19. **已移出 1.0 必做：内存 JEP106 全量友好名表**（本地 Deepin/Kylin 未实现；当前常见表 + 原始码保留已足够对齐）
20. **已完成字段侧：主板 Chassis Lock / Board Features / Board Type**；sysfs Chassis fallback 受内核暴露字段限制，保持保守空值

### P2（长尾）

21. **CPU** Extensions 扩展 AVX/AES/SHA/VMX/SVM；Cache 结构化 `CacheEntry`；三源计数不一致 warning
22. **主板** UEFI 变量 / fwupd / mokutil 交叉校验；DMI type 11/12
23. **内存** NVDIMM 系列字段；`dmidecode -t 16` 冗余映射到 memory 类别；MiB/GiB→GB UI 归一
24. **存储** `checkDiskSize` 定制机型容量归一（可能与"精确 bytes"策略冲突）；USB 桥接盘 `smartctl -d sat` 重试
25. **显卡** Modalias / PhysID 输出；Kylin HW990 `gpuinfo` ARM Mali 兼容；`gddr_capacity` 独立字段
26. **显示器** EDID 大小端检测；CEA-861 扩展块；ddcutil 亮度/输入源；EDID 三元组指纹去重

---

## 8. 系统识别与策略切换建议

当前 qurbrix 采用通用 Linux 多源优先级，未按 `/etc/os-release` 分叉。为对齐"Deepin 主 / Kylin 辅"策略，建议：

1. 在 `hw-collect` 或 `hw-probe/src/context.rs` 增加 `PlatformId { Deepin, Kylin, Uos, Debian, Ubuntu, Other }`，
   源自 `/etc/os-release` 的 `ID` + `ID_LIKE`
2. 类别级差异走 feature flag：
   - **Deepin**：默认策略（当前实现方向），完整启用 hwinfo/lshw/smartctl/customgpuinfo 等相关兜底
   - **Kylin**：额外启用 Phytium 1500a `/sys/phytium1500a_info`、HW990 `gpuinfo` 命令、Kylin 补丁引入的存储厂商前缀
     （`gloway/changxin → 长鑫`、`ymtc → 长江存储`）
3. 避免硬编码分叉：以数据源可用性 + 品牌 vendor 归一化字典驱动，不按 OS ID 硬切换字段结构；OS ID 只用于
   ranker 优先级与个别专有 sysfs 路径（如 `/sys/phytium1500a_info`）

## 9. 完成度评估方法说明

- **分母**：以 Deepin 或 Kylin 在该类别的所有可识别能力项为分母（数据源 + 显示字段 + 归一化 + 特殊机型），
  权重相等；qurbrix 独占能力项不计入分母，但作为加成项微调评分
- **分子**：Y=1、Partial=0.5、N=0；`x/N × 100%` 得到基础对齐比例
- **区间修正**：叠加数据源/核心字段/扩展字段/UI 归一的加权（40%/30%/20%/10%）后取区间上下界
- 结果为快速评估用途，非精确度量；具体百分比在 ±5% 内浮动均视为等效

## 10. 参考文件索引

qurbrix 主实现：

- `crates/hw-model/src/properties.rs`（数据模型）
- `crates/hw-parser/src/{cpu,dmi,storage,gpu,edid,monitor,memory}.rs`（解析）
- `crates/hw-parser/src/normalize/{cpu_vendor,gpu_vendor,arch}.rs`（归一化）
- `crates/hw-probe/src/existing.rs`（主 probe 编排，含 CPU / BIOS / Memory / Storage / GPU / Monitor）
- `crates/hw-collect/src/{collector,merge}.rs`（编排 + 合并）
- `docs/hardware-compatibility-{gap-report,reference-audit}.md`（历史差距快照）

Deepin 主实现：

- `deepin-devicemanager/src/DeviceManager/{DeviceCpu,DeviceBios,DeviceMemory,DeviceStorage,DeviceGpu,DeviceMonitor}.{h,cpp}`
- `deepin-devicemanager/src/GenerateDevice/{DeviceGenerator,CmdTool,HWGenerator,MipsGenerator,CustomGenerator}.cpp`
- `deepin-devicemanager/src/Tool/EDIDParser.cpp`
- `deepin-devicemanager-server/customgpuinfo/main.cpp`（Vivante GC 显存 helper）

Kylin 主实现：

- `rubbish-clear-dbus/src/detailinfo/cpuinfo.py`（CPU / 内存 / 存储 / 显卡集中实现）
- `rubbish-clear-dbus/src/appcollections/monitorball/monitor_ball.py`（显示器）
- `rubbish-clear-dbus/src/sysinfo/__init__.py`（整体总量兜底）
- `debian/patches/0049-73.patch`、`0110-92.patch`（存储厂商前缀补丁）
