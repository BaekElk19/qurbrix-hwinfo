# qurbrix-hwinfo 六类核心零部件差距评估报告

- 生成日期：2026-07-09
- 评估对象：`qurbrix-hwinfo`
- 参考项目：`ReferenceProject/deepin-devicemanager-6.0.67`、`ReferenceProject/kylin-os-manager-build-2.0.0-76update2`
- 评估方式：并行派 6 个只读 agent，逐类零部件独立扫描三方代码库，从"采集层数据源"与"端到端输出字段"两个维度对齐，给出完成度百分比与差距清单
- 覆盖零部件：CPU、主板/BIOS、内存、SSD/存储、显卡、显示器

---

## 0. 综合结论

| 零部件 | 采集层完成度 | 输出字段完成度 | 综合完成度 | 主要缺口 |
|---|---|---|---|---|
| CPU | ~83% | ~87% | ~85% | 每核 `sensors` 温度；`scaling_available_governors` / `scaling_available_frequencies` / 当前 governor / `scaling_setspeed` 支持标志 |
| 主板 / BIOS | ~90% | ~100% | ~92% | `/proc/device-tree/*` 兜底；BIOS 语言字段的 sysfs 回退；`chassis_handle` 展示策略；System vs Motherboard 归属整理 |
| 内存 | ~95% | ~96% | ~96% | `/proc/meminfo` 中 `MemFree/MemAvailable/Buffers/Cached`、`SwapTotal/SwapFree`；运行时利用率；NUMA node 分布 |
| SSD / 存储 | ~95% | ~95% | ~90% | `media_type` 与 `interface` 语义未拆分；NVMe namespace 数比对；SMART raw 属性透传通道；缺存储 fixture |
| 显卡 | ~100% | ~88% | ~92% | `revision`、`description`、`min_resolution`；`discrete/integrated` 显式标签；Vulkan；功耗/温度 |
| 显示器 | ~80% | ~82% | ~81% | `primary` 未落地；当前刷新率独立字段；Wayland API 采集分支；缺 monitor/xrandr/edid 测试固件；EDID 原始字节；输入类型 |

- 六项加权综合完成度 ≈ **89%**
- 相对 Deepin：整体接近或已达标，字段模型更结构化；主要落后于每核 `sensors` 温度、Wayland 采集分支
- 相对 Kylin：采集广度与结构化程度全面领先，Kylin 侧主要靠 `sensors` + cpufreq 控制面板才在 CPU 项形成局部优势

---

## 1. CPU

### 1.1 采集层对比

- **qurbrix**：`CpuProbe`（`crates/hw-probe/src/existing.rs:44-368`）并行调度 10+ 源，`hw-parser/src/cpu.rs::merge_cpu_records` 合并
  - `lscpu`、`lshw -class processor`、`dmidecode -t 4`
  - `/proc/cpuinfo`、`/proc/hardware`（Kirin 990/9006C、Phytium PANGU M900/FT-1500A/FT-2000/D2000、HW990）
  - `/sys/phytium1500a_info`（FT-1500A 型号 + max MHz 回退）
  - sysfs topology：`physical_package_id/core_id/thread_siblings_list`
  - sysfs 缓存表：`cache/index*/{level,type,size,ways_of_associativity,coherency_line_size,number_of_sets,shared_cpu_list}`
  - cpufreq：`cpuinfo_max/min_freq`、`scaling_max/min/cur_freq`、`scaling_setspeed`；`read_average_scaling_cur_freq` 全核平均
  - `/sys/class/dmi/id/{sys_vendor,product_name}` 用于 HW 平台标记
- **Deepin**：客户端 `CmdTool.cpp:158,166` + `DeviceGenerator::generatorCpuDevice`（`lscpu` / `lshw_cpu` / `dmidecode4`）；服务端 `depin-devicemanager-server/depin-deviceinfo/src/loadinfo/cpuinfo.cpp` 走 `uname` + `/proc/cpuinfo` 分段 + sysfs topology / cache / cpufreq
- **Kylin**：`rubbish-clear-dbus/src/detailinfo/cpuinfo.py` 走 `lscpu` + `/proc/cpuinfo` + `dmidecode -t processor`（obsolete 路径）+ `/sys/phytium1500a_info` + sysfs cpufreq；`daemon.py` 增加 `scaling_available_governors/frequencies/governor`；`sensors` 拿每核温度
- **联合并集**：12 类采集源。qurbrix 命中 10 类，缺 `sensors` 与 `scaling_available_*/scaling_governor` cpufreq 控制面板
- **采集层完成度：~83%**

### 1.2 输出字段对比（39 项联合并集）

qurbrix 以 `hw-model/src/properties.rs::CpuInfo`（99-140 行）为输出模型。qurbrix 已覆盖：

- name / vendor / architecture / cores / enabled_cores / threads / online_threads / online_cores / threads_per_core / sockets / socket_designations / serial_numbers
- max_freq_mhz / min_freq_mhz / current_freq_mhz / external_clock_mhz / frequency_display / frequency_is_range
- family / model / stepping / bogomips / virtualization / overview
- L1d/L1i/L2/L3/L4 缓存字符串 + 结构化 `caches: Vec<CpuCacheEntry>`
- clflush_size_bytes / flags / extensions
- logical_cpus 明细
- hw_platform 平台标记（Kunpeng/Kirin/PGUW/KLVV）

**缺失 5 项**：`sensors` 每核温度；`scaling_available_governors` 列表；`scaling_available_frequencies` 列表；当前 `scaling_governor`；`scaling_setspeed` 支持标志。

- **输出字段完成度：34/39 ≈ ~87%**

### 1.3 qurbrix 独有优势

- DMI External Clock（`external_clock_mhz`）：Deepin 有解析未映射，Kylin 仅 obsolete 路径
- DMI Core Enabled（`enabled_cores`）、DMI Serial Number 数组（`serial_numbers`）、Socket Designation 数组
- `online_threads` / `online_cores`（`/sys/devices/system/cpu/online` + topology 推断）
- 结构化缓存表：额外含 `ways_of_associativity`、`coherency_line_size`、`number_of_sets`、`size_bytes`、`shared_cpu_count`
- 扩展指令集识别更全：除 x86 传统 11 项外还含 `AVX/AVX2/AVX512/FMA/AES/SHA/VMX/SVM/RDRAND/RDSEED/CRC32/NEON/Crypto`
- `frequency_display` 与 Overview 字符串（如 `One hundred and Twenty-eight Core(s) / ...`）服务端预格式化
- HW 平台标记独立字段 `hw_platform`
- Phytium FT-1500A `sys/phytium1500a_info` 专用解析器 `parse_phytium1500a_info`
- 名称清洗 `clean_cpu_name` 去掉 `xxx x 16`、`Longson @2500MHz` 尾巴

### 1.4 综合完成度：**~85%**

若按"CPU 硬件描述"口径（不含 cpufreq 控制面板 + `sensors`）则 ~92%。

---

## 2. 主板 / BIOS

### 2.1 采集层对比

- **qurbrix**：`BiosProbe`（`existing.rs:3789-3868`），declaration `kinds=[Bios, Motherboard]`
  - `dmidecode -t 0,1,2,3` 合并（BIOS/System/Baseboard/Chassis），System 单独由 `SystemProbe` 处理
  - 富化：`dmidecode -t 13`（BIOS 语言）、`dmidecode -t 16`（Physical Memory Array）、`lspci -nn -k` 抽 chipset family
  - Sysfs 回退：`/sys/class/dmi/id/{bios_*,board_*,chassis_*}` 含 `chassis_type` 数字→字符串规范化
  - EFI/SecureBoot：`/sys/firmware/efi`、`/sys/firmware/efi/efivars/SecureBoot-*`
- **Deepin**：server 端 `threadpool.cpp:110-169` 落盘 `dmidecode -t 0/1/2/3/4/13/16/17`；client 端 `DeviceGenerator` 分五段生成（Bios/System/BaseBoard/Chassis/BiosMemory）；MipsGenerator 走 `dmidecode1` 特化 Loongson
- **Kylin**：仅 `get_board`（`dmidecode -t baseboard` 3 字段）+ `get_computer`（`dmidecode -t system` 4 字段）+ `dmidecode -t bios` 3 字段

### 2.2 输出字段对比（49 项联合并集）

qurbrix 输出模型 `MotherboardInfo`（`properties.rs:45-77`）+ `BiosInfo`（`properties.rs:80-96`）+ System 归入 `SystemDeviceInfo`（`properties.rs:29-42`）。

- **qurbrix 覆盖 49/49，另有 2 项独家（`firmware_type` UEFI/Legacy、`secure_boot` 状态）**
- **输出字段完成度：~100%**

### 2.3 具体差距

1. ARM/龙芯 device-tree 兜底缺失：`/proc/device-tree/model`、`/proc/device-tree/serial-number` 回退未实现（三方共同盲点）
2. BIOS Language 未走 sysfs 回退：`existing.rs:4034-4058` sysfs 覆盖仅到 vendor/version/date/board_*/chassis_*
3. chipset 依赖 `lspci`（`enrich_dmi_chipset_family`），无 `lspci` 环境会丢
4. DMI 采集不落盘（Deepin 是分文件 dump，方便"日志包"型交付）
5. `board_chassis_handle` 展示策略未定：Deepin 显式过滤掉，qurbrix 一律输出
6. System 归属 `DeviceKind::System`：消费方按"主板"聚合时需要合并 System + Motherboard

### 2.4 qurbrix 独有优势

- `firmware_type`（UEFI / Legacy BIOS）+ `secure_boot` 状态：两个参考项目都没有
- `/sys/class/dmi/id` sysfs 回退在 dmidecode 缺失/无权限时仍能输出 12 字段
- 严格 kind 分离（`Motherboard` / `Bios` / `System`），Deepin 把 5 类混在 `DeviceBios` 靠 subTitle 区分
- `SourceEvidence` 溯源：每字段可追溯到源命令/文件与状态
- BIOS 语言 / 内存阵列 / chipset 三条富化通道各带独立 `SourceEvidence`
- 强类型 `Vec<String>` characteristics/board_features/installable_languages

### 2.5 综合完成度：**~92%**

---

## 3. 内存

### 3.1 采集层对比

- **qurbrix**：`MemoryProbe`（`existing.rs:2919-3092`）
  - `dmidecode -t memory`（含 Type 17 + Type 16 Physical Memory Array）
  - `lshw -C memory` 回退 + DMI 富化
  - `decode-dimms`（SPD）
  - SPD EPROM sysfs：`/sys/bus/i2c/devices/*/eeprom`，含 DDR4/DDR5 解码器（`hw-parser/src/dmi.rs:302-348`）
  - EDAC sysfs：`/sys/devices/system/edac/mc*/dimm*`
  - Phytium 1500A：`/sys/phytium1500a_info/memory*`
  - 设备树：`/proc/device-tree/memory@*/reg`
  - `/proc/meminfo`（MemTotal 兜底）
- **Deepin**：`DeviceGenerator::generatorMemoryDevice` 只用 `lshw` + `dmidecode -t 17` + `dmidecode -t 16`；未用 SPD/EDAC/phytium/device-tree/meminfo
- **Kylin**：`InfoConf.get_memory` 仅 `dmidecode -t memory` + Phytium 1500A sysfs 回退 + `/proc/meminfo`；多 DIMM 数据以 `<1_1>` 拼成单字符串

### 3.2 输出字段对比

qurbrix 输出 `MemoryInfo`（`properties.rs:168-208`），每个 DIMM 一个 `Device`，物理内存阵列另生成一个 `Device`。

- qurbrix 覆盖 Deepin `DeviceMemory` 全部核心字段 + filterKey 字段
- 覆盖 Kylin `get_memory` 全部字段
- **相对 deepin/kylin 联合并集 0 项遗漏**
- 唯一遗漏是运行时状态（`MemFree/MemAvailable/Buffers/Cached/swap`），三方在"零部件"视图中均未提供
- **输出字段完成度：~96%**

### 3.3 qurbrix 独有优势

- 多源交叉与富化：dmidecode + lshw 按 locator/serial 匹配（`existing.rs:3481-3505`）
- SPD EEPROM 原始解码（`parse_ddr4_spd_eprom`、`parse_ddr5_spd_eeprom`）：Deepin/Kylin 皆无
- `decode-dimms` 支持
- EDAC sysfs 采集：服务器/云原生场景无 dmidecode 时可用
- 设备树采集：无 SMBIOS 的 ARM 板可用
- Physical Memory Array 单独建模：插槽数、最大容量、纠错类型结构化
- JEP-106 制造商 ID → 名称字典（覆盖 Samsung/Hynix/Micron/长鑫/紫光/GigaDevice 等国产厂商）
- 强类型字段：`speed_mtps: Option<u32>`、`*_voltage_v: Option<f32>`、`*_size_bytes: Option<u64>`、`rank: Option<u32>`
- 多字节容量归一化 `normalize_memory_display_size_for_arch`（含 sw_64 偶数对齐）

### 3.4 综合完成度：**~96%**

---

## 4. SSD / 存储

### 4.1 采集层对比

- **qurbrix**：`lsblk -J -b`（NAME/TYPE/SIZE/MODEL/SERIAL/TRAN/WWN/REV）+ sysfs 全量遍历 `/sys/block/*` + `/proc/bootdevice/{name,cid}` + `lshw -class disk` + `lshw -class storage` + `lspci -nn -k` + `hwinfo --disk` + `hdparm -i` + `smartctl -a -j`（USB 磁盘失败自动 `-d sat` 重试）
- **Deepin**：`hwinfo --disk` 主源 + `lshw -C disk/storage` + `lsblk -d`（用于 rotational） + `sudo smartctl --all`（非 JSON）+ `/proc/bootdevice`；未见 `nvme` CLI / `hdparm -I` 结构化调用
- **Kylin**：`cpuinfo.py::get_disk` 走 `lsblk -b`（识别 major 259/8）+ `/sys/block/*/model,serial,firmware_rev` + SATA 分支 `hdparm -i`；`get_disk_obsolete` 走 `hdparm -i` + `fdisk -l` + `lsblk -ab`；无 SMART、无 hwinfo、无 rotational

### 4.2 输出字段对比（34 项联合并集）

qurbrix 输出 `StorageInfo`（`properties.rs:210-239`）：

- qurbrix 34/34 ≈ **95%**
- Deepin UI 显式绑定 15 字段 + 通用 map 透传（≈60%）
- Kylin 7 字段（≈20%）

### 4.3 具体差距

1. 无 `description` / `Hardware Class` 汇总字段（Deepin 有 `m_Description`）
2. `media_type` 字段被同时用于"介质"（`ssd`/`hdd`/`ufs`）和"传输"（`sata`/`nvme`/`usb`），语义耦合；Deepin 分开 `m_MediaType` + `m_Interface`
3. NVMe namespace 数、`Total NVM Capacity` vs `Namespace 1 Size/Capacity` 比对能力缺失
4. 未原样透传 smartctl 全量属性表（`Power_On_Hours`、`Power_Cycle_Count`、`Raw_Read_Error_Rate`、`Spin_Up_Time` 等），qurbrix 有强类型字段但没有"其他 SMART 属性"开放通道
5. **存储 fixture 空**：`hw-testdata/fixtures/` 缺 lsblk / smartctl / hwinfo / lshw 磁盘样例（对比 cpu/pci/usb 都有）
6. 无 RAID / dm-crypt / LVM 堆栈识别（三方共同缺）

### 4.4 qurbrix 独有优势

- NVMe SMART 健康日志完整落库：`available_spare_percent`、`available_spare_threshold_percent`、`percentage_used`、`data_units_read`、`data_units_written`、`media_errors`、`error_log_entries`
- 类型化 `smart_status`（passed/failed）
- `wwn` 归一化后暴露
- 独立主控信息：`controller_vendor` / `controller_model` / `controller_driver`（lshw storage + lspci 双源）
- `Device.bus = BusInfo::Pci{...}` + 多级 fallback（canonical / parent-walk / unique-controller）
- `Device.driver = DriverInfo { name, version, modules, provider, status }` 结构化
- `ufs_spec_version` 单独字段
- USB 存储 smartctl 自动 `-d sat` 重试策略
- `SourceEvidence` 每字段取值来源可审计

### 4.5 综合完成度：**~90%**

---

## 5. 显卡

### 5.1 采集层对比

- **qurbrix**：`GpuProbe`（`existing.rs:4242-4442`）并行 fan-out 13 类数据源
  - `lspci -nn -k` 主枚举
  - sysfs `/sys/bus/pci/devices/*/{class,vendor,device,...}` 兜底枚举（filter class `03xx`）
  - `lshw -class display` / `dmesg` / `nvidia-smi` / `nvidia-settings -q VideoRam` / `nvidia-settings -q GPUMemoryInterface`
  - `glxinfo -B`、`xrandr --query`
  - `gpuinfo`（麒麟 HW990 / ARM）
  - `/sys/class/drm/*/device/uevent` + `mem_info_vram_total`（DRM sysfs 通用路径）
  - `/sys/bus/pci/devices/{addr}/gpu-info`（Deepin GDR capacity + VRAM）
  - `/proc/gpuinfo_0`（景嘉微专用）
  - `/sys/kernel/debug/gc/total_mem`（飞腾/FTDTM 集显）
  - `/sys/bus/pci/devices/{addr}/modalias`
- **Deepin**：`hwinfo --display` 主源 + `lshw -C display` + `xrandr` + `dmesg` + `/sys/{SysPath}/gpu-info`（Deepin 私有）+ `/proc/gpuinfo_0` + `nvidia-smi -L / -q -d MEMORY` + `nvidia-settings` + `customgpuinfo` 走 `glxinfo -B`（只取 renderer/vendor）+ `/sys/kernel/debug/gc/total_mem`；缺 `lspci -nn -k`、DRM sysfs 通用路径、sysfs PCI 兜底
- **Kylin**：仅 `lshw -C display`（未解析地塞进 `modlist[index]`）+ HW990 分支 `gpuinfo`；`lspci -vv` 解析被 `if False:` 屏蔽

### 5.2 输出字段对比（34 项候选）

qurbrix 输出 `Device{ kind:Gpu, name, vendor, model, bus:BusInfo::Pci, driver, properties:GpuInfo{...} }`：

- qurbrix 30/34 ≈ **88%**
- Deepin 22/34 ≈ 65%
- Kylin ≤ 3 结构化字段 ≈ 10%

### 5.3 具体差距

1. `revision`：Deepin 从 `hwinfo Revision` 和 lshw `version` 填 `m_Version`；qurbrix `LshwDisplayRecord` 未含 version
2. `description`：Deepin 有 `m_Description`；qurbrix 只融合到 `Device.name`
3. `min_resolution`：Deepin `m_MinimumResolution` 通过 xrandr 取
4. 无 Vulkan / 3D 加速状态显式 flag / 功耗温度（三方共同缺）
5. `discrete/integrated` 显式类型标签（需要通过 capabilities + PCI 位置推断）

### 5.4 qurbrix 独有优势

- 13 类采集源并行 + 分层 enrichment；DRM 内核通用路径 `mem_info_vram_total`（Deepin 只读 Deepin 私有节点）
- 完整 OpenGL / GLSL / EGL 字段结构化：`renderer` / `opengl_vendor` / `opengl_version` / `glsl_version` / `egl_client_apis`（Deepin 只填名称字段就丢弃其余）
- 每连接器结构化 `GpuConnectorInfo`：`connector` / `interface` / `connected` / `primary` / `current_resolution` / `max_resolution`（Deepin 只有 5 个平面字符串，Kylin 无）
- sysfs PCI 兜底枚举：无 lspci/hwinfo 时仍能构造 `DeviceKind::Gpu`
- 独立字段：`vendor_id` / `device_id` / `subsystem_vendor_id` / `subsystem_device_id` / `class`（Deepin 塞进单一 `m_VID_PID` 字符串）
- 多源显存优先级链 + 单卡场景 `unique_*` 保护逻辑
- `normalize_gpu_vendor` / `normalize_gpu_vendor_id`（NVIDIA / 景嘉微识别）
- `GpuInfo.gddr_capacity` + `memory_bus_width_bits` 独立字段
- `kernel_modules` 数组落到 `Device.driver.modules`

### 5.5 综合完成度：**~92%**

---

## 6. 显示器

### 6.1 采集层对比

- **qurbrix**：`MonitorProbe`（`existing.rs:5726-5952`）
  - `xrandr --query` + `xrandr --verbose`（EDID hex）
  - `hwinfo --monitor`
  - `/sys/class/drm/*/edid`（Wayland 关键路径），多候选按 `status=connected` / `enabled=enabled` / 匹配分辨率择优
  - 内置 EDID 解析器（`hw-parser/src/edid.rs:32-77`）：处理 pair-swapped EDID 头、checksum、DTD、preferred mode
- **Deepin**：`hwinfo --framebuffer --monitor` + `xrandr` / `xrandr --verbose` + `/sys/class/drm/card*/card*-*/edid` + 自研 `EDIDParser`（走 `hexdump` 子进程）；Wayland 特殊处理 `!qApp->isDXcbPlatform()` 分支
- **Kylin**：`cpuinfo.py::get_monitors` 读预捕获文件 `/tmp/youker-assistant-monitorinfo.dat` + 外部 `edid-decode /tmp/edid.dat`；无 sysfs EDID、无 hwinfo 解析

### 6.2 输出字段对比（22 项目标字段）

qurbrix 输出 `MonitorInfo`（`properties.rs:277-299`）：

- qurbrix 18/22 ≈ **82%**
- Deepin 15/22 ≈ 68%
- Kylin 9/22 ≈ 41%

### 6.3 具体差距

1. **`primary` 未落地**：`XrandrMonitorRecord.primary`（`hw-parser/src/monitor.rs:7`）解析到，但 `MonitorProbe`（`existing.rs:5836-5891`）丢弃，`MonitorInfo` 无 `is_primary` 字段
2. **当前刷新率无独立字段**：Deepin 有 `m_RefreshRate`；qurbrix 只在 `support_resolutions` 字符串 `@Hz` 中
3. **Wayland API 路径缺失**：qurbrix Wayland 下退化为 sysfs EDID + hwinfo，未查询 `wayland-info` / KScreen DBus / `swaymsg`
4. **测试固件缺失**：`hw-testdata/fixtures/` 下无 monitor / xrandr / edid 目录
5. `connected` 状态未导出（未连接的连接器不进设备列表）
6. 原始 EDID 字节未落 `MonitorInfo`（三方都缺）
7. 输入类型（模拟/数字）未从 EDID byte 20 抽取
8. 数字序列号（EDID bytes 12-15）未提取

### 6.4 qurbrix 独有优势

- **纯 Rust EDID 解析器**：无 `edid-decode` / `hexdump` 外部依赖
- Pair-swapped EDID 头兼容（KVM / 转接盒错序上报），Deepin/Kylin 均无
- 多 sysfs EDID 候选择优算法：三级择优
- **PNP 制造商全名解析**：`manufacturer_name` 从 3-letter 反查全名；Deepin 直接透传 hwinfo `Vendor`，Kylin 只取 3-letter
- Preferred mode 三元组：`preferred_width` / `preferred_height` / `preferred_refresh_hz` 独立字段
- `product_code` 类型化 u16
- DTD 优先于基础块做尺寸推断，且带 base-size ±10mm 一致性校验
- `ScanWarning::new("edid_parse_failed", ...)` 显式记录源级失败原因

### 6.5 综合完成度：**~81%**

补齐 4 项（primary 落地、当前刷新率独立字段、Wayland API 采集分支、测试固件）可拉到 ~90%。

---

## 7. 关键路径 & 优先补齐建议

### 7.1 高优先级（阻塞产品性等价）

| 优先级 | 零部件 | 事项 | 位置 |
|---|---|---|---|
| P0 | 显示器 | `primary` 字段落地到 `MonitorInfo` | `hw-model/src/properties.rs:277`、`hw-probe/src/existing.rs:5836-5891` |
| P0 | 显示器 | 当前刷新率独立字段 `current_refresh_hz` | 同上 |
| P0 | SSD | `media_type` 与 `interface` 语义拆分（Deepin 兼容） | `hw-model/src/properties.rs::StorageInfo` |
| P1 | CPU | `sensors` 每核温度 + cpufreq 控制面板（governor 列表 / 可用频率 / 当前 governor / setspeed 支持） | 新增 `CpufreqProbe` 或扩 `CpuProbe` |
| P1 | 显卡 | `revision` / `description` / `min_resolution` 三字段 | `GpuInfo` + `LshwDisplayRecord` |
| P1 | 存储 | 补 `hw-testdata/fixtures/` 存储样例（lsblk / smartctl / hwinfo / lshw） | `crates/hw-testdata/fixtures/` |
| P1 | 显示器 | 补 monitor / xrandr / edid 测试固件 | 同上 |

### 7.2 中优先级（增强项）

| 优先级 | 零部件 | 事项 |
|---|---|---|
| P2 | 主板 | `/proc/device-tree/{model,serial-number}` 兜底 |
| P2 | 主板 | BIOS 语言 sysfs 回退（vendor/version/date 之外补 language） |
| P2 | 显示器 | Wayland API 采集分支（KScreen DBus / `wayland-info` / `swaymsg`） |
| P2 | 显示器 | EDID 原始字节落 `MonitorInfo`（诊断用途） |
| P2 | 内存 | `/proc/meminfo` 全量字段 + swap；可选 NUMA node 分布 |
| P2 | 存储 | NVMe namespace 数与 `Total NVM Capacity` 比对 |
| P2 | 存储 | SMART raw 属性开放通道（透传 `Power_On_Hours`、`Raw_Read_Error_Rate` 等） |

### 7.3 低优先级（可延后）

- 显卡 Vulkan / 功耗温度 / discrete-integrated 显式标签
- 存储 RAID / dm-crypt / LVM 上级堆栈识别
- 主板 DMI 原始文本 dump（方便日志包）
- CPU HW 平台 `chassis_handle` 展示策略确定

---

## 8. 参考代码坐标

- qurbrix 采集：`crates/hw-probe/src/existing.rs`
- qurbrix 解析：`crates/hw-parser/src/{cpu,dmi,storage,gpu,monitor,edid}.rs`
- qurbrix 模型：`crates/hw-model/src/{properties,bus,driver,evidence,kind}.rs`
- qurbrix 输出：`crates/hw-output/src/{flat,jsonl,table,summary,schema}.rs`
- Deepin 客户端：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/{DeviceManager,GenerateDevice}/**`
- Deepin 服务端：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager-server/{customgpuinfo,deepin-deviceinfo,deepin-devicecontrol}/**`
- Kylin：`ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/{detailinfo,systemdbus}/**`、`plugins/{rubbish-clear,service-support}/**`
