# qurbrix-hwinfo 全零部件差距评估报告 (2026-07-11)

- 生成时间：2026-07-11
- 评估对象：`qurbrix-hwinfo`
- 参考项目：`ReferenceProject/deepin-devicemanager-6.0.67`、`ReferenceProject/kylin-os-manager-build-2.0.0-76update2`
- 评估方式：并行派 9 个只读 agent，逐类零部件独立扫描三方代码库，从「采集层数据源」与「端到端输出字段」两个维度对齐，给出完成度百分比与差距清单
- 覆盖零部件（12 类）：CPU、主板 / BIOS、内存、存储 (HDD/SSD/NVMe/CD-ROM)、显卡 / 视频、显示器、音频、蓝牙、网络 (有线 / 无线)、USB、输入 (键鼠 / 触摸板)、打印机、电源 / 电池

---

## 0. 综合结论

| 序号 | 零部件 | 采集层完成度 | 输出字段完成度 | 综合完成度 | 主要缺口 |
|---|---|---|---|---|---|
| 1 | CPU | ~92% | ~100% | **~95%** | DBus governor 写入通道、hwmon/thermal_zone 温度、多插槽 subTitle |
| 2 | 主板 / BIOS | ~105% | ~100% | **~102%** | `dmidecode -t 11` OEM 字符串、独立 `ChassisInfo`、`/proc/device-tree` 回退 |
| 3 | 内存（DIMM） | ~118% | ~120% | **~119%** | 运行态利用率、`MemFree/MemAvailable/Buffers/Cached`、Swap、NUMA |
| 4 | 存储 (HDD/SSD/NVMe/CD-ROM) | ~180%(vs depin) | ~135% | **~135%** | 分区/挂载点、TRIM、扇区大小、SMART attributes 表 |
| 5 | 显卡 / 视频 | ~85% | ~90% | **~88%** | `hwinfo --display`、`xrandr --verbose`、Vulkan、GPU 温度/功耗、`driver_version` 未填 |
| 6 | 显示器 | ~85% | ~90% | **~88%** | DBus Display1（Wayland）、`edid_hex` 输出、TOML 机型修正 |
| 7 | 音频 | ~90%(vs depin) | ~60% | **~70%** | IRQ/内存范围/位宽/时钟、Chip、`of_node/compatible`、`aplay -l` |
| 8 | 蓝牙 | ~80% | ~35% | **~50%** | HCI/LMP Version、Manufacturer、Class、Features 全丢弃；`bluetoothctl show` |
| 9 | 网络 (有线/无线) | ~43% | ~46% | **~45%** | ethtool WoL、`hwinfo --netcard`、无线 SSID/信号、MTU 未落库 |
| 10 | USB | ~85% | ~80% | **~82%** | `hwinfo --usb`、USB 拓扑、驱动模块字段 |
| 11 | 输入设备 | ~75% | ~60% | **~68%** | 接口显式字段、Wakeup、蓝牙 pair 关联、Modalias |
| 12 | 打印机 | ~60% | ~55% | **~58%** | IPP 属性 / marker / shared / interface type |
| 13 | 电源 / 电池 | ~80% | ~75% | **~78%** | `dmidecode -t 22` SBDS、AC 适配器（`line_power_*`）、充电阈值 |

**综合加权完成度 ≈ 82%**（12 类平均，硬件核心 6 类 ~ 95%，外设与网络次级 ~ 60%）

**相对参考项目定位**：
- **相对 Deepin**：核心硬件（CPU / 主板 / 内存 / 存储）已达到或超越；显示 / 音频 / 蓝牙 / 网络 落后于 hwinfo 结构化字段解析
- **相对 Kylin**：所有 12 类均全面领先（Kylin 主源码树以 DBus + 命令文本抓取为主，结构化程度低）

---

## 1. CPU

### 1.1 采集层对比

| 项 | qurbrix-hwinfo | deepin | kylin |
|---|---|---|---|
| 命令 | `lscpu`, `lshw -class processor`, `dmidecode -t 4`, `sensors` | `lscpu`, `lshw -c processor`, `dmidecode -t 4` | `lscpu`, `dmidecode -t processor` |
| procfs | `/proc/cpuinfo`, `/proc/hardware`（识别 Kirin/Phytium/HW990） | `/proc/cpuinfo` | `/proc/cpuinfo` |
| sysfs | topology、online、cache/index*、cpufreq/{scaling_governor,available_governors,available_frequencies,scaling_setspeed,cur/min/max_freq,bogomips}、`/sys/phytium1500a_info` | topology、cache/index*、cpufreq/{cpuinfo_min/max_freq,scaling_cur_freq} | `scaling_max_freq`（仅存在性判断） |
| DBus | 无 | 无 | `get_cpu_info` / `get_cpu_sensor` / `get_cpu_range` / `get_cpufreq_scaling_governer_list` / `adjust_cpufreq_scaling_governer`（有写入能力） |
| 运行时监控 | 无 | 无 | `libgtop glibtop_cpu` 采集整机占用 |

代码位置：`qurbrix-hwinfo/crates/hw-probe/src/existing.rs:44-401`，parser `crates/hw-parser/src/cpu.rs`；合并策略 `merge_cpu_records` 优先级 lscpu → proc_cpuinfo → proc_hardware → sysfs → phytium1500a → lshw → dmidecode。

### 1.2 输出字段对比

qurbrix `CpuInfo`（`crates/hw-model/src/properties.rs:98-170`）字段最丰富：name/vendor/architecture、cores/enabled_cores/threads/online_threads/online_cores/threads_per_core、sockets/socket_designations/serial_numbers、max/min/current/external_clock MHz、frequency_display、frequency_is_range、overview、family/cpu_implementer/cpu_architecture/cpu_variant/cpu_part/cpu_revision/model/stepping/bogomips、virtualization、l1d/l1i/l2/l3/l4_cache + `caches[]`、clflush_size_bytes、flags、extensions、`logical_cpus[]`（per-core min/max/current/bogomips/online/temperature）、hw_platform、scaling_governor/scaling_available_governors/scaling_available_frequencies_khz/scaling_setspeed_supported。

deepin：Name/Vendor/Frequency（"X‑Y GHz"）/CurFrequency/MaxFrequency/Architecture/Model/CacheL1Data/CacheL1Order/CacheL2/CacheL3/CacheL4/Extensions(拼接串)/CPUCoreNum/LogicalCPUNum/FrequencyIsRange/subTitle "Processor {physicalID}"。

kylin：CpuVersion/CpuVendor/cpu_cores/cpu_cores_online/CpuCapacity/CpuSlot/CpuSerial/CpuSize/CpuClock/cpu_siblings/clflush_size/cache_size + 动态 cpu_sensor/cpu_range/cpu_average_frequency/governor 列表。

### 1.3 完成度评估

| 维度 | vs deepin | vs kylin | 综合完成度 |
|---|---|---|---|
| 采集层 | 100% | 85%（缺 DBus 接口） | **~92%** |
| 输出字段 | 110% | 100% | **~100%** |
| 综合 | ~105% | ~92% | **~95%** |

### 1.4 主要差距

1. **DBus 服务接入缺口**（对 kylin）：无 governor 写入 API `adjust_cpufreq_scaling_governer`（纯采集器定位）
2. **CPU 实时监控数据未采集**：无 `usage_percent`、`load_avg`
3. **CPU 温度只依赖 sensors**：未读 `/sys/class/thermal/thermal_zone*/temp` 与 `/sys/class/hwmon/*/temp*_input`
4. **多物理 CPU 未拆分设备条目**：`socket_designations`/`serial_numbers` 是 Vec 但没拆成多 Device
5. **无 TOML 冷补丁机制**（对 deepin）：遇到 firmware 错误值无法热修

---

## 2. 主板 / BIOS

### 2.1 采集层对比

| 项 | qurbrix-hwinfo | deepin | kylin |
|---|---|---|---|
| dmidecode | `-t 0,1,2,3` 一次调用 + `-t 1` 独立 System + `-t 13` 语言 + `-t 16` 内存阵列 | `-t 0,1,2,3,11,13,16,17` 分派 | `-t system`, `-t baseboard`, `-t bios` |
| sysfs 双通道 | `/sys/class/dmi/id/*` 全量回退 | 无系统性回退 | 单条 `sys_vendor/product_version/product_name`（补丁） |
| UEFI / Secure Boot | `/sys/firmware/efi` + `SecureBoot-*` | 无 | 无 |
| 运行时上下文 | hostname/os-release/uname -r/uname -m | 无 | 无 |
| Chipset 反推 | `lspci -nn -k` Host Bridge → chipset_family | 无 | 无 |
| 未覆盖 | `dmidecode -t 11` OEM Strings、`/proc/device-tree`、`ipmitool` | | |

### 2.2 输出字段对比

qurbrix 输出（`hw-model/src/properties.rs`，共 57 项）：
- `SystemDeviceInfo` 12 项：hostname、os、kernel、architecture、manufacturer、product_name、version、serial、uuid、wake_up_type、sku_number、family
- `BiosInfo` 15 项：vendor/version/release_date/smbios_version/rom_size/runtime_size/address/characteristics/bios_revision/firmware_revision/firmware_type/secure_boot/language_description_format/installable_languages/currently_installed_language
- `MotherboardInfo` 30 项：主板 10 项 + 机箱 15 项 + 内存阵列 6 项

deepin 通过 `getOtherMapInfo` 透传约 40 键；类成员仅 5 个强类型属性，其余靠 filter key 白名单。

kylin 合计 10 个 BIOS/主板/系统字段。

### 2.3 完成度评估

| 参照 | 采集层 | 输出字段 | 综合 |
|---|---|---|---|
| vs deepin | ~105% | ~100% | **~102%** |
| vs kylin | ~340% | ~570% | ~455% |

### 2.4 主要差距

1. **`dmidecode -t 11` OEM Strings 未采集**（deepin 有）
2. **无独立 `ChassisInfo` / `DeviceKind::Chassis`**：机箱与主板揉在同一结构里
3. **无 `ipmitool` / IPMI FRU 采集**
4. **无 `/proc/device-tree/*` 回退**：ARM/龙芯等无 SMBIOS 平台的 BIOS 缺失时无兜底
5. **Secure Boot 字节位解析简化**（仅取第 4 字节或首字节）

---

## 3. 内存（DIMM + 运行态）

### 3.1 采集层对比

**qurbrix 主源** `dmidecode -t memory`（Type 16 Array + Type 17 Device）；**Fallback 链**：`lshw -class memory` → `decode-dimms` → 原始 SPD EEPROM sysfs（`/sys/bus/i2c/drivers/eeprom/*/eeprom`、`ee1004`、`/sys/bus/nvmem/devices/*/nvmem`，含 DDR4/DDR5 SPD 解码）→ EDAC sysfs（`/sys/devices/system/edac/mc*`）→ Phytium 1500A 专用 sysfs → OpenFirmware/Device Tree `/proc/device-tree/memory@*/reg` → `/proc/meminfo:MemTotal`。lshw 富化在 DMI 之后。

**deepin**：`lshw -C memory` + `dmidecode -t memory` + TOML 覆盖；无 SPD/EDAC/DT。

**kylin**：libgtop2 `glibtop_get_mem`（`memTotal.total/used`） + `/proc/meminfo:MemTotal` + `dmidecode -t memory`（正则抓 DIMM）；无 Swap / NUMA / EDAC / SPD / DT。

### 3.2 输出字段对比

qurbrix `MemoryInfo`（`properties.rs:173-215`）：物理 DIMM 32+ 项（Type 17 全部：size_bytes、vendor、memory_type、speed_mtps、configured_speed_mtps、total_width_bits、data_width_bits、min/max/configured_voltage_v、locator、serial、part_number、error_information_handle、form_factor、set、bank_locator、type_detail、asset_tag、rank、module_manufacturer_id、module_product_id、memory_subsystem_controller_manufacturer/product_id、memory_technology、memory_operating_mode_capability、firmware_version、non_volatile_size_bytes、volatile_size_bytes、cache_size_bytes、logical_size_bytes、overview、mem_info） + Memory Array 6 项独立设备。

deepin：27 项 DIMM 字段（略缺 set/error_information_handle/overview/mem_info），Memory Array 只有 Handle。

kylin：8 项（MemInfo/MemWidth/Memnum/MemSlot/MemProduct/MemVendor/MemSerial/MemSize） + 运行态 `m_memProportion` 使用率。

### 3.3 完成度评估

| 维度 | vs deepin | vs kylin |
|---|---|---|
| 物理 DIMM 视角 | ~118% | ~400% |
| Memory Array（Type 16） | ~600% | ∞ |
| 运行态使用率 | 持平（缺） | 落后（kylin 有 `m_memProportion`） |
| Swap / NUMA | 持平（三方均缺） | 持平 |
| dmidecode 数值语义化（B/MHz/mV） | 领先 | 领先 |
| 综合 | **~119%** | **~300%+** |

### 3.4 主要差距

1. **`/proc/meminfo` 只解析 MemTotal**：`crates/hw-parser/src/dmi.rs:612-620` 未导出 `MemFree/MemAvailable/Buffers/Cached/SReclaimable/Slab/HugePages_*/Committed_AS`
2. **交换分区完全缺失**：未读 `/proc/swaps`、未从 `/proc/meminfo` 抽 `SwapTotal/SwapFree/SwapCached`
3. **NUMA 拓扑完全缺失**：未扫描 `/sys/devices/system/node/node*/{meminfo,numastat,cpumap}`
4. **运行时使用率**：kylin 已有，qurbrix 无 `used_bytes / used_percent`
5. **DDR5 SPD 部分识别的警告**（`spd_record_is_partial_ddr5`）：DDR5 SPD 解析未完全覆盖 size/speed
6. **建议新增独立子域**：`MemoryRuntimeInfo` + `SwapInfo` + `NumaNodeInfo`

---

## 4. 存储（HDD/SSD/NVMe/CD-ROM）

### 4.1 采集层对比

**qurbrix** 9 个数据源：`lsblk -J -b`（主线）+ `lshw -class disk` + `lshw -class storage` + `lspci` + `hwinfo --disk` + `hdparm -I` + `smartctl -j -a`（SAT 自动重试） + sysfs（rotational/modalias/vendor/model/rev/serial/wwid/by-id） + `storage_devices_from_sysfs`（lsblk 失败兜底）。控制器归并：`apply_storage_canonical_pci_identity` / `apply_storage_parent_pci_identity`。

**CD-ROM 独立**：`/proc/sys/dev/cdrom/info` + `lshw -class disk` 过滤 cdrom + `hwinfo --cdrom` + `/sys/class/block/sr*/device/*` 兜底。

**deepin** 5 源：`hwinfo --disk`（主）+ `lsblk -d` + `lshw -class disk/storage` + `smartctl --all` + `/proc/bootdevice/*` UFS 序列号。

**kylin** 3 源（主源码树）：`lsblk -b/-ab` + `hdparm -i` + `/sys/block/*` 与 `device/*` 兜底。无 smartctl、无 CDROM 结构化输出（相关代码被注释）。

### 4.2 输出字段对比

qurbrix `StorageInfo` 28 项 + 顶层：device_node/size_bytes/size_display/media_type/interface/controller_vendor/controller_model/controller_driver/firmware/wwn/speed/rotation_rate/ufs_spec_version/vid_pid/phys_id/modalias/capabilities + SMART：smart_status/temperature_celsius/power_on_hours/power_cycle_count/available_spare_percent/available_spare_threshold_percent/percentage_used/data_units_read/data_units_written/media_errors/error_log_entries。

`CdromInfo` 4 项 + 顶层：device_node/media_present/firmware/capabilities。

deepin 存储 12 项 + CDROM 5 项；SMART 采集了 POH/Temp/RealSect/PendSect 但仅字符串透传。

kylin 存储 6 项，CDROM 0 项。

### 4.3 完成度评估

| 维度 | qurbrix | deepin | kylin | vs deepin | vs kylin |
|---|---|---|---|---|---|
| 存储数据源数 | 9 | 5 | 3 | 180% | 300% |
| CD-ROM 源数 | 4 | 2 | 0 | 200% | ∞ |
| 存储结构字段 | 28 | 12 | 6 | 233% | 467% |
| CD-ROM 结构字段 | 4 | 5 | 0 | 80%（缺 MaxPower/Speed） | ∞ |
| SMART/NVMe 遥测 | 全 NVMe health + ATA 基础 | ATA 字符串透传 | 无 | 130% | ∞ |
| WWN 字段化 | ✓ | ✗ | ✗ | ∞ | ∞ |
| 综合 | | | | **~135%** | **~350%** |

### 4.4 主要差距

1. **分区与挂载点视图**：`lsblk` 未取 `MOUNTPOINT/FSTYPE/PARTUUID/LABEL`
2. **物理/逻辑扇区大小**：未读 `/sys/block/*/queue/{physical_block_size,logical_block_size}`
3. **TRIM 支持标记**：未读 `/sys/block/*/queue/discard_max_bytes`
4. **ATA SMART 具体属性字段化**：只吃 `smart_status.passed`/`temperature.current`/`power_on_time.hours`/`power_cycle_count`；缺 `Reallocated_Sector_Ct` / `Current_Pending_Sector` / `crc_error_count`（deepin 有）
5. **CD-ROM 缺 max_power / speed 字段**；`/proc/sys/dev/cdrom/info` 能力位只识别 3 项
6. **NVMe 多温度传感器数组未取**：`nvme_smart_health_information_log.temperature_sensors` 未映射
7. **`ufs_spec_version` 字段存在但未赋值**（`properties.rs:229`）
8. **USB 磁盘走不到 `BusInfo::Usb`**：`Device.bus` 只在 PCI 分支设置

---

## 5. 显卡 / 视频

### 5.1 采集层对比

**qurbrix** 采集源（`existing.rs:4397-4595`）：`lspci -n -k`（主链路 + sysfs 兜底）+ `lshw -class display`（product/vendor/description/version/bus info/driver/width/clock/irq/capabilities/io_port/mem_address）+ DRM sysfs `/sys/class/drm/*/device/uevent` + `dmesg` VRAM 行 + `nvidia-smi --query-gpu`（含 CSV 显存） + `nvidia-settings -q VideoRam/-q GPUMemoryInterface` + `glxinfo -B`（renderer/vendor/OpenGL/GLSL/EGL） + `xrandr --query`（connectors/current/min/max） + `gpuinfo`（HW990 Mali）+ `/proc/gpuinfo_0`（景嘉微 JW） + `/sys/kernel/debug/gc/total_mem` + `/sys/{SysPath}/gpu-info`。

**deepin**：`lshw -C display` + `hwinfo --display` + `xrandr --verbose` + `xrandr` + `dmesg` + `nvidia-smi -L/-q -d MEMORY` + `nvidia-settings -q VideoRam` + `gpuinfo` + `/proc/gpuinfo_0` + `/sys/{SysPath}/gpu-info` + `/sys/kernel/debug/gc/total_mem`（`customgpuinfo` helper）+ `lspci -v -s <addr>`。

**kylin**：仅 `lshw -C display`（原样堆到 modlist） + `gpuinfo`（HW990 分支）。旧补丁里的 `nvidia-smi/-lspci` 已在 76update2 剥离。

**qurbrix 缺项**：`hwinfo --display/--gfxcard`、`xrandr --verbose`、`vulkaninfo`、GPU 温度/功耗（`nvidia-smi` 只解析显存 CSV，未取 temperature.gpu/power.draw/utilization/fan.speed）。

**视频（camera）** 侧：qurbrix `hw-probe/src/camera.rs`：`v4l2-ctl --list-devices`/`--list-formats-ext` + `lshw -class multimedia` + `/sys/class/video4linux/video*` USB 身份读取。

### 5.2 输出字段对比

qurbrix `GpuInfo` 21 项 + `GpuConnectorInfo` 6 项 + `BusInfo::Pci{address,vendor_id,device_id,subsystem_vendor_id,subsystem_device_id,class}` + `DriverInfo{name,version,modules,provider,status}`：vendor/description/revision/renderer/opengl_vendor/opengl_version/glsl_version/egl_version/egl_client_apis/memory_bytes/memory_bus_width_bits/irq/clock_mhz/capabilities/io_port/mem_address/vid_pid/phys_id/modalias/gddr_capacity/current_resolution/min_resolution/max_resolution/connectors(connector/interface/connected/primary/current_resolution/max_resolution)。

deepin `DeviceGpu` 约 35 项：Name/Vendor/Model/Version/Graphics Memory/Width/DisplayPort/Clock/IRQ/Capabilities/DisplayOutput/VGA/HDMI/eDP/DVI/DigitalOutput/CurrentResolution/MinimumResolution/MaximumResolution/Type(discrete/integrated)/BusInfo/IOPort/MemAddress/Description/Driver/Module Alias/Physical ID/SubVendor/SubDevice/Driver Modules/Config Status/Latency/GDDR capacity/GPU vendor/GPU type/EGL version/EGL client APIs/GL version/GLSL version。

kylin：仅 description/product/vendor 三段字符串。

### 5.3 完成度评估

| 维度 | vs deepin | vs kylin |
|---|---|---|
| 采集数据源 | ~85%（缺 hwinfo --display、xrandr --verbose；额外覆盖 DRM sysfs、nvidia-settings memory-interface） | ~380% |
| 结构化输出字段 | ~90%（缺 discrete/integrated 显式 Type、Display Output 聚合串；结构化 connectors 更细） | ~800% |
| OpenGL / GLSL / EGL | 100% | ∞ |
| Vulkan | 0%（三方均缺） | 并列 |
| GPU 温度/功耗 | 0%（三方均缺） | 并列 |
| 综合 | **~88%** | **≥400%** |

### 5.4 主要差距

1. **未跑 `hwinfo --display` / `--gfxcard`**：拿不到 SysFS ID/Unique ID/SubDevice/Module Alias/Driver Modules
2. **未使用 `xrandr --verbose`**：仅 `--query` 拿不到 EDID/gamma/preferred mode 与 XRandr GPU 关联
3. **无 `vulkaninfo` 采集**：缺 vulkan_api_version/vulkan_driver_name/vulkan_driver_info/vulkan_device_type
4. **无 GPU 温度/功耗**：`nvidia-smi` 未取 `temperature.gpu`/`power.draw`/`utilization.gpu`/`fan.speed`/`clocks.gr`/`clocks.mem`；AMD 未接 `radeontop` 或 `/sys/class/hwmon/*/temp*_input`
5. **无 discrete/integrated 显式标记**：缺 `is_discrete: Option<bool>` 或 `gpu_class: enum`
6. **多 GPU 场景 `glxinfo` 与 `nvidia-settings` 被整体丢弃**：`unique_gpu_count == 1` 才应用
7. **`DriverInfo.version` 字段存在但始终填 None**：未调 `modinfo <driver>` 或 `/sys/module/<driver>/version` 或 `nvidia-smi --query-gpu=driver_version`
8. **无 UVC descriptors / `libcamera-list-cameras`**：新 IPU/CSI 摄像头会漏采

---

## 6. 显示器

### 6.1 采集层对比

**qurbrix** 4 源：`xrandr --query`（connected 状态/primary/当前分辨率+刷新率/最大最小模式/支持分辨率） + `xrandr --verbose`（EDID hex 拼接） + `/sys/class/drm/*/edid`（glob + 多副本用 status/enabled/分辨率三级去重） + `hwinfo --monitor` 富化 vendor/model/size/serial + 原生 EDID 解析器（manufacturer(PNP)/product_code/serial/name/week/year/size_cm/size_mm/gamma/preferred_mode，支持 pair-swapped EDID 头修复）。

**deepin**：`xrandr --verbose` + `xrandr` + DBus 双套 `org.deepin.dde.Display1` (V23) 与 `com.deepin.daemon.Display` (V20) + `/sys/class/drm/*/edid` + `hwinfo --monitor` + TOML 机型级修正 + 自研 EDIDParser。

**kylin**：读预写文件 `/tmp/youker-assistant-monitorinfo.dat`（离线 xrandr 输出）+ 外部 `edid-decode` 二进制。无 DBus/`/sys` 直读/hwinfo/primary/接口类型。

### 6.2 输出字段对比

qurbrix `MonitorInfo`：connector/interface/raw_interface/is_primary/resolution/current_refresh_hz/max_resolution/support_resolutions/aspect_ratio/size_mm/size_cm/diagonal_inches/production_date/manufacturer/manufacturer_name/product/product_code/serial/manufactured_year/manufactured_week/gamma/preferred_width/preferred_height/preferred_refresh_hz。

deepin：Name/Vendor/Model/DisplayInput/VGA/HDMI/DVI 独立位/Interface/RawInterface/ScrenSize(inch+m 复合)/AspectRatio/MainScren/CurrentResolution(含 @Hz)/SerialNumber/ProductionWeek(→年月)/SupportResolution(逗号列表)/RefreshRate/Width/Height。**无 gamma、product_code、preferred_mode 输出**。

kylin：Mon_output/vendor/product/year/week/size(cm×cm)/in(inch)/gamma/maxmode + Vga_num；**无 primary、无 refresh rate、无 support_resolutions、无 aspect ratio、无 interface**。

### 6.3 完成度评估

| 维度 | vs deepin | vs kylin |
|---|---|---|
| 采集源数量 | ~80%（缺 DBus） | ~150% |
| xrandr 解析深度 | ~100% | ~200% |
| EDID 字段（12 vs 5） | ~140% | ~240% |
| 多副本 EDID 去重 | ~130% | ~200% |
| DBus / Wayland 兜底 | **0%** | N/A |
| 综合 | **~90%** | **~180%** |

### 6.4 主要差距

1. **无 DBus 显示服务兜底**：`org.deepin.dde.Display1` / `com.deepin.daemon.Display` 未覆盖；Wayland-only + 无 sysfs EDID 的机型会丢显示器
2. **无 wlr-randr / KScreen / Mutter 覆盖**：纯 Wayland 桌面（GNOME/KDE/wlroots）盲区
3. **无 `edid_hex` 输出字段**：已抓取但仅内部消费；建议加 `edid_hex: Option<String>`
4. **未导出 EDID version/revision、输入类型（digital/analog/bit depth/interface）**：block[18..21] 位未字段化
5. **无 TOML 机型级修正入口**：deepin 的 `setInfoFromTomlOneByOne` 支持 OEM 覆盖 Model/Interface/Size/Serial

---

## 7. 音频

### 7.1 采集层对比

**qurbrix** 覆盖：`/proc/asound/cards`（主）+ `/sys/class/sound/card*` glob + `/sys/class/sound/card*/device/uevent`（DRIVER + PCI）+ `/sys/class/sound/card*/device/{vendor,subsystem_vendor,subsystem_device}`（生成 subsystem id）+ `/proc/asound/card{N}/codec#*`（HDA 编解码器）+ `lshw -class multimedia`（product/vendor/driver 富化）+ `hwinfo --sound`（Model/Driver/Driver Modules/SysFS BusID）+ `pactl list cards`（PulseAudio profiles）。

**deepin**：`hwinfo --sound`（主） + `lshw -C multimedia` + `/proc/asound/card0/codec#0`（KLU 特化） + `/sys/class/sound`（`vendor_name`、`chip_name` 自定义节点） + `dmesg` 抽芯片型号。

**kylin**：`/sys/class/sound/card*` glob + `/proc/asound/card{N}/codec#0` + `driver realpath` + `/sys/class/sound/card{N}/device/of_node/compatible`（平台/SoC）+ 服务支持包内 `hwinfo --sound`。

### 7.2 输出字段对比

qurbrix `AudioInfo`：card_index/card_name/codec/subsystem(ssvid:ssdid)/profiles + 通用 Device 字段（vendor/model/driver.name/driver.modules/bus.pci.address/id/name/sources/warnings）。

deepin `DeviceAudio` 覆盖：Model/Vendor/Driver/DriverModules/BusInfo/**Irq/Memory(地址)/Width/Clock/Capabilities/Chip(dmesg 芯片型号)**/Name/Version/UniqueID/SerialID/SysPath/HardwareClass/Enable/CanUninstall。

kylin：MulNum/MulProduct/MulVendor/MulBusinfo/MulDrive。

### 7.3 完成度评估

| 维度 | vs deepin | vs kylin |
|---|---|---|
| 采集层 | ~90% | ~150% |
| 输出字段 | **~60%** | ~180% |
| 综合 | **~70%** | ~165% |

### 7.4 主要差距

1. **芯片型号 (Chip)**：未把 `codec` 与 dmesg vendor+chip 拼装
2. **平台/SoC 音频**：`of_node/compatible` 分支未处理，SoC 声卡会缺 vendor/product
3. **PCI 详细信息**：`AudioInfo` 未建模 IRQ / Memory Range / Width / Clock / Capabilities
4. **`aplay -l` / `/proc/asound/pcm`**：未使用，缺 PCM playback/capture 能力矩阵
5. **`pactl` 富字段**：仅取 profile 名，未取 active profile、ports、sinks/sources 状态

---

## 8. 蓝牙

### 8.1 采集层对比

**qurbrix**：`hciconfig -a`（主）+ `bluetoothctl paired-devices` + `lshw -class communication`（vendor/product/driver 关联 `logical_name=hciN`）+ `/sys/class/bluetooth/hci*`（address / rfkill*/name / rfkill*/state → powered via `parse_rfkill_unblocked`）。**未用 DBus `org.bluez` / `hcitool dev` / USB VID:PID 关联**。

**deepin**：`hciconfig --all` + `bluetoothctl show <BD>` （Powered/Discoverable/Pairable/UUID/Modalias/Discovering）+ `hwinfo`（Serial ID/Revision/Vendor/Model/SysFS BusID/Driver/Speed/Module Alias/VID_PID）+ `lshw -C communication`。

**kylin**：仅 HW990 分支跑 `hciconfig -a`（按固定行号切片）；无 bluetoothctl/lshw/sysfs/rfkill。

### 8.2 输出字段对比

qurbrix `BluetoothInfo`：address/controller_name/powered/discoverable/paired_device_count/paired_devices[] + Device（vendor/model/driver.name via lshw）。

deepin `DeviceBluetooth` filter key 展开：BD Address/Logical Name/BusInfo/Capabilities/DriverVersion/MaximumPower/Speed/Alias/Model/Vendor/Driver/Version(Revision)/SerialID/UniqueID/SysPath/Modalias/PhysID(VID_PID) + **HCI Version、LMP Version、Subversion、Manufacturer、Class、Service Classes、Device Class、Features、Packet type、Link policy、Link mode**。

kylin：bus/address/service_classes/device_class/**bluetooth version**/**manufacturer**。

### 8.3 完成度评估

| 维度 | vs deepin | vs kylin |
|---|---|---|
| 采集层 | ~80% | ~200% |
| 输出字段 | **~35%** | ~140% |
| 综合 | **~50%** | ~170% |

### 8.4 主要差距

1. **HCI/LMP 版本与厂商**：`hciconfig -a` 输出的 HCI Version/LMP Version/Manufacturer 被 parser 丢弃（`hw-parser/src/bluetooth.rs:28-56` 只匹配 BD Address/Name/全大写 flags）
2. **Bus / Type**：`BluetoothControllerRecord.bus` 已解析但未写入 `BluetoothInfo` / `Device.bus`
3. **Class / Service Classes / Device Class / Features / Packet type / Link policy / Link mode**：完全未采集
4. **bluetoothctl 详细属性**：只调 `paired-devices`，未 `bluetoothctl show <addr>`；`powered/discoverable` 靠 `hciconfig` flags 近似
5. **Modalias / VID_PID / SysFS BusID / Revision / Serial**：无 hwinfo bluetooth 段采集
6. **paired device 详细字段**：只保留 `name`，丢弃 `address`（`BluetoothInfo.paired_devices: Vec<String>` 类型受限）

---

## 9. 网络（有线 / 无线）

### 9.1 采集层对比

**qurbrix**：`ip -j link`（主，失败回退 `/sys/class/net/*` 枚举）+ `ip -j addr` + `lshw -class network`（product/vendor/serial/bus info/logical name/capacity/driver/driverversion/firmware） + sysfs `address/operstate/speed/duplex/wireless/device/uevent/device/driver/module/drivers/*`。**未使用**：`ethtool -i/-k/-a`、`iwconfig`/`iw dev`、`rfkill list`、`/proc/net/wireless`、`lspci -k` 网络类解析、`lsusb` USB 无线关联、`hwinfo --netcard`。接口过滤：`lo` + `docker/veth/br-/virbr/lxcbr/cni/flannel/tun/tap`。

**deepin**：`lshw -class network` + `hwinfo --netcard`（`Permanent HW Address`/`Module Alias`/`VID_PID`/`Hardware Class`/`SysFS Device Link`） + `/sys/class/net/<if>/phy80211` + `wireless` 目录 + `ethtool` ioctl（`SIOCETHTOOL`, `ETHTOOL_GWOL/SWOL`）用于 WoL + `correctCurrentLinkStatus` 运行时纠偏 + USB 无线通过 `Modalias contains "usb"` 走 `setVendorNameBylsusbLspci`。

**kylin**：NetworkManager DBus 主数据面 + `/proc/net/dev`（流量统计）；硬件校验仅回答"有无网卡 / 是否连接 / 主连接是否有线"。

### 9.2 输出字段对比

| 字段 | qurbrix | deepin | kylin |
|---|---|---|---|
| interface | ✓ | ✓ Logical Name | ✓ Interface (DBus) |
| MAC | ✓ | ✓ MAC + Permanent HW Address | ✓ HwAddress |
| vendor/model | ✓ (lshw) | ✓ (lshw + hwinfo) | ✗ |
| PCI 地址 / bus info | ✓ | ✓ | ✗ |
| USB VID:PID | ✗ | ✓ | ✗ |
| driver_version | ~（仅 lshw） | ✓ | ✗ |
| firmware | ✓ (lshw only) | ✓ | ✗ |
| link_speed / duplex / operstate | ✓ | ✓ | ✗ |
| link_capacity | ~（仅 sysfs 缺失时兜底） | ✓ 独立字段 | ✗ |
| **port 类型 (tp/fiber)** | ✗ | ✓ | ✗ |
| **自动协商 / broadcast / multicast** | ✗ | ✓ | ✗ |
| MTU | 已解析未落库 | ✗ | ✗ |
| IPv4/IPv6 | ✓ | ✓ (单字段 IP) | ✓ |
| 无线 SSID/freq/signal | ✗ | ✗ | ✓ (AccessPoints) |
| **Wake-on-LAN** | ✗ | ✓ | ✗ |
| IRQ / 内存地址 | ✗ | ✓ | ✗ |
| kernel modules | ✓ | ✓ Driver Modules | ✗ |
| **内核内置驱动判定 (kernel-in)** | ✗ | ✓ | ✗ |
| **modem/蜂窝** | ✗ | ✗ | ✓ NM_DEVICE_TYPE_MODEM (=8) |
| rfkill Wi-Fi 状态 | ✗（仅蓝牙用） | ✗ | ✗ |

### 9.3 完成度评估

| 维度 | vs deepin | vs kylin（硬件视角） |
|---|---|---|
| 采集源多样性 | ~43% | ~150% |
| 有线核心字段 (interface/MAC/driver/speed/duplex/operstate/MTU/IP/firmware/WoL) 8/11 | ~73% | — |
| 有线扩展字段 (capacity/auto-neg/broadcast/multicast/IRQ/memory/kernel-in) 1/9 | ~11% | — |
| 无线相关 (wireless flag/SSID/freq/signal/rfkill/USB VID) 1/4 | ~25% | ~50% |
| 综合 | **~45%** | ~110%（仅硬件档案视角） |

### 9.4 主要差距

1. **无 ethtool 集成**：`SIOCETHTOOL` ioctl 未使用；缺 WoL、`auto-negotiation`、`port` (TP/fibre/BNC/MII/AUI)、`advertised/supported link modes`、`pause params`
2. **MTU 已采未落库**：`IpLinkRecord.mtu` 解析但 `NetworkProbe` 未消费
3. **无 hwinfo --netcard**：`Permanent HW Address` vs current-assigned MAC 无法区分；USB 无线拿不到 modalias/VID:PID
4. **无线数据薄弱**：仅 `/sys/class/net/<if>/wireless` glob 布尔判定；缺 `iw dev` / `iwconfig` / `/proc/net/wireless` / SSID/BSSID/channel/frequency/signal/tx_power
5. **Wake-on-LAN / rfkill 空白**
6. **PCI/USB 富化仅走 lshw**：缺 `lspci -vmk -d <addr>` 与 USB 关联通道
7. **link 运行时纠偏缺失**（`correctCurrentLinkStatus` 类型逻辑）
8. **Modem/蜂窝无独立分类**：无 ModemManager DBus / `mmcli` 通道
9. **接口过滤过于激进**：`tun/tap` 直接过滤，看不到 VPN 接口
10. **驱动版本仅来自 lshw**：未走 `modinfo <driver>` / `ethtool -i`

---

## 10. USB

### 10.1 采集层对比

| 项 | qurbrix-hwinfo | deepin | kylin |
|---|---|---|---|
| 主源 | `lsusb` + `lsusb -v` + `/sys/bus/usb/devices/*` | `hwinfo --usb`（CmdTool） + `lshw` + `/proc/bus/input/devices` 反过滤 | `lsusb -v` 文本抓取 |
| 三源合并 | ✓（表 + `-v` 补 bInterfaceNumber/Class/SubClass/Protocol + sysfs 补 manufacturer/serial/speed/bMaxPower + 过滤 root hub） | ✓（hwinfo 单源 + Printer/mouse/keyboard/Wacom 过滤） | 只走 `Bus 0` 分段拼串 |
| USB 拓扑 / hub 树 | ✗ | ✓（`SysFS BusID` 父子归并） | ✗ |

### 10.2 输出字段对比

qurbrix `UsbInfo`：bus_number/device_number/vendor_id/product_id/class/subclass/protocol/manufacturer/product/serial/speed/max_power_ma + `BusInfo::Usb`。

deepin 拆到 DeviceImage/DeviceInput/DeviceOthers 等类，主字段：Model/Vendor/SysFS BusID/Speed/MaxPower/Hardware Class/Unique ID/VID_PID/Serial ID/Driver。

kylin：Usbnum/UsbVendor/UsbProduct/UsbBusinfo/UsbID/bcdUsb/UsbMaxpower。

### 10.3 完成度评估

| 维度 | qurbrix | deepin | kylin |
|---|---|---|---|
| 采集层 | 85% | 95% | 40% |
| 输出字段 | 80% | 90% | 35% |
| 综合 | **~82%** | 100%（基准） | ~40% |

### 10.4 主要差距

1. **无 `hwinfo --usb` 语义化字段**：Hardware Class、Module Alias、Driver Modules、SysFS 路径
2. **无 `bcdUSB/version` 字段**（从 sysfs 或 `lsusb -v` 抽）
3. **USB 拓扑（父 port、hub 树、`ports/portX/connect_type`）未采集**
4. **未采集 `Driver`/`Driver Modules`**：`UsbInfo` 无驱动字段
5. **未做打印机/摄像头/输入设备的 USB 反过滤合并**：跨探针可能重复统计

---

## 11. 输入设备（键鼠 / 触摸板 / 触摸屏）

### 11.1 采集层对比

**qurbrix**：`/proc/bus/input/devices`（主，抽 name/phys/uniq/handlers/EV/KEY/REL/ABS/PROP） + `/sys/class/input/event*` capabilities 兜底 + `hwinfo --keyboard/--mouse`（补 vendor/model/driver_modules）。分类：evdev 能力位判定 Keyboard/Mouse/Touchpad/Touchscreen/Tablet（5 类）。

**deepin**：`hwinfo --keyboard` + `hwinfo --mouse` + `lshw` + `/proc/bus/input/devices` + `bluetoothctl paired-devices`（蓝牙输入关联） + `addMouseKeyboardInfoMapInfo` 噪音过滤。

**kylin**：`/proc/bus/input/devices` EV bitmask 判定 keyboard(0x12003)/mouse(0x7)/touchpad(0xb)（3 类）+ 服务支持包 `hwinfo --keyboard/--mouse` 存文本。

### 11.2 输出字段对比

qurbrix `InputInfo`：input_kind/event_node/phys/uniq/handlers/bus_type/vendor_id/product_id/version（`InputKind` 枚举：Keyboard/Mouse/Touchpad/Touchscreen/Tablet/UnknownInput）+ 通用 Device.vendor/model/driver。

deepin `DeviceInput`：Model/Interface/BusInfo/Capabilities/MaximumPower/Speed/KeyToLshw/**WakeupID/BluetoothIsConnected**/m_keysToPairedDevice/m_supportInterfaces + 基类 SerialID/UniqueID/SysPath/Modalias/PID/VID/Driver/Enable/CanEnable/CanUninstall + `canWakeupMachine`/`wakeupPath`/`isWakeupMachine`。

kylin：product/address/description。

### 11.3 完成度评估

| 维度 | qurbrix | deepin | kylin |
|---|---|---|---|
| 采集层 | 75% | 100% | 30% |
| 输出字段 | **60%** | 100% | 20% |
| 类型细分 | 5 类 | 含蓝牙/USB/PS/2 | 3 类 |
| 综合 | **~68%** | 100%（基准） | ~25% |

### 11.4 主要差距

1. **接口类型显式字段缺失**（PS/2 / USB / Bluetooth / I2C）：deepin `m_Interface` 从 `SysFS ID` / `Hotplug` 派生
2. **MaximumPower / Speed 缺失**（HID 接口最大电流/速率）
3. **无 Wakeup 能力**：`/sys/.../power/wakeup` / `wakeup_count` / `wakeup_last_time_ms` 未采集
4. **蓝牙输入关联缺失**：不查 pair 状态（deepin `setInfoFromBluetoothctl`）
5. **SerialID / UniqueID / SysPath / Modalias**：未采（禁用/驱动定位关键字段）
6. **触摸屏多点触摸能力（`ABS_MT_*`）与压感字段**：未字段化

---

## 12. 打印机

### 12.1 采集层对比

**qurbrix**：`lpstat -a`（queue + accepting） + `lpstat -v`（device_uri） + `lpstat -l -p`（Description / Make and Model） + `lpstat -d`（默认打印机）。

**deepin**：libcups (`cupsGetDests2`) 直接链接 CUPS C API，遍历 `cups_dest_t.options` 拿到 `printer-info`/`device-uri`/`printer-state`/`printer-make-and-model` 等 30+ IPP 选项 + `DBusEnableInterface::enablePrinter` 启用/禁用。

**kylin**：源码内无实现。相关能力在 debian 补丁（`driver-manager-service/hardwarefinder/printerfinder`）中，未展开。

### 12.2 输出字段对比

qurbrix `PrinterInfo`：queue_name/accepting/device_uri/make_model/is_default。

deepin `DevicePrint`：Model/SerialNumber/InterfaceType（从 URI scheme 拆分）/URI/Status/**Shared**/MakeAndModel + 基类 Name/Vendor/Driver/Enable。

kylin：无。

### 12.3 完成度评估

| 维度 | qurbrix | deepin | kylin |
|---|---|---|---|
| 采集层 | 60% | 100% | 0% |
| 输出字段 | 55% | 90% | 0% |
| 依赖复杂度 | 低（只需 CUPS 客户端命令） | 高（-lcups） | — |
| 综合 | **~58%** | 100% | 0% |

### 12.4 主要差距

1. **无 `serial_number`/`model_number` 拆分**（deepin 从 `printer-info` 拆 vendor）
2. **无 Shared 状态、`printer-state-reasons`、`printer-location`、`printer-uri-supported`、`marker-supply-*`**（墨盒/耗材）
3. **无 InterfaceType**（`usb://`/`ip://`/`socket://` 前缀派生）
4. **无 `ippfind` / `avahi-browse _ipp._tcp` 发现层**
5. **无 `/etc/cups/printers.conf` fallback**

---

## 13. 电源 / 电池

### 13.1 采集层对比

**qurbrix**：`upower --dump`（主）+ `/sys/class/power_supply/BAT*`（fallback） + 从 sysfs 额外拿 `cycle_count` / `temp`。过滤 `line_power_*` 只保留电池。

**deepin**：`upower --dump` + `dmidecode -t 22`（SMBIOS Portable Battery：SBDS Chemistry/Manufacture Date/Serial Number/Version）；line_power 也被跳过。

**kylin**：`upower --dump`（`log-collection.csv`，供 service-support 打包）+ `sensors`/SMBus 风扇；补丁队列 `bateryinfofinder` 未展开。

### 13.2 输出字段对比

qurbrix `BatteryInfo`：power_type/vendor/model/serial/technology/state/capacity_percent/energy_full_wh/energy_design_wh/energy_now_wh/voltage_v/temperature_celsius/cycle_count/present（LGC/LG Chem 厂商归一化）。

deepin `DevicePower`：Model/Type/SerialNumber/ElectricType/MaxPower/Status/**Enabled/HotSwitch**/Capacity/Voltage/**Slot**/**DesignCapacity/DesignVoltage/SBDSChemistry/SBDSManufactureDate/SBDSSerialNumber/SBDSVersion**/Temp + 基类 Name/Vendor/Driver。

kylin：无结构化产物。

### 13.3 完成度评估

| 维度 | qurbrix | deepin | kylin |
|---|---|---|---|
| 采集层 | 80% | 100% | 25% |
| 输出字段 | 75% | 90% | 10% |
| 数据源冗余 | 双源（upower+sysfs） | 双源（upower+dmidecode） | 单源 |
| 综合 | **~78%** | 100% | ~15% |

### 13.4 主要差距

1. **未采集 dmidecode Type 22（Portable Battery）**：缺 `SBDS Chemistry/Version/Serial/ManufactureDate`、`Design Capacity/Voltage`、`Slot/Location`
2. **未采集 AC 线电源**：`line_power_*` 被过滤，缺"电源适配器"子类型（online 状态、ElectricType、MaxPower）
3. **`state` 未标准化枚举**：透传 upower 原文 `fully-charged` 等
4. **未采集 `charge_control_start/end_threshold`**（Linux 6.x 电池管理字段）
5. **sysfs fallback 只处理 `energy_*` μJ**：`charge_full` (μAh) 部分设备需乘 voltage 才能得 Wh，当前无此分支
6. **无 UPower DBus 信号订阅**：一次性 snapshot

---

## 14. 交叉建议与优先级

按 **投入产出比 (ROI)** 排序，可分三批落地：

### 14.1 快速修复（低成本高回报，1-3 天）

| 编号 | 差距 | 涉及模块 | 收益 |
|---|---|---|---|
| Q1 | 蓝牙 parser 恢复 HCI/LMP Version、Manufacturer、Class、Features 字段 | `crates/hw-parser/src/bluetooth.rs` | 蓝牙完成度 50%→85% |
| Q2 | 网络 MTU 落库（`IpLinkRecord.mtu` → `NetworkInfo.mtu`） | `crates/hw-probe/src/existing.rs:1352-1366` | 一行改动 |
| Q3 | 显示器 `edid_hex: Option<String>` 输出 | `crates/hw-model/src/properties.rs` | EDID 已抓取只需暴露 |
| Q4 | GPU `DriverInfo.version` 从 `/sys/module/<driver>/version` 或 `nvidia-smi --query-gpu=driver_version` 填充 | `crates/hw-probe/src/existing.rs:4530` | 消灭"始终为空"字段 |
| Q5 | 存储 `ufs_spec_version` 从 sysfs 或 hwinfo 回填 | `crates/hw-probe/src/existing.rs` | 消灭"始终为空"字段 |
| Q6 | `dmidecode -t 11` OEM Strings 采集 | 主板/BIOS 探针 | OEM 机型识别 |
| Q7 | 存储分区 & 挂载：`lsblk -O -J` 加 `MOUNTPOINT/FSTYPE/PARTUUID/LABEL` | `existing.rs:2872` | 主流工具基础特性 |

### 14.2 中期补齐（有独立特性，1-2 周）

| 编号 | 差距 | 涉及模块 | 收益 |
|---|---|---|---|
| M1 | ethtool 集成（WoL、port、auto-neg、link modes、pause） | 新增 `crates/hw-probe/src/ethtool.rs` | 网络完成度 45%→80% |
| M2 | `hwinfo --netcard` / `--usb` / `--display` / `--gfxcard` / `--sound` 采集通道 | 复用 `hwinfo-*.txt` fixture 模式 | 全线富化 |
| M3 | 无线数据：`iw dev` + `/proc/net/wireless` + `rfkill list` | 新增 wireless 子模块 | 无线可视化 |
| M4 | 内存运行态：`/proc/meminfo` 全字段 + `/proc/swaps` + `/sys/devices/system/node/*` NUMA | `MemoryRuntimeInfo` + `SwapInfo` + `NumaNodeInfo` | 系统监控视角 |
| M5 | GPU 温度/功耗：`nvidia-smi --query-gpu=temperature.gpu,power.draw,utilization.gpu,fan.speed,clocks.*` + AMD `/sys/class/hwmon/*/temp*_input` | GPU 探针 | 三方均缺，可差异化 |
| M6 | ATA SMART 属性表：`ata_smart_attributes.table` 循环解析 `Reallocated_Sector_Ct` / `Current_Pending_Sector` / `crc_error_count` | `crates/hw-parser/src/storage.rs` | 老化预警 |
| M7 | `dmidecode -t 22` Portable Battery 采集 | 电源探针 | 电池完成度 78%→92% |
| M8 | 输入 Wakeup / SerialID / Modalias 采集 | 输入探针 | deepin 对齐 |

### 14.3 长期规划（架构级投入，1-2 月）

| 编号 | 差距 | 涉及模块 | 收益 |
|---|---|---|---|
| L1 | DBus 兜底（org.deepin.dde.Display1 / org.freedesktop.NetworkManager / org.bluez / UPower） | 新增 `crates/hw-probe/src/dbus.rs` | 显示 Wayland 兜底 / 网络 / 蓝牙全线 |
| L2 | `vulkaninfo` 采集 + `vulkan_api_version` / `vulkan_driver_*` / `vulkan_device_type` 字段 | GPU 探针 + `GpuInfo` | 三方均缺，率先补齐 |
| L3 | 独立 `ChassisInfo` / `DeviceKind::Chassis` | `crates/hw-model` | 主板/机箱语义拆分 |
| L4 | ModemManager DBus / `mmcli` 蜂窝网络分类 | 网络探针新增子路径 | 覆盖 4G/5G 模组机型 |
| L5 | UVC descriptors / `libcamera-list-cameras` | 摄像头探针 | 新 IPU/CSI 平台 |
| L6 | TOML 机型级修正入口 | `crates/hw-collect` | OEM 兼容清单 |
| L7 | `/proc/device-tree/*` ARM/龙芯 SMBIOS 缺失回退 | 主板/BIOS 探针 | 国产平台兜底 |

---

## 15. 附：核心代码位置索引

| 零部件 | qurbrix 主入口 | 输出模型 |
|---|---|---|
| CPU | `crates/hw-probe/src/existing.rs:44-401`, `crates/hw-parser/src/cpu.rs` | `crates/hw-model/src/properties.rs:98-170` |
| 主板/BIOS | `crates/hw-probe/src/existing.rs:3947`, `4021`, `4057`, `4101`, `4178-4271` | `crates/hw-model/src/properties.rs:29-96` |
| 内存 | `crates/hw-probe/src/existing.rs:3064-3236`, `crates/hw-parser/src/dmi.rs:105-180` | `crates/hw-model/src/properties.rs:173-215` |
| 存储 | `crates/hw-probe/src/existing.rs:2867-2971`, `crates/hw-parser/src/storage.rs` | `crates/hw-model/src/properties.rs:216-245` |
| CD-ROM | `crates/hw-probe/src/cdrom.rs:25-93` | `crates/hw-model/src/properties.rs:404-409` |
| GPU | `crates/hw-probe/src/existing.rs:4397-4595`, `crates/hw-parser/src/gpu.rs` | `crates/hw-model/src/properties.rs:247-283` |
| Camera | `crates/hw-probe/src/camera.rs` | `crates/hw-model/src/properties.rs:370-374` |
| 显示器 | `crates/hw-probe/src/existing.rs:5925-6080`, `crates/hw-parser/src/monitor.rs` | `crates/hw-model/src/properties.rs:286-311` |
| 音频 | `crates/hw-probe/src/audio.rs:29-287`, `crates/hw-parser/src/audio.rs` | `crates/hw-model/src/properties.rs:326-333` |
| 蓝牙 | `crates/hw-probe/src/bluetooth.rs:23-227`, `crates/hw-parser/src/bluetooth.rs` | `crates/hw-model/src/properties.rs:335-343` |
| 网络 | `crates/hw-probe/src/existing.rs:1301-1591`, `crates/hw-parser/src/network.rs` | `crates/hw-model/src/properties.rs:314-324` |
| USB | `crates/hw-probe/src/usb.rs:23-141`, `crates/hw-parser/src/usb.rs:38-92` | `crates/hw-model/src/properties.rs:411-425` |
| 输入 | `crates/hw-probe/src/input.rs:26-306`, `crates/hw-parser/src/input.rs:46-92` | `crates/hw-model/src/properties.rs:346-368` |
| 打印机 | `crates/hw-probe/src/printer.rs:22-168`, `crates/hw-parser/src/printer.rs:21-94` | `crates/hw-model/src/properties.rs:394-401` |
| 电池 | `crates/hw-probe/src/battery.rs:23-173`, `crates/hw-parser/src/power.rs:20-70` | `crates/hw-model/src/properties.rs:377-392` |

---

*报告生成于 2026-07-11，由 9 个并行只读 agent 扫描 `qurbrix-hwinfo` / `ReferenceProject/deepin-devicemanager-6.0.67` / `ReferenceProject/kylin-os-manager-build-2.0.0-76update2` 三方代码库产出。*
