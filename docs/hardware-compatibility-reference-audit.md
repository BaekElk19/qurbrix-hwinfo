# Hardware Compatibility Reference Audit

Date: 2026-07-06

This audit compares the current qurbrix-hwinfo implementation with:

- `../ReferenceProject/deepin-devicemanager-6.0.67`
- `../ReferenceProject/kylin-os-manager-build-2.0.0-76update2`

Scope: verify each component parser/probe against the two references, with special attention to fallback and exception-handling practices.

## Summary

Current qurbrix-hwinfo now absorbs the P0/P1 compatibility work that was previously missing:

- CPU uses `lscpu`, `lshw -class processor`, and `dmidecode -t 4` as optional sources, merges DMI counts/speeds, protects Loongson names, and normalizes CPU vendors/architectures.
- Monitor uses `xrandr --query`, `xrandr --verbose`, and `/sys/class/drm/*/edid`, parses EDID in-process, and preserves monitor devices when EDID parsing fails.
- GPU uses `lspci -nn -k`, normalizes common and domestic GPU vendor names, and now falls back to PCI vendor IDs when lspci text is generic.
- Source execution has structured `Missing`, `PermissionDenied`, `Timeout`, and `Failed` classifications, which is cleaner than most reference scripts and should remain the project-wide pattern.

Confirmed defects fixed during this audit:

- `CPU MHz` from `lscpu` is now used as `current_freq_mhz` when DMI current speed is unavailable.
- Monitor sysfs fallback now works when `xrandr --query` is missing, but does not create fake devices from empty/bad sysfs EDID.
- sysfs EDID read failures now produce source warnings instead of being silently ignored.
- GPU vendor normalization now uses numeric PCI vendor IDs when text is only generic `Device`.
- Loopback/common virtual network interfaces, UPower line-power devices, USB root hubs/hubs, empty BIOS DMI output, and duplicate UVC camera video nodes are now filtered.

## Component Matrix

| Component | Current qurbrix implementation | Absorbed from references | Remaining gaps | Priority |
| --- | --- | --- | --- | --- |
| CPU | `CpuProbe` runs `lscpu`, `lshw -class processor`, `dmidecode -t 4`; parser reads extended lscpu and DMI type 4; merge handles DMI counts/speeds and Loongson protection. | Deepin CPU three-source merge, DMI count correction, Loongson/lshw protection, Phytium DMI current-speed fallback. Kylin-style CPU vendor alias is partially absorbed. | No `/proc/cpuinfo` `Hardware`/`Processor` fallback; no `/proc/hardware` Kirin fallback; `lscpu` locale is not forced to English; parsed family/model/stepping/bogomips/virtualization are not exposed in `CpuInfo`. | P0/P1/P2 |
| Architecture/vendor normalization | `normalize_arch`, `normalize_cpu_vendor_id`, `infer_cpu_vendor_from_name`, `normalize_gpu_vendor`, `normalize_gpu_vendor_id`. | Deepin arch aliases for amd64/arm64/sw_64/loongarch; Kylin domestic CPU/GPU vendor heuristics. | Huawei/Kunpeng/HiSilicon canonical choice intentionally differs from Kylin; alias tables remain intentionally small. | P1 |
| Monitor | `xrandr --query` for connector/resolution, `xrandr --verbose` and sysfs EDID for identity; EDID parser fills manufacturer/product/date/size/preferred mode. | Kylin xrandr verbose EDID extraction; Deepin sysfs DRM EDID parsing pattern; no `/tmp` temp file or external `edid-decode`. | No gamma/inch/maxmode fields; no `hwinfo_monitor`; ambiguous duplicate sysfs connectors are skipped rather than matched by card. | P2 |
| GPU | `lspci -nn -k` display/VGA/3D records, driver/modules, text and numeric vendor normalization. | PCI GPU parsing and domestic GPU aliases from Kylin; driver extraction from lspci. | No `lshw -C display`, `glxinfo -B`, `/sys/class/drm/card*/device`, dmesg/nvidia memory enrichment. | P2/P3 |
| Memory | `dmidecode -t memory`, DIMM size/vendor/type/speed/slot/serial/part, filters `No Module Installed`. | Main DMI memory path from both references. | No `lshw_memory`, `/proc/meminfo`, or sysfs fallback when DMI is unavailable/permission denied. | P2 |
| BIOS / motherboard / DMI | `dmidecode -t 0,1,2,3`, BIOS vendor/version/date and baseboard manufacturer/product/serial; empty DMI output produces `source_empty` rather than generic devices. | Core DMI BIOS/baseboard parsing and Deepin-style skip-empty behavior. | No `/sys/class/dmi/id` fallback; chassis/system/language/memory-array data not modeled. | P2 |
| Storage | `lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN`, disk-only filtering. | Basic lsblk disk enumeration. | No lshw/hwinfo/sysfs/hdparm/smartctl fusion; no WWN/firmware/SMART/temp/controller/driver enrichment; malformed JSON currently becomes empty output without warning. | P2 |
| Network | `ip -j link`, interface/MAC/operstate; filters loopback and common virtual interfaces. | Basic network interface enumeration plus Kylin-style avoidance of non-physical interfaces. | No lshw/lspci/sysfs/NM DBus fallback; no driver/wireless/type/speed/duplex/IP enrichment; malformed JSON becomes empty output without warning. | P1 |
| Audio | `/proc/asound/cards`, card index/name. | Deepin/Kylin use `/proc/asound` and multimedia sources; base source absorbed. | No PCI/lshw/sysfs/codec fallback; no driver/vendor/codec/subsystem. | P1/P2 |
| Bluetooth | `hciconfig -a`, optional `bluetoothctl paired-devices`. | Deepin `hciconfig` path and lightweight paired-device enrichment. | `hciconfig` is a hard dependency; no lshw/sysfs/DBus fallback; paired-devices failure is silent. | P1/P2 |
| Input | `/proc/bus/input/devices`, handlers/IDs, keyboard/mouse/touchpad/touchscreen classification. | Proc input parsing and basic classification. | No lshw/hwinfo enrichment; no EV bitmask classification; `Tablet` remains unused; limited bus-specific classification. | P2 |
| Camera | `v4l2-ctl --list-devices`, emits one device per physical camera record using the first `/dev/video*` node. | Basic video device discovery and Deepin-style physical-device deduplication. | No sysfs/lshw/hwinfo fallback; no vendor/driver/speed/serial. | P2 |
| Battery | `upower --dump`, battery capacity/energy/voltage/vendor/model/serial; filters line-power devices. | UPower-based collection and Deepin-style line-power filtering. | No sysfs cycle/temp fallback. | P2 |
| Printer | `lpstat -a`, optional `lpstat -v`. | CUPS queue enumeration. | No make/model/default/state/interface; `lpstat -v` failure is silent. | P2 |
| CD-ROM | `/proc/sys/dev/cdrom/info`, drive names and basic capabilities. | Proc cdrom discovery. | No lshw/hwinfo/lsscsi fallback; no vendor/model/firmware/serial. | P2 |
| USB | `lsusb`, bus/device/VID/PID/product; filters root hubs and USB hubs. | Basic USB enumeration plus Deepin/Kylin hub filtering. | No `lsusb -v`; no interface/class/speed/maxpower/serial enrichment; USB devices consumed by Bluetooth/camera/input/printer are not deduplicated. | P2 |
| PCI / Other PCI | `lspci -nn -k`, class/vendor/device IDs, driver/modules; unconsumed PCI devices become `OtherPci`. | PCI class and driver extraction. | Only GPU consumes backing PCI; network/audio/storage/camera/bluetooth may duplicate as `OtherPci`; no sysfs fallback. | P1/P2 |

## Exception Handling Audit

Absorbed and preserved:

- `hw-source` classifies command/file errors as `Missing`, `PermissionDenied`, `Timeout`, or `Failed`.
- CPU treats `lscpu`, `lshw`, and `dmidecode` as optional and emits warnings for failed sources while still producing a CPU when any useful source exists.
- Monitor treats `xrandr --verbose` and sysfs EDID as optional and continues after bad EDID with `edid_parse_failed` warnings.
- Fake runners and fixture tests cover missing commands, permission-denied DMI, bad EDID, ambiguous sysfs connectors, and numeric GPU vendor IDs.

Still weak:

- Several parsers convert malformed command output to an empty device list without a warning (`ip -j link`, `lsblk -J`).
- Optional secondary commands can fail silently (`bluetoothctl paired-devices`, `lpstat -v`).
- Some probes still treat one source as hard failure even when Linux has obvious fallback sources (`NetworkProbe`, `StorageProbe`, `MemoryProbe`, `BiosProbe`, `UsbProbe`, `BatteryProbe`).

## Deferred Items

These are not fully implemented yet and should remain tracked:

- P1: add warning-on-empty-parse for JSON/parsers where command success does not mean usable data; make optional enrichment command failures visible as warnings.
- P1/P2: add `/proc/cpuinfo` fallback for CPU and `/sys/class/dmi/id` fallback for DMI.
- P2: add network/sysfs, storage/sysfs/SMART, USB verbose/sysfs, camera/sysfs, audio codec/sysfs, Bluetooth DBus/sysfs enrichments.
- P3: optional heavy display/GPU sources such as `glxinfo`, `hwinfo`, and vendor-specific tools.

## Evidence Pointers

- qurbrix CPU: `crates/hw-probe/src/existing.rs`, `crates/hw-parser/src/cpu.rs`, `crates/hw-parser/tests/cpu_sources.rs`.
- qurbrix normalization: `crates/hw-parser/src/normalize/`, `crates/hw-parser/tests/normalize.rs`.
- qurbrix monitor/GPU: `crates/hw-probe/src/existing.rs`, `crates/hw-parser/src/edid.rs`, `crates/hw-parser/src/monitor.rs`, `crates/hw-probe/tests/remaining_category_probes.rs`.
- qurbrix source errors: `crates/hw-source/src/runner.rs`, `crates/hw-probe/src/result.rs`.
- qurbrix remaining category probes: `crates/hw-probe/src/audio.rs`, `battery.rs`, `bluetooth.rs`, `camera.rs`, `cdrom.rs`, `input.rs`, `printer.rs`, `usb.rs`, `pci.rs`.
- Deepin source pool and generators: `../ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/GetInfoPool.cpp`, `DeviceGenerator.cpp`, `HWGenerator.cpp`.
- Deepin device-specific merge/parsing: `../ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/`.
- Kylin hardware heuristics and aliases: `../ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py`.
- Kylin `/proc/cpuinfo` fallback: `../ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py`.
