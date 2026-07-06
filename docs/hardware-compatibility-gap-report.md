# qurbrix-hwinfo Hardware Compatibility Gap Report

生成日期：2026-07-05

更新说明（2026-07-06）：本报告保留为初始差距基线。当前实现已经吸收部分当时缺口，最新状态以
`docs/hardware-compatibility-reference-audit.md` 为准。已完成的关键改进包括：
`lscpu` + `lshw -class processor` + `dmidecode -t 4` 多源合并、`/proc/cpuinfo`
`Hardware`/`Processor` fallback、`/proc/hardware` Kirin fallback、DMI 当前频率/count 修正、CPU vendor/arch 归一化，并已把 family/model/stepping/bogomips/virtualization 暴露到 `CpuInfo`；Memory 已在 `dmidecode -t memory` 不可用或解析为空时用 `lshw -class memory` 恢复 DIMM 级字段，再尝试 EDAC sysfs DIMM 节点，最后退回 `/proc/meminfo` 总量，空解析会产生 `source_empty` warning；System 已从 runtime 与 `dmidecode -t 1` 保留 hostname/OS/kernel/architecture 和 System Information 字段，并在 dmidecode 不可用时用 `/sys/class/dmi/id` 产品字段 fallback；BIOS/主板已在 `dmidecode -t 0,1,2,3` 不可用时读取 `/sys/class/dmi/id`，并从 `/sys/firmware/efi` 补 firmware type 和 Secure Boot state；PCI 已在 `lspci -nn -k` 不可用时读取 `/sys/bus/pci/devices/*` 基础 ID 字段和 kernel driver，GPU 已可消费其中 display-class 节点并保留 driver，还可用 `lshw -class display` 补人类可读 product/vendor/driver，并可从 `/sys/class/drm/*/device/mem_info_vram_total` 按 PCI 地址补 VRAM total；Network 已可从 `ip -j addr` 补 IPv4/IPv6，可用 sysfs 标记 wireless/ethernet capability 和 `network_type`，同时保留 PCI bus ID、driver modules，并可用 `lshw -class network` 补人类可读 product/vendor/capacity/driver/firmware；Storage 已可从 `/sys/block/*/device/uevent` 补 kernel driver 和可用 PCI controller identity，可从 `/sys/class/nvme/<controller>/device/uevent` 补 NVMe controller PCI identity，并可在 SATA/SCSI leaf uevent 只有 block driver 时向父级回溯补 PCI controller identity；可用 `lshw -class disk` 和 `hwinfo --disk` 补人类可读 vendor/model/serial/firmware/driver，并可从 `smartctl -a -j` 补 SMART health/temperature 和 NVMe power-on hours、power-cycle count、available spare/threshold、percentage used、data units read/written、media errors、error-log entries（含非零状态但 JSON 可解析的输出）；USB 已可用 `/sys/bus/usb/devices/*` enrich `lsusb` 成功路径，可用 `lsusb -v` 补首个 interface descriptor，并在 `lsusb` 不可用时读取 sysfs 基础 device 字段和 max power；Audio 已在 `/proc/asound/cards` 不可用时读取 `/sys/class/sound/card*` 基础声卡节点，并从 `/proc/asound/card*/codec#*` 与 sysfs 声卡节点补 codec/driver/modules/vendor/subsystem/PCI bus ID，还可用 `lshw -class multimedia` 和 `hwinfo --sound` 补人类可读 product/vendor/driver，用 `pactl list cards` 补 card profile；Bluetooth 已在 `hciconfig -a` 不可用时读取 `/sys/class/bluetooth/hci*` controller address 和 rfkill 字段，并可用 `lshw -class communication` 补控制器 vendor/model/driver；Input 已在 `/proc/bus/input/devices` 不可用时读取 `/sys/class/input/event*` 基础事件节点，并可用 procfs/sysfs evdev capability bitmask 分类 touchscreen/touchpad/tablet/mouse，还可用 `hwinfo --keyboard`/`--mouse` 按 `/dev/input/event*` 补输入设备 vendor/model/driver 或生成基础 keyboard/mouse 设备；Battery 已可在 UPower/sysfs 路径归一化 `LGC` 为 `LG Chem`，并可从 sysfs fallback 补电池温度；Camera 已在 `v4l2-ctl --list-devices` 不可用时读取 `/sys/class/video4linux/video*` 基础节点，从 video4linux sysfs 补 kernel driver 和 USB identity/speed，可用 `lshw -class multimedia` 按 `/dev/video*` 补人类可读 product/vendor/driver，并可用 `v4l2-ctl --device <node> --list-formats-ext` 补格式/分辨率 capability；Printer 已在 `lpstat -a` 不可用时使用 `lpstat -v` 恢复基础队列/URI，从 `lpstat -d` 标记默认队列，并从 `lpstat -l -p` 补 make/model 描述；CD-ROM 已可用 `/sys/class/block/sr*` enrich proc 成功路径，并可用可选 `lshw -class disk` `*-cdrom` 和 `hwinfo --cdrom` 记录补 vendor/model/serial/firmware/driver；在 `/proc/sys/dev/cdrom/info` 不可用时也会读取基础光驱节点和可用身份字段，sysfs 不可用时可由 `hwinfo --cdrom` 生成基础光驱设备。

补充（2026-07-06）：Monitor 已使用 `xrandr --query`、`xrandr --verbose`、`/sys/class/drm/*/edid` 和可选 `hwinfo --monitor`，在进程内解析 EDID vendor/product/week/year/size/preferred mode/gamma，并从物理尺寸计算 diagonal inches；同时从 xrandr mode 列表保留 max resolution，并用 `hwinfo --monitor` 补安全匹配显示器的 vendor/model/serial/size 或创建 hwinfo-only fallback 设备。重复 sysfs connector 中只有一个可读 EDID，或多个可读但只有一个 `status=connected` 时，已可使用该候选；剩余差距是多个可读且状态仍无法唯一判定的重复 connector 精确按 card 匹配。

## 1. Executive Summary

1. `qurbrix-hwinfo` 的硬件类别覆盖已经比较宽：CPU、PCI、USB、内存、BIOS/主板、GPU、显示器、存储、网络、音频、蓝牙、输入、摄像头、电池、打印机、光驱都在统一 probe orchestration 中注册；证据：`qurbrix-hwinfo/crates/hw-collect/src/collector.rs:21-39`。
2. 与 Deepin/Kylin 相比，最大差距不是“完全没有硬件类别”，而是“特殊硬件兼容规则、字段 fallback、厂商/型号归一化、非 x86 字段差异处理”不足。
3. CPU 曾是最明显短板：初始审计时只执行 `lscpu`，只解析 `Architecture`、`CPU(s)`、`Model name`、`Vendor ID`、`Core(s) per socket`、`Socket(s)` 六类字段；当前已改为多源合并并增加 `/proc/cpuinfo` fallback，详见最新审计文档。
4. Deepin 会融合 `lscpu`、`lshw_cpu`、`dmidecode -t 4`，并用 DMI 修正 core/thread、CPU name/vendor 空缺；证据：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceGenerator.cpp:173-259`。
5. Kylin 明确做了国产 CPU vendor 归一化和 `/proc/cpuinfo` 的 `Hardware` fallback；证据：`ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:704-745`，`ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py:220-228`。
6. 国产 CPU、ARM64、LoongArch 仍有 P1 风险：`qurbrix-hwinfo` 已有 `/proc/cpuinfo` parser、`Hardware`/`Processor` fallback、`/proc/hardware` Kirin fallback 和 locale 强制，但更广泛真机 fixture 仍待补齐。
7. Deepin 已有架构 alias 和架构分流，覆盖 `aarch64`、`sw_64`、`loongarch`、`loongarch64`；证据：`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/commonfunction.cpp:25-33`，`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/DeviceFactory.cpp:29-69`。
8. `qurbrix-hwinfo` 的错误、超时、命令缺失、权限不足处理比参考项目更结构化，值得保留；证据：`qurbrix-hwinfo/crates/hw-source/src/runner.rs:22-59`。
9. 优先补齐顺序建议：P1 国产 CPU/vendor/arch 真机 fixtures；P2 状态仍无法唯一判定的显示器重复 connector 按 card 匹配；P2 音频 SoC alias、USB/network 过滤和分类增强。

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
| CPU | `lscpu`、`lshw -class processor`、`dmidecode -t 4`、`/proc/cpuinfo`、`/proc/hardware` | 多源扫描；已补 `Hardware`/`Processor` fallback、Kirin fallback、DMI fallback、locale 强制、主要 vendor alias 和扩展 CPU 字段；仍缺更广泛真机 fixture | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`；`qurbrix-hwinfo/crates/hw-parser/src/cpu.rs` |
| CPU model | `CpuInfo` 有 name/vendor/arch/core/thread/socket/frequency/family/model/stepping/bogomips/virtualization/flags | model 已可承载并由 parser/probe 填充主要 CPU 字段 | `qurbrix-hwinfo/crates/hw-model/src/properties.rs:54-70` |
| PCI | `lspci -nn -k` | 能解析 class/vendor/device/driver/modules | `qurbrix-hwinfo/crates/hw-probe/src/pci.rs:22-83` |
| USB | `lsusb` + optional `lsusb -v` + `/sys/bus/usb/devices/*` enrichment/fallback | 能解析基础 USB 字段；`lsusb` 成功时可按 bus/dev 从 sysfs 补 class/subclass/protocol、manufacturer/serial/speed/max power，并从 `lsusb -v` 补首个 interface descriptor；无 `lsusb` 时可从 sysfs 读取 bus/dev、VID/PID、device class/subclass/protocol、manufacturer/product/serial/speed/max power；仍缺多 interface 结构化建模和跨类别 consumed dedup | `qurbrix-hwinfo/crates/hw-probe/src/usb.rs` |
| Memory | `dmidecode -t memory` + `lshw -class memory` fallback + EDAC sysfs DIMM fallback + `/proc/meminfo` fallback | 可识别 DIMM size/vendor/type/speed/slot/serial/part；DMI 不可用或解析为空时先尝试 lshw DIMM bank，再尝试 EDAC sysfs DIMM label/type/size，最后退回总量；空解析会产生 warning | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| BIOS / Motherboard | `dmidecode -t 0,1,2,3` + `/sys/class/dmi/id` fallback + `/sys/firmware/efi` enrichment | 可识别 BIOS vendor/version/date、firmware type、Secure Boot state 和 board manufacturer/product/serial | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| Storage | `lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV` + `/sys/block/*` fallback + `/sys/class/nvme/<controller>/device/uevent` + bounded SATA/SCSI parent `uevent` traversal + optional `lshw -class disk` + optional `hwinfo --disk` + optional `hdparm -i` + optional `smartctl -a -j` | 正常路径取 disk 并保留 WWN/firmware/kernel driver 和可用 PCI controller identity；fallback 路径补 node/vendor/model/serial/WWN/firmware/size/rotational/driver；NVMe namespace 可从 controller uevent 补 PCI bus identity；SATA/SCSI disk 可在 leaf uevent 只有 block driver 时向父级回溯补 PCI controller identity；lshw/hwinfo 可补人类可读 vendor/model/serial/firmware/driver；hdparm 可补 ATA/SATA model/firmware/serial；smartctl 可补 SMART health/temperature 和 NVMe 健康计数，且保留非零状态 JSON | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| GPU | `lspci -nn -k`，GPU parser；optional `lshw -class display`；optional DRM sysfs VRAM enrichment；optional `dmesg` PCI-addressed VRAM enrichment；optional Deepin `gpu-info` / Jingjia `/proc/gpuinfo_0` VRAM enrichment；optional `nvidia-smi` PCI-addressed VRAM enrichment；optional unique-NVIDIA `nvidia-settings -q VideoRam` enrichment；optional `glxinfo -B` single-GPU renderer enrichment；`lspci` 不可用时可消费 `/sys/bus/pci/devices/*` display-class 节点 | 能识别 PCI GPU 和 driver/modules；lshw 可按 PCI bus 补人类可读 product/vendor/driver；DRM sysfs、dmesg、Deepin/Jingjia vendor path、`nvidia-smi` 和唯一 NVIDIA GPU 场景的 `nvidia-settings` 可补 VRAM total；单显卡场景可从 glxinfo 补 OpenGL renderer/vendor/version；sysfs fallback 可保留基础 PCI ID/VID/PID/class/driver/modules，并可用 lshw/DRM/dmesg/gpu-info/NVIDIA/glxinfo 补人类可读 identity 和显存；仍缺多 GPU glxinfo 精确归属 | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| Monitor | `xrandr --query` + `xrandr --verbose` + `/sys/class/drm/*/edid` + optional `hwinfo --monitor` | 取 connector/resolution/max resolution，并从 EDID 补 vendor/product/week/year/size/preferred mode/gamma/diagonal inches；可用 `hwinfo --monitor` 补安全匹配显示器的 vendor/model/serial/size，或在 xrandr/sysfs 无设备时生成 hwinfo-only fallback；重复 sysfs connector 中只有一个可读 EDID，或多个可读但只有一个 `status=connected` 时可使用该候选；仍缺多个可读且 status 也无法唯一判定的重复 connector 精确按 card 匹配 | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| Network | `ip -j link` + `ip -j addr` + `/sys/class/net/*` enrichment/fallback + optional `lshw -class network` | 正常路径取 interface/MAC/operstate/IPv4/IPv6，并从 sysfs 补 speed/duplex、wireless/ethernet capability、`network_type`、uevent driver、driver modules 和 PCI bus ID；fallback 路径也补 sysfs 字段；可从 lshw 补 product/vendor/capacity-derived speed/driver version/firmware；过滤 loopback/常见虚拟网卡；仍缺 NM DBus enrichment | `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
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
| CPU 字段解析 | 解析 model/vendor/thread/bogomips/architecture/family/frequency/model/stepping/flags/virtualization | qurbrix 已补主要 CPU 字段解析和模型暴露 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:224-289` |
| Loongson CPU | 对 Loongson 避免被 lshw/dmidecode 覆盖型号 | 特殊 CPU name 保护规则 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:306-323`；`ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:365-390` |
| Phytium/ARM 频率 | 注释说明飞腾无法通过 lscpu 获取当前频率，使用 dmidecode Current Speed | ARM/国产 CPU 频率 fallback | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/DeviceCpu.cpp:384-390` |
| Monitor | 使用 `xrandr`/verbose/EDID 处理显示器信息 | qurbrix monitor 已吸收 query/verbose/sysfs EDID 路径、xrandr max mode、唯一可读或唯一 connected 重复 sysfs connector 和可选 `hwinfo --monitor` enrichment/fallback，剩余状态仍无法唯一判定的重复 connector 按 card 匹配 | `ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/GetInfoPool.cpp:109-110` |

Not Applicable：Deepin 的 GUI 文案加载进度、发行版专属 UI 结构不适合直接进入 `qurbrix-hwinfo`。qurbrix 应保留 CLI/library 输出模型，不引入 GUI service 依赖。

## 5. Kylin Reference Capability Map

| 类别 | Reference 行为 | 参考价值 | 证据 |
| --- | --- | --- | --- |
| CPU vendor 归一化 | 从 `lscpu` Model name 推断 Phytium/Huawei/Hygon/Zhaoxin/Loongson/Intel/D2000 | 国产 CPU alias 规则 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:704-745` |
| `/proc/cpuinfo` fallback | `model name` 不存在时使用 `Hardware` | ARM SoC/非 x86 必需 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py:220-228` |
| Kirin SoC | 读取 `/proc/hardware` 识别 HUAWEI Kirin 990/9006C | HiSilicon/Kirin SoC 兼容参考 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:62-71` |
| 厂商表 | 覆盖 CPU、GPU、显示器、整机、虚拟机、网卡、硬盘、声卡、摄像头、输入、电池、BIOS 等 vendor alias | qurbrix 可建立小型、可测、许可证无关的归一化表 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:409-565` |
| 国产 GPU alias | 包含 `JINGJIA`/`JJM`、`Wuhan Digital Engineering` 等 | GPU vendor 归一化参考 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:415-425` |
| Monitor EDID | 从 xrandr verbose 提取 EDID，用 `edid-decode` 解析 manufacturer/product/week/year/size/gamma/maxmode | qurbrix 已吸收 manufacturer/product/week/year/size/gamma/maxmode，并计算 diagonal inches | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:1339-1411` |
| Audio/SoC | 对 Loongson/HiSilicon 声卡 vendor 有 alias | 国产平台外设展示增强 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:479-483` |
| VirtualBox | 识别 `INNOTEK`/`VBOX`/`VIRTUALBOX` | 虚拟机设备识别和标注参考 | `ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py:452-455` |

Not Applicable：Kylin 代码中有大量 `/tmp/youker-assistant-*` 临时文件和发行版服务流程；这不适合 qurbrix 的 library/CLI 架构。可借鉴数据源和规则，不建议引入临时文件协议。

## 6. CPU Special Handling Gap

| CPU/架构 | Reference 处理 | qurbrix-hwinfo 当前处理 | 缺口 | 建议 | 证据 |
| --- | --- | --- | --- | --- | --- |
| 通用 x86_64 | Deepin 解析 model/vendor/family/model/stepping/frequency/flags/virtualization；Kylin 用 lscpu 计算 core | 已扩展 lscpu/procfs/DMI 主要字段，并暴露 family/model/stepping/bogomips/virtualization | 更广泛真机 fixture 仍不足 | 后续补充真实机器样本覆盖 | Deepin: `.../DeviceCpu.cpp:224-289`；qurbrix: `qurbrix-hwinfo/crates/hw-parser/src/cpu.rs` |
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
| LoongArch | Deepin arch alias 覆盖 `loongarch`/`loongarch64` | qurbrix 已归一化 `loongarch`/`loongarch64` | 真机样本仍不足 | 继续补 Loongson fixtures | Deepin: `.../commonfunction.cpp:30-32` |

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
| 申威 | Deepin arch alias 包含 `sw_64` | qurbrix 已归一化 `sw_64` | 真机样本仍不足 | 补 sw_64 fixture | Deepin: `.../commonfunction.cpp:30-31` |
| 国产 GPU | Kylin vendor 表包含 Jingjia/JJM/Wuhan Digital Engineering | qurbrix 已有 GPU vendor alias、PCI ID fallback，并可从 `lshw -class display` 补人类可读 product/vendor，从 DRM sysfs、dmesg、Deepin `gpu-info`、唯一景嘉微 `/proc/gpuinfo_0`、`nvidia-smi` 或唯一 NVIDIA GPU 场景的 `nvidia-settings` 补 VRAM，单显卡时可从 `glxinfo -B` 补 renderer/vendor/version | 仍缺更多真机样本和多 GPU glxinfo 精确归属 | Kylin: `.../cpuinfo.py:415-425`；Deepin: `.../DeviceGpu.cpp:196-252`；qurbrix: `qurbrix-hwinfo/crates/hw-probe/src/existing.rs` |
| 国产声卡/SoC 音频 | Kylin 表包含 Loongson/HiSilicon 声卡 vendor | qurbrix 已补通用 sysfs PCI vendor ID 归一化，但 SoC 音频 alias 仍弱 | audio parser 继续扩展 SoC vendor alias | Kylin: `.../cpuinfo.py:479-483` |

## 8. Hardware Data Source Gap

| 数据源 | Deepin 使用情况 | Kylin 使用情况 | qurbrix-hwinfo 使用情况 | 差距 | 建议 |
| --- | --- | --- | --- | --- | --- |
| `/proc/cpuinfo` | 间接经 lscpu/lshw，另有 `/proc/boardinfo` | 直接读取，支持 `Hardware` fallback | CPU probe 已作为 optional procfs fallback 使用 | 已吸收 | 继续补更多国产 CPU fixtures |
| `/proc/meminfo` | 非核心证据 | 有注释/部分系统信息逻辑 | memory probe 已在 dmidecode 失败时读取 MemTotal 总量 | 已吸收部分 | 后续如需 DIMM 级信息仍需 DMI/lshw/sysfs |
| `/sys/class/dmi/id` | 主要走 dmidecode | 可作为系统信息来源 | System、BIOS、board、chassis probe 已作为 dmidecode fallback 使用，Physical Memory Array 通过 `dmidecode -t 16` 补齐，BIOS Language Information 通过可选 `dmidecode -t 13` 补齐 | 已吸收 | 后续可扩更细 DMI/SMBIOS 扩展字段 |
| `/sys/firmware/efi` | 固件/启动状态类系统信息 | 系统硬件常用补充来源 | BIOS probe 已读取 UEFI/BIOS firmware type，并从 SecureBoot efivar 补 Secure Boot state | 已吸收部分 | 后续可补更多 EFI/TPM 启动安全字段 |
| `/sys/class/drm` | 参考项目使用 xrandr/EDID 类能力 | 通过 xrandr verbose/EDID | 已用于 monitor sysfs EDID fallback 和 GPU DRM VRAM enrichment；monitor 已可使用唯一可读或唯一 connected 重复 connector 的 sysfs EDID | 已吸收部分 | 后续补状态仍无法唯一判定的重复 connector 按 card 匹配和更多 DRM 显示能力 |
| `/sys/class/net` | Deepin 有网络 sysfs/MAC 过滤逻辑 | 结合 lshw/lspci/driver | Network probe 已读取 MAC/operstate/speed/duplex、wireless/ethernet capability、`network_type`、uevent driver、PCI bus ID 和 driver modules，并在 `ip` 失败时作为 fallback；`ip -j addr` 已补 IPv4/IPv6；`lshw -class network` 已补 product/vendor/capacity/driver version/firmware | 已吸收部分 | 后续补 NM DBus enrichment |
| `/sys/class/power_supply` | Deepin 使用 upower/dmesg 类电源源 | Kylin 有电源厂商 alias | Battery probe 已在 UPower 失败时读取 BAT* sysfs 字段，保留温度，并对 `LGC` 做轻量厂商归一化 | 已吸收部分 | 后续可补更多厂商 alias |
| `/sys/block` | Deepin 用 lsblk/sg，并对 NVMe SysFS link 做匹配 | Kylin 磁盘逻辑复杂 | Storage probe 已读取 vendor/model/serial/WWN/firmware/size/rotational，并从 uevent 补 kernel driver 和可用 PCI controller identity；可从 `/sys/class/nvme/<controller>/device/uevent` 补 NVMe controller PCI identity；SATA/SCSI disk leaf uevent 只有 block driver 时可有限向父级回溯补 PCI controller identity；可选 `lshw -class disk`、`hwinfo --disk` 和 `hdparm -i` 补人类可读 vendor/model/serial/firmware/driver 或 ATA identity；可选 `smartctl -a -j` 补 SMART health/temperature 和 NVMe 健康计数 | 已吸收部分 | 后续补更丰富 controller model/vendor 命名和更完整 SATA/SCSI sysfs parent 拓扑 |
| `/sys/bus/pci` | 参考项目重视 PCI/driver | lspci/lshw/driver | `lspci -nn -k`；无 lspci 时读 sysfs vendor/device/class/subsystem/driver/modules，GPU 消费 display-class 节点并可用 `lshw -class display` 补人类可读名称 | 已吸收部分 | 后续补跨类别消费和更多类别的人类可读名称 |
| `/sys/bus/usb` | Deepin USB 过滤/去重 | Kylin `lsusb -v` | USB probe 已用 sysfs enrich `lsusb` 成功路径，用 `lsusb -v` 补首个 interface descriptor，并在 `lsusb` 失败时读取基础 sysfs device 字段和 max power | 已吸收部分 | 后续补多 interface 结构化建模、跨类别 dedup |
| `lscpu` | CPU 主来源之一 | CPU 主来源之一 | CPU primary source，另有 lshw/DMI/procfs fallback | 已吸收主要 fallback，并强制英文 locale | 继续补真机样本 |
| `lspci` | PCI/GPU/driver 来源 | 网络/GPU/声卡等来源 | PCI/GPU 来源 | P2 | 继续使用，补分类和 alias |
| `lsusb` | USB 来源并带过滤/去重 | 使用更详细输出 | USB primary 基础来源，缺失时有 sysfs fallback | P2 | 加 `lsusb -v` optional source |
| `dmidecode` | BIOS/board/memory/CPU 修正 | 系统硬件常用来源 | CPU、BIOS/board/memory 来源；BIOS/board 权限不足走 sysfs；memory 权限不足先走 `lshw -class memory` DIMM fallback，再走 EDAC sysfs DIMM fallback，最后走 `/proc/meminfo` 总量 fallback | 已吸收部分 | memory 仍可补 SPD/eeprom 级字段 |
| `lshw` | CPU/audio/network/display 等多类 fallback | 网络/硬件详情 | CPU 和 Memory 已作为 optional fallback 使用；Network、Audio、Storage、GPU、CD-ROM 已作为 optional enrichment 使用 | 已吸收部分 | 可选，不作为硬依赖，后续补 camera/bluetooth 等轻量 enrichment |
| `hwinfo` | monitor/general 来源 | 依赖或脚本中存在 | 已作为 monitor/storage/audio/input/CD-ROM 的可选 enrichment/fallback 来源 | 已吸收部分 | 保持可选，不作为硬依赖 |
| `udevadm` | 搜索范围内需关注 | 参考项目可通过 udev 类源 | 未见核心使用 | P2 | 可用于 USB/PCI/input 属性补充 |
| `xrandr` | query/verbose | verbose + EDID | 已使用 `xrandr --query` 和 `xrandr --verbose`，并解析 verbose EDID 和 query mode 列表 max resolution | 已吸收部分 | 继续保留 optional source 失败不影响基础显示器 |
| `glxinfo` | GPU 展示增强类 | GPU 可能使用 | 已作为单显卡 optional source 解析 OpenGL renderer/vendor/version | 已吸收部分 | 多 GPU 时仍需 renderer 到 PCI 设备的精确归属策略 |

## 9. Device Category Gap Matrix

| 类别 | Reference 行为 | qurbrix-hwinfo 当前状态 | 差距 | 严重程度 | 是否建议实现 | 证据 |
| --- | --- | --- | --- | --- | --- | --- |
| CPU | 多源合并、DMI fallback、国产 vendor/arch 处理 | 已实现多源合并、`/proc/cpuinfo` fallback、`/proc/hardware` Kirin fallback 和 locale 强制 | 真机样本覆盖仍不足 | P1/P2 | 是 | Deepin `.../DeviceGenerator.cpp:173-259`；qurbrix `crates/hw-probe/src/existing.rs` |
| 主板/BIOS/DMI | 多个 dmidecode type | `SystemProbe` 已读取 runtime、`dmidecode -t 1` 和 sysfs DMI 产品字段；BIOS/board/chassis 使用 dmidecode `0,1,2,3` + `/sys/class/dmi/id` fallback，Physical Memory Array 使用 `dmidecode -t 16` 补 location/use/error-correction/maximum-capacity/error-handle/device-count，BIOS Language Information 使用可选 `dmidecode -t 13` 补 language description format/installable languages/currently installed language，并从 `/sys/firmware/efi` 补 firmware type / Secure Boot state | 当前六类范围内暂无明确 Deepin/Kylin 主板/BIOS/DMI 字段缺口 | P3 | 已实现 | qurbrix `crates/hw-probe/src/existing.rs` |
| 内存 | dmidecode type 17 等 | dmidecode memory，dmidecode 失败时用 `lshw -class memory` 产出 DIMM fallback，再用 EDAC sysfs 产出 DIMM label/type/size fallback，最后用 `/proc/meminfo` 产出总量 fallback | 缺 SPD/eeprom 级厂商/序列号/时序字段 fallback | P2 | 是 | qurbrix `crates/hw-probe/src/existing.rs` |
| 硬盘/分区/控制器 | lsblk/sg/lshw/厂商修正 | lsblk disk，失败时 fallback 到 `/sys/block/*` 并保留 vendor/model/serial/WWN/firmware/size/rotational/driver 和可用 PCI controller identity；NVMe namespace 可从 controller sysfs uevent 补 PCI bus identity；SATA/SCSI disk 可有限向父级 uevent 回溯补 PCI controller identity；可选 `lshw -class disk`、`hwinfo --disk` 和 `hdparm -i` 补人类可读 vendor/model/serial/firmware/driver 或 ATA identity；可选 smartctl 补 SMART health/temperature 和 NVMe 健康计数并处理非零状态 JSON | 缺更丰富 controller model/vendor 命名和更完整 SATA/SCSI 拓扑覆盖 | P2 | 是 | qurbrix `crates/hw-probe/src/existing.rs` |
| 显卡/显示控制器 | lspci、lshw display、dmesg、gpu-info、glxinfo、nvidia、国产 GPU alias | lspci GPU/driver/modules；可选 `lshw -class display` 按 PCI bus 补人类可读 product/vendor/driver；可选 DRM sysfs、`dmesg`、Deepin/Jingjia vendor path、`nvidia-smi` 和唯一 NVIDIA GPU 场景的 `nvidia-settings` 补 VRAM total；单显卡时可选 `glxinfo -B` 补 OpenGL renderer/vendor/version；lspci 不可用时从 sysfs PCI display-class 生成基础 GPU 并消费对应 PCI，保留 kernel driver/modules | 缺多 GPU glxinfo 精确归属 | P3 | 是 | Kylin `.../cpuinfo.py:415-425`；Deepin `.../commontools.cpp:333-351`；Deepin `.../DeviceGpu.cpp:196-252`；qurbrix `crates/hw-probe/src/existing.rs` |
| 显示器 | xrandr verbose/EDID/product/year/size/gamma/maxmode，service-support 使用 `hwinfo --monitor` | xrandr query/verbose + sysfs EDID + optional `hwinfo --monitor`，已补 vendor/product/week/year/size/preferred mode/gamma/diagonal inches/max resolution，hwinfo 可补 vendor/model/serial/size 或生成 fallback 设备，重复 sysfs connector 中只有一个可读 EDID，或多个可读但只有一个 `status=connected` 时可使用该候选 | 缺多个可读且 status 也无法唯一判定的重复 connector 精确按 card 匹配 | P2 | 是 | Kylin `.../cpuinfo.py:1339-1411`；Kylin `hardware-capability-scan-report.md:135`；qurbrix `crates/hw-probe/src/existing.rs` |
| 网卡/Wi-Fi/蓝牙 | sysfs/MAC/filter、lshw/lspci/driver | network 使用 `ip -j link`/`ip -j addr` 并用 `/sys/class/net/*` enrich speed/duplex/wireless/ethernet capability/`network_type`/driver/modules/PCI bus ID，失败时 fallback 到 sysfs；`lshw -class network` 可补 product/vendor/capacity/driver version/firmware；bluetooth 使用 `hciconfig -a`，可用 `lshw -class communication` 补 vendor/model/driver，失败时 fallback 到 `/sys/class/bluetooth/hci*` controller address/rfkill 字段 | Network 仍缺 NM DBus enrichment；Bluetooth 仍缺 hwinfo/BlueZ DBus enrichment 和 sysfs paired-device fallback | P1/P2 | 是 | qurbrix `crates/hw-probe/src/existing.rs`；`crates/hw-probe/src/bluetooth.rs` |
| 声卡 | lshw、`/proc/asound`、SoC vendor | qurbrix 使用 `/proc/asound/cards`，可从 `/proc/asound/card*/codec#*` 和 `/sys/class/sound/card*/device` 补 codec/driver/modules/vendor/subsystem/PCI bus ID，proc cards 缺失时 fallback 到 sysfs card，可从 `lshw -class multimedia` 和 `hwinfo --sound` 补人类可读 product/vendor/driver，并可从 `pactl list cards` 补 ALSA card profiles | 仍缺更广 SoC alias | P2 | 是 | Deepin `.../GetInfoPool.cpp:88,119`；Kylin `.../cpuinfo.py:479-483` |
| USB 设备 | 过滤 hub/重复/无效设备，详细描述符 | `lsusb` 基础字段；成功路径和 fallback 均可读 sysfs 基础字段和 max power；`lsusb -v` 可补首个 interface descriptor；并过滤 hub/host controller/interface entries | 无多 interface 结构化建模、跨类别 consumed dedup | P2 | 是 | qurbrix `crates/hw-probe/src/usb.rs` |
| PCI 设备 | PCI 分类、驱动识别 | lspci class/vendor/device/driver/modules；lspci 缺失时 sysfs fallback 保留 IDs、driver 和 modules | 分类消费只对部分类别；alias 和人类可读名称不足 | P2 | 是 | qurbrix `crates/hw-probe/src/pci.rs:22-83` |
| 摄像头 | USB/vendor 表、video source | qurbrix 使用 `v4l2-ctl --list-devices`，失败时 fallback 到 `/sys/class/video4linux/video*` 基础 name/node/driver，可从 video4linux sysfs 链接补 USB VID/PID、manufacturer/product/serial、bus/interface 和 speed 字段，可从 `lshw -class multimedia` 按 `/dev/video*` 补人类可读 product/vendor/driver，并可从 `v4l2-ctl --list-formats-ext` 补格式/分辨率 capability | 厂商 alias、hwinfo enrichment 仍不足 | P2 | 是 | Kylin `.../cpuinfo.py:484-491`；qurbrix `crates/hw-probe/src/camera.rs` |
| 打印机/扫描仪 | printer source | qurbrix 有 printer fixtures，CUPS 队列可补 URI、默认队列和 `lpstat -l -p` make/model 描述 | 扫描仪未明确；打印机详细 state/interface 仍不足 | P3 | 部分 | Deepin `.../GetInfoPool.cpp:89` |
| 电池/电源 | upower/dmesg/vendor alias | qurbrix 有 UPower 和 `/sys/class/power_supply/BAT*` fallback fixtures，保留 sysfs 温度，并归一化常见 `LGC` alias | 更多厂商 alias 待增强 | P2 | 是 | Deepin `.../GetInfoPool.cpp:105,111`；Kylin `.../cpuinfo.py:524-525` |
| 输入设备 | `/proc/bus/input/devices` + `/sys/class/input/event*` fallback + `hwinfo --keyboard`/`--mouse` 可选 enrichment/fallback | qurbrix 有 input fixtures，并可从 procfs/sysfs evdev capability bitmask 补 touchscreen/touchpad/tablet/mouse 分类；可按 `/dev/input/event*` 从 hwinfo 补 keyboard/mouse vendor/model/driver 或生成基础设备 | vendor alias、lshw enrichment 和更细 bus-specific 分类仍可增强 | P2 | 是 | Deepin `.../GetInfoPool.cpp:118`；Kylin `.../cpuinfo.py:492-503` |
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
| 目标 | 显示器 EDID/DRM/sysfs/max resolution/`hwinfo --monitor` 已吸收，剩余状态仍无法唯一判定的重复 connector 按 card 匹配 |
| 涉及模块 | `MonitorProbe`、monitor parser、fixtures |
| 推荐实现方式 | 已用 `xrandr --query` 保留 max resolution，用 `xrandr --verbose` 和 `/sys/class/drm/*/edid` optional source 解析 manufacturer/product/week/year/size/preferred mode/gamma，并计算 diagonal inches；已用可选 `hwinfo --monitor` 补安全匹配显示器的 vendor/model/serial/size 或生成 fallback 设备 |
| 不建议照抄 reference 的原因 | Kylin 写 `/tmp/edid.dat` 并调用 `edid-decode`，不适合 library 并发和无副作用要求 |
| 建议测试 | 内屏无 EDID name、外接显示器 EDID、headless xrandr 失败、gamma/diagonal inches、hwinfo 单屏和唯一分辨率匹配、重复 sysfs connector |
| 验收标准 | 有 EDID 时 vendor/product/size/date/gamma/diagonal inches 填充；无 EDID 不影响 resolution/max resolution；有可安全匹配 hwinfo monitor 时补 vendor/model/serial/size；xrandr/sysfs 无设备但 hwinfo 可用时仍输出显示器 |

### P2

| 项 | 内容 |
| --- | --- |
| 目标 | 网络、USB、存储、DMI 的 fallback 和过滤 |
| 涉及模块 | `NetworkProbe`、`UsbProbe`、`StorageProbe`、`BiosProbe`、parser tests |
| 推荐实现方式 | network 已加 `/sys/class/net` speed/duplex/wireless/ethernet capability/`network_type`/driver/modules/PCI bus ID enrichment、`ip -j addr` 地址补齐和 `lshw -class network` 人类可读 enrichment，后续补 NM DBus；USB 已补 `/sys/bus/usb/devices/*` success-path enrichment/fallback、max power 和 `lsusb -v` 首个 interface descriptor，后续补多 interface 建模；DMI 已加 `/sys/class/dmi/id` fallback 和 `/sys/firmware/efi` firmware type/Secure Boot enrichment；memory 已补 `lshw -class memory` DIMM fallback 和 EDAC sysfs DIMM fallback；storage 已补 `/sys/block` rotational/WWN/firmware/vendor/driver、NVMe controller PCI identity、有限 SATA/SCSI parent uevent PCI identity、`lshw -class disk` 人类可读 identity 和 smartctl SMART/temp/NVMe 健康计数 |
| 不建议照抄 reference 的原因 | 参考项目大量展示逻辑、发行版服务和临时文件协议不适合 qurbrix |
| 建议测试 | virtual NIC、USB hub、空 serial、dmidecode permission denied |
| 验收标准 | 关键字段补齐，虚拟/无效设备不会污染核心类别 |

### P3

| 项 | 内容 |
| --- | --- |
| 目标 | 展示增强和可选 heavy source |
| 涉及模块 | output views、optional source configuration |
| 推荐实现方式 | `glxinfo` renderer 已作为单显卡 optional source，Deepin `gpu-info`、景嘉微 `/proc/gpuinfo_0`、`nvidia-smi` 和唯一 NVIDIA GPU 场景的 `nvidia-settings` 已作为轻量 optional source；后续剩余 GPU 展示增强继续保持可选，不作为默认硬依赖 |
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
* `qurbrix-hwinfo/crates/hw-probe/src/usb.rs`：证明 USB 当前使用基础 `lsusb`，并用 `/sys/bus/usb/devices/*` 做成功路径 enrichment 和 fallback。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`：证明 storage 当前用 `lsblk`，可选 `lshw -class disk`/`smartctl` enrichment，并在 `lsblk` 失败时 fallback 到 `/sys/block/*`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:168-229`：证明 memory 当前依赖 `dmidecode -t memory`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs:242-295`：证明 BIOS/board 当前依赖 `dmidecode -t 0,1,2,3`。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`：证明 GPU 当前基于 `lspci -nn -k`，并可用 `lshw -class display` 和 sysfs PCI display-class fallback。
* `qurbrix-hwinfo/crates/hw-probe/src/existing.rs`：证明 monitor 当前使用 `xrandr --query`、`xrandr --verbose`、`/sys/class/drm/*/edid` 和可选 `hwinfo --monitor`，并保留 EDID/hwinfo 字段。
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
