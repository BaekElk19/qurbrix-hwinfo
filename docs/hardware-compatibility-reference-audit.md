# Hardware Compatibility Reference Audit

Date: 2026-07-06

This audit compares the current qurbrix-hwinfo implementation with:

- `../ReferenceProject/deepin-devicemanager-6.0.67`
- `../ReferenceProject/kylin-os-manager-build-2.0.0-76update2`

Scope: verify each component parser/probe against the two references, with special attention to fallback and exception-handling practices.

## Summary

Current qurbrix-hwinfo has absorbed several P0/P1 compatibility gaps that were previously missing:

- CPU uses `lscpu`, `lshw -class processor`, and `dmidecode -t 4` as optional sources, merges DMI counts/speeds, protects Loongson names, and normalizes CPU vendors/architectures.
- CPU also reads `/proc/cpuinfo` as an optional procfs fallback, using Kylin-style `Hardware`/`Processor` fields when command sources are missing or incomplete.
- CPU reads `/proc/hardware` as an optional procfs fallback for Kylin's Kirin 990/9006C detection path.
- Monitor uses `xrandr --query`, `xrandr --verbose`, and `/sys/class/drm/*/edid`, parses EDID in-process, and preserves monitor devices when EDID parsing fails.
- GPU uses `lspci -nn -k`, normalizes common and domestic GPU vendor names, falls back to PCI vendor IDs when lspci text is generic, and consumes display-class `/sys/bus/pci/devices/*` nodes with kernel driver data when `lspci` cannot run.
- Source execution has structured `Missing`, `PermissionDenied`, `Timeout`, and `Failed` classifications, which is cleaner than most reference scripts and should remain the project-wide pattern.
- Command execution forces `LC_ALL=C`, `LANG=C`, and `LANGUAGE=en`, absorbing Deepin/Kylin's locale-stabilization practice for English-key parsers.
- BIOS/motherboard probing falls back to `/sys/class/dmi/id` when `dmidecode` is missing or denied.
- Memory probing falls back to `/proc/meminfo` total memory when `dmidecode -t memory` cannot run.
- Battery probing falls back to `/sys/class/power_supply/BAT*` when `upower --dump` cannot run.
- Network probing uses `ip -j link` plus optional `ip -j addr`, enriches interfaces from `/sys/class/net/*` with speed, duplex, wireless marking, and kernel driver, and falls back to sysfs interfaces when `ip -j link` cannot run.
- Storage probing requests WWN/firmware revision from `lsblk`, enriches kernel driver data from `/sys/block/*/device/uevent`, and falls back to `/sys/block/*` when `lsblk` cannot run.
- USB probing enriches `lsusb` records from `/sys/bus/usb/devices/*` when bus/device numbers match, and falls back to sysfs devices when `lsusb` cannot run.
- Bluetooth probing falls back to `/sys/class/bluetooth/hci*` when `hciconfig -a` cannot run, and warns when successful `hciconfig` output contains no parsable controllers.
- Camera probing falls back to `/sys/class/video4linux/video*` when `v4l2-ctl --list-devices` cannot run and enriches cameras with video4linux sysfs driver data when available.
- Input probing falls back to `/sys/class/input/event*` when `/proc/bus/input/devices` cannot be read or parses no devices.
- CD-ROM probing falls back to `/sys/class/block/sr*` with basic vendor/model/serial identity when `/proc/sys/dev/cdrom/info` cannot be read or parses no drives.
- Printer probing falls back to `lpstat -v` when `lpstat -a` cannot run or parses no queues, and uses `lpstat -d` to mark the default destination when available.
- PCI probing falls back to `/sys/bus/pci/devices/*`, including available kernel driver data, when `lspci -nn -k` cannot run.

Confirmed defects fixed during this audit:

- `CPU MHz` from `lscpu` is now used as `current_freq_mhz` when DMI current speed is unavailable.
- Monitor sysfs fallback now works when `xrandr --query` is missing, but does not create fake devices from empty/bad sysfs EDID.
- sysfs EDID read failures now produce source warnings instead of being silently ignored.
- GPU vendor normalization now uses numeric PCI vendor IDs when text is only generic `Device`.
- Loopback/common virtual network interfaces, UPower line-power devices, USB root hubs/hubs, empty BIOS DMI output, and duplicate UVC camera video nodes are now filtered.
- Malformed `ip -j link` / `lsblk -J` output and failed optional Bluetooth/printer enrichment sources now produce warnings instead of silently returning incomplete data.
- `/proc/cpuinfo` fallback now covers ARM-style `Hardware`/`Processor`, `cpu MHz`, `Features`, `BogoMIPS`, and logical processor counts.
- English/C locale is now enforced for real command execution, preventing localized command keys from silently breaking parsers such as `lscpu`.
- `/proc/hardware` fallback now recognizes `HUAWEI Kirin 990`, `kirin990`, and `HUAWEI Kirin 9006C`.
- `/sys/class/dmi/id` fallback now preserves BIOS/baseboard identity when `dmidecode -t 0,1,2,3` cannot run.
- `/proc/meminfo` fallback now preserves aggregate memory capacity when `dmidecode -t memory` cannot run.
- `/sys/class/power_supply` fallback now preserves battery identity, capacity, energy, voltage, cycle count, and present state when UPower cannot run.
- Network probing now preserves IPv4/IPv6 addresses from `ip -j addr` when available; `/sys/class/net` preserves interface name, MAC, operstate, speed, duplex, wireless capability, and kernel driver, including when `ip -j link` cannot run.
- Successful `lsblk` storage probing now preserves WWN and firmware revision and uses WWN for stable storage IDs when available.
- `/sys/block` storage enrichment now preserves kernel driver data when available; fallback also preserves storage node, vendor, model, serial identity, WWN, firmware revision, size, and rotational media type when `lsblk` cannot run.
- `/sys/bus/usb/devices` enrichment now preserves USB class/subclass/protocol, manufacturer, serial, and speed on matched `lsusb` records; fallback preserves bus/device numbers, VID/PID, device class/subclass/protocol, manufacturer, product, serial, and speed when `lsusb` cannot run.
- `/sys/class/bluetooth` fallback now preserves basic Bluetooth controller presence and rfkill unblock/block state when `hciconfig -a` cannot run.
- Empty successful `hciconfig -a` output now produces a `source_empty` warning instead of silently returning no Bluetooth devices.
- `/sys/class/video4linux` fallback now preserves basic camera name and `/dev/video*` node when `v4l2-ctl --list-devices` cannot run.
- Empty successful `/proc/bus/input/devices` reads now produce a `source_empty` warning and still try `/sys/class/input/event*`.
- Empty successful `/proc/sys/dev/cdrom/info` reads now produce a `source_empty` warning and still try `/sys/class/block/sr*`.
- Empty successful `lpstat -a` output now produces a `source_empty` warning and still tries `lpstat -v`.
- Printer probing now preserves the default CUPS destination from `lpstat -d` when available without treating a missing default source as fatal.
- Camera probing now preserves the kernel video driver from `/sys/class/video4linux/video*/device/uevent` on both `v4l2-ctl` and sysfs fallback paths.
- PCI and sysfs-backed GPU fallback now preserve kernel driver data from `/sys/bus/pci/devices/*/uevent` when available.

## Component Matrix

| Component | Current qurbrix implementation | Absorbed from references | Remaining gaps | Priority |
| --- | --- | --- | --- | --- |
| CPU | `CpuProbe` runs `lscpu`, `lshw -class processor`, `dmidecode -t 4`, optional `/proc/cpuinfo`, and optional `/proc/hardware`; parser reads extended lscpu, DMI type 4, procfs `Hardware`/`Processor` fallback fields, and Kirin `/proc/hardware` names; merge handles DMI counts/speeds and Loongson protection. | Deepin CPU three-source merge, DMI count correction, Loongson/lshw protection, Phytium DMI current-speed fallback, locale-stable command execution. Kylin `/proc/cpuinfo` `Hardware` fallback, `/proc/hardware` Kirin fallback, CPU vendor aliases, and `LANGUAGE=en` command practice are partially absorbed. | Parsed family/model/stepping/bogomips/virtualization are not exposed in `CpuInfo`; broader real-machine CPU fixture coverage is still thin. | P1/P2 |
| Architecture/vendor normalization | `normalize_arch`, `normalize_cpu_vendor_id`, `infer_cpu_vendor_from_name`, `normalize_gpu_vendor`, `normalize_gpu_vendor_id`. | Deepin arch aliases for amd64/arm64/sw_64/loongarch; Kylin domestic CPU/GPU vendor heuristics. | Huawei/Kunpeng/HiSilicon canonical choice intentionally differs from Kylin; alias tables remain intentionally small. | P1 |
| Monitor | `xrandr --query` for connector/resolution, `xrandr --verbose` and sysfs EDID for identity; EDID parser fills manufacturer/product/date/size/preferred mode. | Kylin xrandr verbose EDID extraction; Deepin sysfs DRM EDID parsing pattern; no `/tmp` temp file or external `edid-decode`. | No gamma/inch/maxmode fields; no `hwinfo_monitor`; ambiguous duplicate sysfs connectors are skipped rather than matched by card. | P2 |
| GPU | `lspci -nn -k` display/VGA/3D records, driver/modules, text and numeric vendor normalization; falls back to display-class `/sys/bus/pci/devices/*` records with kernel driver data when lspci cannot run. | PCI GPU parsing and domestic GPU aliases from Kylin; driver extraction from lspci; Linux sysfs PCI display-class fallback with lightweight driver preservation. | No `lshw -C display`, `glxinfo -B`, `/sys/class/drm/card*/device`, dmesg/nvidia memory enrichment; sysfs fallback lacks human-readable device names and kernel modules. | P2/P3 |
| Memory | `dmidecode -t memory`, DIMM size/vendor/type/speed/slot/serial/part, filters `No Module Installed`; falls back to `/proc/meminfo` aggregate total when DMI is unavailable/permission denied. | Main DMI memory path from both references plus procfs total-memory fallback. | No `lshw_memory` or sysfs DIMM-level fallback when DMI is unavailable/permission denied. | P2 |
| BIOS / motherboard / DMI | `dmidecode -t 0,1,2,3`, with `/sys/class/dmi/id` fallback for BIOS vendor/version/date and baseboard manufacturer/product/serial when dmidecode cannot run; empty dmidecode output still produces `source_empty` rather than generic devices. | Core DMI BIOS/baseboard parsing, sysfs DMI fallback, and Deepin-style skip-empty behavior. | Chassis/system/language/memory-array data not modeled. | P2 |
| Storage | `lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV`, disk-only filtering, parse failure warning, WWN-backed stable IDs, firmware revision; enriches kernel driver from `/sys/block/*/device/uevent`; falls back to `/sys/block/*` for node/vendor/model/serial/WWN/firmware/size/rotational media type and driver when `lsblk` cannot run. | Basic lsblk disk enumeration plus Linux sysfs disk fallback with WWN/firmware and lightweight driver preservation. | No lshw/hwinfo/hdparm/smartctl fusion; no SMART/temp/controller enrichment. | P2 |
| Network | `ip -j link`, interface/MAC/operstate, optional `ip -j addr` IPv4/IPv6 enrichment; filters loopback/common virtual interfaces; malformed JSON produces warning; enriches interfaces from `/sys/class/net/*` with speed, duplex, wireless capability, and `DRIVER=` from uevent; falls back to sysfs interfaces when `ip` cannot run. | Basic network interface enumeration, IP address enrichment, Kylin-style avoidance of non-physical interfaces, Linux sysfs fallback, and lightweight sysfs driver/wireless enrichment. | No lshw/lspci/NM DBus fallback; no explicit ethernet/wireless type field beyond `wireless` capability. | P1/P2 |
| Audio | `/proc/asound/cards`, card index/name; enriches from `/proc/asound/card*/codec#*` for codec and `/sys/class/sound/card*/device` for driver/subsystem IDs; falls back to `/sys/class/sound/card*` for ALSA card index/name plus available enrichment when proc asound cards is unavailable. | Deepin/Kylin use `/proc/asound` and multimedia sources; base source, Linux sysfs card fallback, lightweight codec, driver, and subsystem enrichment absorbed. | No lshw/hwinfo/PCI fusion; no vendor normalization; limited profile data. | P2 |
| Bluetooth | `hciconfig -a`, optional `bluetoothctl paired-devices`; paired source failures warn; empty controller parses emit `source_empty`; falls back to `/sys/class/bluetooth/hci*` plus rfkill name/state when `hciconfig` cannot run. | Deepin `hciconfig` path and lightweight paired-device enrichment, plus Linux sysfs controller fallback and skip-empty warning behavior. | No lshw/hwinfo/BlueZ DBus fallback; sysfs fallback cannot recover controller address or paired devices. | P1/P2 |
| Input | `/proc/bus/input/devices`, handlers/IDs, keyboard/mouse/touchpad/touchscreen classification; falls back to `/sys/class/input/event*` for basic event node/name/id fields when proc input devices is unavailable or parses empty. | Proc input parsing, basic classification, Linux sysfs event fallback, and skip-empty warning behavior. | No lshw/hwinfo enrichment; no EV bitmask classification; `Tablet` remains unused; limited bus-specific classification; sysfs fallback cannot recover handlers. | P2 |
| Camera | `v4l2-ctl --list-devices`, emits one device per physical camera record using the first `/dev/video*` node; enriches from `/sys/class/video4linux/video*/device/uevent` for kernel driver; falls back to `/sys/class/video4linux/video*` for basic name, node, and driver when `v4l2-ctl` cannot run. | Basic video device discovery, Deepin-style physical-device deduplication, Linux video4linux sysfs fallback, and lightweight driver enrichment. | No lshw/hwinfo fallback; no vendor/speed/serial enrichment. | P2 |
| Battery | `upower --dump`, battery capacity/energy/voltage/vendor/model/serial; filters line-power devices; falls back to `/sys/class/power_supply/BAT*` for battery fields including cycle count. | UPower-based collection, Deepin-style line-power filtering, and Linux sysfs battery fallback. | No temperature fallback or vendor normalization. | P2 |
| Printer | `lpstat -a`, optional `lpstat -v`, optional `lpstat -d`; URI source failures warn; falls back to `lpstat -v` queue/URI records when `lpstat -a` cannot run or parses empty; marks default destination when available. | CUPS queue enumeration, URI-source fallback, default-destination enrichment, and skip-empty warning behavior. | No make/model/state/interface; fallback cannot recover accepting state. | P2 |
| CD-ROM | `/proc/sys/dev/cdrom/info`, drive names and basic capabilities; falls back to `/sys/class/block/sr*` for drive nodes plus vendor/model/serial identity when proc cdrom info is unavailable or parses empty. | Proc cdrom discovery, Linux sysfs block fallback, basic sysfs identity enrichment, and skip-empty warning behavior. | No lshw/hwinfo/lsscsi fallback; no firmware enrichment; sysfs fallback cannot recover capabilities. | P2 |
| USB | `lsusb` for bus/device/VID/PID/product plus matched `/sys/bus/usb/devices/*` enrichment for class/subclass/protocol, manufacturer, serial, and speed; falls back to sysfs for bus/device IDs, VID/PID, identity, class data, and speed when `lsusb` cannot run; filters root hubs, USB hubs, sysfs host controllers, and sysfs interface entries. | Basic USB enumeration, Linux sysfs success-path enrichment and fallback, and Deepin/Kylin hub filtering. | No `lsusb -v`; no maxpower or detailed interface descriptor enrichment; USB devices consumed by Bluetooth/camera/input/printer are not deduplicated. | P2 |
| PCI / Other PCI | `lspci -nn -k`, class/vendor/device IDs, driver/modules; falls back to `/sys/bus/pci/devices/*` for address, vendor/device/class/subsystem IDs and kernel driver when lspci cannot run; sysfs class IDs retain the full 24-bit code such as `040300`; display-class sysfs PCI nodes are consumed by GPU; unconsumed PCI devices become `OtherPci`. | PCI class and driver extraction plus Linux sysfs PCI ID and driver fallback; GPU consumption of sysfs display-class PCI nodes. | Network/audio/storage/camera/bluetooth may duplicate as `OtherPci`; sysfs fallback lacks kernel modules and human-readable vendor/device/class names. | P2 |

## Exception Handling Audit

Absorbed and preserved:

- `hw-source` classifies command/file errors as `Missing`, `PermissionDenied`, `Timeout`, or `Failed`, and runs commands with stable English/C locale environment.
- CPU treats `lscpu`, `lshw`, and `dmidecode` as optional and emits warnings for failed sources while still producing a CPU when any useful source exists.
- Monitor treats `xrandr --verbose` and sysfs EDID as optional and continues after bad EDID with `edid_parse_failed` warnings.
- USB preserves the missing/failed `lsusb` warning while still emitting devices from `/sys/bus/usb/devices/*` when usable sysfs device directories exist.
- Audio preserves the missing/failed `/proc/asound/cards` warning while still emitting ALSA card devices from `/sys/class/sound/card*` when present, with sysfs driver/subsystem enrichment where available.
- Bluetooth preserves the missing/failed `hciconfig -a` warning while still emitting controllers from `/sys/class/bluetooth/hci*` when usable sysfs controller directories exist, and emits `source_empty` when successful `hciconfig` output parses no controllers.
- Input preserves the missing/failed `/proc/bus/input/devices` warning while still emitting basic event devices from `/sys/class/input/event*` when present, and emits `source_empty` plus sysfs fallback when proc input parsing yields no devices.
- Camera preserves the missing/failed `v4l2-ctl --list-devices` warning while still emitting devices from `/sys/class/video4linux/video*` when usable sysfs video nodes exist, and preserves video4linux kernel driver data when available.
- CD-ROM preserves the missing/failed `/proc/sys/dev/cdrom/info` warning while still emitting optical drive nodes and available identity fields from `/sys/class/block/sr*` when present, and emits `source_empty` plus sysfs fallback when proc cdrom parsing yields no drives.
- PCI preserves the missing/failed `lspci -nn -k` warning while still emitting PCI ID and driver devices from `/sys/bus/pci/devices/*` when present.
- GPU preserves the missing/failed `lspci -nn -k` warning while still emitting GPU devices from display-class `/sys/bus/pci/devices/*` with available driver data when present.
- Printer preserves the missing/failed `lpstat -a` warning while still emitting queue/URI devices from `lpstat -v` when available, emits `source_empty` plus URI fallback when `lpstat -a` parsing yields no queues, and preserves default destination data from `lpstat -d` when available.
- Fake runners and fixture tests cover missing commands, permission-denied DMI, bad EDID, ambiguous sysfs connectors, and numeric GPU vendor IDs.

Still weak:

- Input, printer, and CD-ROM still have limited fallback/enrichment coverage compared with the reference projects; audio still lacks lshw/hwinfo/PCI fusion; camera still lacks lshw/hwinfo and vendor/serial enrichment.

## Deferred Items

These are not fully implemented yet and should remain tracked:

- P1: continue adding warning-on-empty-parse for additional parsers where command success does not mean usable data.
- P1b: decide whether parsed CPU family/model/stepping/bogomips/virtualization should be exposed in `CpuInfo` or kept parser-internal.
- P2: add network type/DBus/lshw/lspci, storage SMART/temp/controller, USB verbose descriptors, camera lshw/hwinfo enrichment, audio lshw/hwinfo/PCI fusion, and Bluetooth lshw/DBus enrichments.
- P3: optional heavy display/GPU sources such as `glxinfo`, `hwinfo`, and vendor-specific tools.

## Evidence Pointers

- qurbrix CPU: `crates/hw-probe/src/existing.rs`, `crates/hw-parser/src/cpu.rs`, `crates/hw-parser/tests/cpu_sources.rs`, `crates/hw-probe/tests/existing_category_probes.rs`.
- qurbrix normalization: `crates/hw-parser/src/normalize/`, `crates/hw-parser/tests/normalize.rs`.
- qurbrix monitor/GPU: `crates/hw-probe/src/existing.rs`, `crates/hw-parser/src/edid.rs`, `crates/hw-parser/src/monitor.rs`, `crates/hw-probe/tests/remaining_category_probes.rs`.
- qurbrix source errors: `crates/hw-source/src/runner.rs`, `crates/hw-probe/src/result.rs`.
- qurbrix remaining category probes: `crates/hw-probe/src/audio.rs`, `battery.rs`, `bluetooth.rs`, `camera.rs`, `cdrom.rs`, `input.rs`, `printer.rs`, `usb.rs`, `pci.rs`.
- Deepin source pool and generators: `../ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/GenerateDevice/GetInfoPool.cpp`, `DeviceGenerator.cpp`, `HWGenerator.cpp`.
- Deepin device-specific merge/parsing: `../ReferenceProject/deepin-devicemanager-6.0.67/deepin-devicemanager/src/DeviceManager/`.
- Kylin hardware heuristics and aliases: `../ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/detailinfo/cpuinfo.py`.
- Kylin `/proc/cpuinfo` fallback: `../ReferenceProject/kylin-os-manager-build-2.0.0-76update2/rubbish-clear-dbus/src/sysinfo/__init__.py`.
