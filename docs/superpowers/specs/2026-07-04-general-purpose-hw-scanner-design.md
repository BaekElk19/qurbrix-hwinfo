# General-Purpose Hardware Scanner Design

Date: 2026-07-04

## 1. Goal

Rebuild `qurbrix-hwinfo` as a general-purpose Linux hardware scanning library and CLI. The project should follow the proven device coverage and data-source strategy of the two reference projects under `/home/qur/Desktop/20260704_qurbrix-hw/ReferenceProject`:

- `deepin-devicemanager-6.0.67`
- `kylin-os-manager-build-2.0.0-76update2`

The new project should no longer be shaped around the old qurbrix-specific `Inventory`, `ComponentRow`, or `formatprint` APIs. It should expose an industry-default hardware scanner model that scripts, agents, and future qurbrix callers can consume directly.

## 2. Confirmed Scope

### 2.1 Overall sequence

The work is split into two phases:

1. **A: Foundation scanning capability**
2. **B: Output and CLI capability**

Phase A must come first. Phase B depends on the model and evidence/status fields defined in Phase A.

### 2.2 Phase A scope

Phase A uses the A2 scope:

- Add high-priority missing device categories:
  - Audio
  - Bluetooth
  - Input devices: keyboard, mouse, touchpad, touchscreen, tablet, unknown input
  - Camera/image devices
  - Battery/power devices
  - Printer
  - CD-ROM
  - OtherPCI / OtherDevice fallback categories
- Add full USB enumeration.
- Add full PCI enumeration.
- Add a unified driver information model.

### 2.3 Out of scope for Phase A

Phase A does not include:

- Hotplug daemon or long-running service.
- DBus service API.
- Device enable/disable control.
- Driver installation, removal, repository, or recommendation logic.
- CPU governor writes.
- Diagnostic log bundle generation.
- Backward compatibility adapters for old `Inventory`, `ComponentRow`, or `formatprint` callers.

The scanner is read-only. It performs scanning, identification, classification, and structured output.

## 3. Architecture

The project should be rebuilt as a scanner platform with explicit crate boundaries.

```text
qurbrix-hwinfo/
├── crates/
│   ├── hw-model/        # Strongly typed scan model
│   ├── hw-source/       # Command, file, glob, sysfs, procfs, DBus source execution
│   ├── hw-parser/       # Pure parsers for command/file/sysfs outputs
│   ├── hw-probe/        # Per-device probes that produce Device values
│   ├── hw-collect/      # Probe orchestration, merge, warnings, status
│   ├── hw-output/       # Flat JSON, JSONL, summary, table, schema views
│   ├── hw-cli/          # qurbrix-hw CLI entrypoint
│   └── hw-testdata/     # Fixtures and snapshots
└── src/lib.rs           # Top-level facade that re-exports the new API
```

Cross-language callers use the CLI JSON contract. Rust callers use the
top-level `qurbrix-hw` library facade. No separate `hw-api`, `hw-store`, or
`hw-merge` crate is kept without real code and a concrete caller.

### 3.1 `hw-model`

Defines only data structures and enums. It must not read files, run commands, or parse raw text.

Core types:

- `ScanReport`
- `ScanMetadata`
- `SystemInfo`
- `Device`
- `DeviceKind`
- `DeviceProperties`
- `BusInfo`
- `DriverInfo`
- `SourceEvidence`
- `ScanWarning`
- `ScanStatus`

### 3.2 `hw-source`

Provides a unified source execution layer.

Source types:

- Commands: `hwinfo`, `lshw`, `upower`, `lpstat`, `hciconfig`, `bluetoothctl`, `lspci`, `lsusb`, `dmidecode`, `ip`, `ethtool`, `smartctl`, `xrandr`, and similar read-only tools.
- Files: `/proc/*`, `/sys/*`.
- Globs: `/sys/bus/usb/devices/*`, `/sys/bus/pci/devices/*`, `/sys/class/*`.
- DBus: reserved for future BlueZ, CUPS, NetworkManager, and UPower integration.

`hw-source` returns `SourceResult` values containing stdout, stderr, exit status, missing-command status, permission errors, and timeout information. It does not produce hardware devices directly.

### 3.3 `hw-parser`

Contains pure parsing functions. Parsers accept raw command output or file content and return structured intermediate values. They should be easy to test with fixtures.

Important parsers include:

- `lspci -nn -k`
- `lsusb` and optionally `lsusb -v`
- `hwinfo --sound`
- `hwinfo --keyboard`
- `hwinfo --mouse`
- `hwinfo --cdrom`
- `hwinfo --usb`
- `hwinfo --monitor`
- `lshw -C multimedia/input/communication/disk/network/display`
- `upower --dump`
- `lpstat -a` and `lpstat -v`
- `hciconfig -a`
- `bluetoothctl paired-devices`
- `/proc/asound/cards`
- `/proc/asound/modules`
- `/proc/bus/input/devices`
- `/proc/sys/dev/cdrom/info`
- `/sys/class/power_supply/*`
- `/sys/class/video4linux/*`
- `/sys/bus/pci/devices/*`
- `/sys/bus/usb/devices/*`

### 3.4 `hw-probe`

Contains one probe per device category. Probes combine `hw-source` and `hw-parser` and produce strongly typed `Device` values.

Probe interface:

```rust
#[async_trait]
pub trait Probe {
    fn name(&self) -> &'static str;
    fn kinds(&self) -> &'static [DeviceKind];

    async fn probe(&self, ctx: &ProbeContext) -> ProbeResult;
}
```

`ProbeContext` provides access to the source runner, PCI index, USB index, driver map, scan config, and warning sink.

`ProbeResult` contains:

- `devices: Vec<Device>`
- `warnings: Vec<ScanWarning>`
- `consumed: Vec<DeviceRef>` for backing PCI/USB devices that were classified by this probe.

Probes do not perform global deduplication. Global merge is handled by `hw-collect`.

### 3.5 `hw-collect`

Orchestrates the full scan:

1. Run foundational PCI, USB, and driver enumeration.
2. Run category probes.
3. Merge duplicate devices.
4. Associate classified devices with backing PCI/USB devices.
5. Mark consumed PCI/USB devices.
6. Generate `OtherPci` and `OtherDevice` entries from unconsumed backing devices.
7. Aggregate warnings.
8. Compute `ScanStatus`.
9. Return `ScanReport`.

### 3.6 `hw-output`

Converts the internal strongly typed report into stable external views:

- Flat JSON report.
- JSONL device stream.
- Summary output.
- Table output.
- Supported kind list.
- Schema/version output.

### 3.7 `hw-cli`

Implements the `qurbrix-hw` binary. It only handles CLI arguments, logging, exit codes, calls into `hw-collect`, and formats through `hw-output`. It does not contain hardware scanning logic.

## 4. Core Data Model

The model uses two layers:

1. **Internal strongly typed model** for Rust maintainability.
2. **External flat JSON view** for scripts, agents, and CLI consumers.

### 4.1 `ScanReport`

```rust
pub struct ScanReport {
    pub schema_version: String,
    pub metadata: ScanMetadata,
    pub system: SystemInfo,
    pub devices: Vec<Device>,
    pub warnings: Vec<ScanWarning>,
    pub status: ScanStatus,
}
```

Fields:

- `schema_version`: version string, for example `qurbrix.hw.scan.v1`.
- `metadata`: scan time, duration, scanner version, hostname, OS, kernel, and scan configuration summary.
- `system`: whole-machine information that is not naturally one hardware component.
- `devices`: all scanned devices.
- `warnings`: scan-level warnings.
- `status`: `Complete`, `Partial`, or `Failed`.

### 4.2 `Device`

```rust
pub struct Device {
    pub id: String,
    pub kind: DeviceKind,
    pub name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,

    pub bus: Option<BusInfo>,
    pub driver: Option<DriverInfo>,

    pub parent_id: Option<String>,
    pub children: Vec<String>,

    pub capabilities: Vec<String>,
    pub identifiers: Vec<DeviceIdentifier>,
    pub sources: Vec<SourceEvidence>,
    pub warnings: Vec<ScanWarning>,

    pub properties: DeviceProperties,
}
```

### 4.3 `DeviceKind`

Phase A2 supports:

```rust
pub enum DeviceKind {
    System,
    Motherboard,
    Bios,
    Cpu,
    Memory,
    Storage,
    Gpu,
    Monitor,
    Network,
    Audio,
    Bluetooth,
    Input,
    Camera,
    Battery,
    Printer,
    Cdrom,
    Usb,
    Pci,
    OtherPci,
    OtherDevice,
}
```

`Usb` and `Pci` represent full backing-bus enumeration. Specific devices such as USB cameras and USB Bluetooth controllers should appear as their specific kinds and be linked to their backing USB or PCI device when possible.

### 4.4 `DeviceProperties`

```rust
pub enum DeviceProperties {
    System(SystemDeviceInfo),
    Motherboard(MotherboardInfo),
    Bios(BiosInfo),
    Cpu(CpuInfo),
    Memory(MemoryInfo),
    Storage(StorageInfo),
    Gpu(GpuInfo),
    Monitor(MonitorInfo),
    Network(NetworkInfo),
    Audio(AudioInfo),
    Bluetooth(BluetoothInfo),
    Input(InputInfo),
    Camera(CameraInfo),
    Battery(BatteryInfo),
    Printer(PrinterInfo),
    Cdrom(CdromInfo),
    Usb(UsbInfo),
    Pci(PciInfo),
    OtherPci(OtherPciInfo),
    OtherDevice(OtherDeviceInfo),
}
```

Existing CPU, memory, BIOS, monitor, storage, GPU, and network fields should be migrated into these new property types. The old `Inventory`, `ComponentInfo`, `ComponentRow`, and `formatprint` concepts are not part of the new core model.

### 4.5 Device-specific property guidance

Examples of expected fields:

- `AudioInfo`: card index, card name, codec, subsystem, driver, PCI/USB ids, profiles or capabilities when available.
- `BluetoothInfo`: controller address, controller name, driver, powered/discoverable state when available, paired-device count or lightweight paired names when available.
- `InputInfo`: input kind, event node, name, phys path, uniq, handlers, bus type, vendor/product/version, backing USB/Bluetooth/PCI reference.
- `CameraInfo`: video node, name, driver, capabilities, backing USB id.
- `BatteryInfo`: vendor, model, serial, technology, state, capacity, energy full/design/current, voltage, cycle count, presence.
- `PrinterInfo`: queue name, accepting state, device URI, make/model when available, default flag when available.
- `CdromInfo`: device node, vendor, model, media presence, capabilities, bus.
- `PciInfo`: domain/bus/device/function, class, class id, vendor, vendor id, device, device id, subsystem ids, kernel driver, kernel modules.
- `UsbInfo`: bus number, device number, vendor id, product id, class/subclass/protocol, manufacturer, product, serial, speed.
- `OtherPciInfo` and `OtherDeviceInfo`: original kind or class, display name, bus, driver, identifiers, and minimal raw classification hints.

### 4.6 Bus and driver model

```rust
pub enum BusInfo {
    Pci {
        address: String,
        vendor_id: Option<String>,
        device_id: Option<String>,
        subsystem_vendor_id: Option<String>,
        subsystem_device_id: Option<String>,
        class: Option<String>,
    },
    Usb {
        bus: Option<String>,
        device: Option<String>,
        vendor_id: Option<String>,
        product_id: Option<String>,
        interface: Option<String>,
        class: Option<String>,
    },
    Platform { path: String },
    Virtual,
    Unknown,
}

pub struct DriverInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub modules: Vec<String>,
    pub provider: Option<String>,
    pub status: DriverStatus,
}

pub enum DriverStatus {
    InUse,
    Available,
    Missing,
    Unknown,
}
```

Driver status is scan-only. It does not imply driver installation or control features.

### 4.7 Evidence, warnings, and status

```rust
pub struct SourceEvidence {
    pub source: String,
    pub kind: SourceKind,
    pub status: SourceStatus,
    pub summary: Option<String>,
}

pub struct ScanWarning {
    pub code: String,
    pub message: String,
    pub source: Option<String>,
    pub device_id: Option<String>,
}
```

Default output should not include full raw command output. Full raw output is reserved for debug/test paths so normal JSON remains compact and safe to pass through scripts.

Common warning codes:

- `missing_command`
- `permission_denied`
- `timeout`
- `command_failed`
- `parse_failed`
- `partial_data`
- `unsupported_kind`

### 4.8 Device ID strategy

Device IDs must be stable and reproducible:

- PCI: `pci:0000:00:1f.3`
- USB: `usb:001:004`, or `usb:vid:pid:serial` when serial is reliable
- Network: `net:mac:<mac>`, fallback `net:iface:<name>`
- Storage: `storage:wwn:<wwn>`, `storage:serial:<serial>`, fallback `storage:dev:<node>`
- Battery: `battery:<sysfs_name>`
- Input: `input:event:<event_name>` or `input:phys:<phys>`
- Camera: `camera:/dev/video0`
- Printer: `printer:<queue>`
- Unknown: bus/name/identifier-based stable hash

## 5. Collection and Parsing Strategy

### 5.1 Scan order

The default scan runs in this order:

1. Foundational enumeration:
   - PCI full enumeration
   - USB full enumeration
   - unified driver map
2. Category probes:
   - existing categories: CPU, memory, BIOS, storage, GPU, monitor, network
   - new categories: audio, bluetooth, input, camera, battery, printer, CD-ROM
3. Merge and fallback:
   - deduplicate by `Device.id`
   - associate specific devices with backing PCI/USB devices
   - mark consumed backing devices
   - generate `OtherPci` and `OtherDevice`
   - aggregate warnings and compute report status

### 5.2 Data-source table

| Capability | Primary sources | Fallback sources |
| --- | --- | --- |
| PCI + driver | `lspci -nn -k` | `/sys/bus/pci/devices/*` |
| USB | `lsusb`, optional `lsusb -v` | `/sys/bus/usb/devices/*` |
| CPU | `lscpu`, `/proc/cpuinfo`, `dmidecode -t 4` | CPU sysfs |
| Memory | `dmidecode -t memory`, `/proc/meminfo` | sysfs DMI |
| BIOS/board/chassis | `dmidecode -t 0/1/2/3/16/17` | `/sys/class/dmi/id/*` |
| Storage | `lsblk`, `udevadm`, `smartctl`, `hwinfo --disk`, `lshw -C disk` | `/sys/block/*` |
| GPU | `lspci -nn -k`, `lshw -C display`, `hwinfo --display` | `/sys/class/drm` |
| Monitor | DRM EDID, `xrandr`, `hwinfo --monitor` | DRM sysfs |
| Network | `ip -j link`, `ethtool`, `lspci -k`, `lshw -C network` | `/sys/class/net/*` |
| Audio | `hwinfo --sound`, `lshw -C multimedia`, `/proc/asound`, `lspci -k` | PCI/USB index |
| Bluetooth | `hwinfo --usb`, `hciconfig -a`, `bluetoothctl paired-devices` | USB/PCI index |
| Input | `/proc/bus/input/devices`, `hwinfo --keyboard`, `hwinfo --mouse`, `lshw -C input` | `/sys/class/input/*` |
| Camera | `/sys/class/video4linux/*`, `v4l2-ctl --list-devices`, `hwinfo --usb` | USB index |
| Battery/power | `upower --dump` | `/sys/class/power_supply/*` |
| Printer | `lpstat -a`, `lpstat -v` | CUPS DBus later |
| CD-ROM | `hwinfo --cdrom`, `lshw -C disk`, `/proc/sys/dev/cdrom/info` | `/sys/block/sr*` |
| OtherPCI | unconsumed PCI index | none |
| OtherDevice | unconsumed USB index and unclassified source records | none |

### 5.3 Probe-specific strategies

#### PCI probe

Parse `lspci -nn -k` for address, class, class id, vendor, vendor id, device, device id, subsystem ids, `Kernel driver in use`, and `Kernel modules`. Fall back to sysfs files under `/sys/bus/pci/devices/*`.

#### USB probe

Parse `lsusb` for bus, device, VID, PID, and product text. Optionally parse `lsusb -v` for class/subclass/protocol, manufacturer, product, serial, and speed. Fall back to sysfs files under `/sys/bus/usb/devices/*`.

#### Audio probe

Use `hwinfo --sound`, `lshw -C multimedia`, `/proc/asound/cards`, `/proc/asound/modules`, and multimedia/audio-class devices from the PCI/USB index.

#### Bluetooth probe

Use `hwinfo --usb`, `hciconfig -a`, `bluetoothctl paired-devices`, and USB/PCI index classification or name matching. BlueZ DBus is a later enhancement.

#### Input probe

Use `/proc/bus/input/devices`, `hwinfo --keyboard`, `hwinfo --mouse`, `lshw -C input`, and `/sys/class/input/*`. Classify into keyboard, mouse, touchpad, touchscreen, tablet, or unknown input.

#### Camera probe

Use `/sys/class/video4linux/*`, optional `v4l2-ctl --list-devices`, `hwinfo --usb`, and USB video-class or product-name matching.

#### Battery probe

Use `upower --dump`, falling back to `/sys/class/power_supply/*`.

#### Printer probe

Use `lpstat -a` and `lpstat -v`. CUPS DBus can be added later.

#### CD-ROM probe

Use `hwinfo --cdrom`, `lshw -C disk`, `/proc/sys/dev/cdrom/info`, and `/sys/block/sr*`.

#### Existing categories

CPU, memory, BIOS, storage, GPU, monitor, and network should be migrated from current parser/model code into the new `Device` model. Existing reliable parsing should be reused where practical, then enriched with reference-project data sources.

### 5.4 Merge and fallback rules

- Deduplicate by `Device.id`.
- Prefer a specific category over generic `Pci` or `Usb` classification.
- Keep backing `Pci`/`Usb` devices available for bus visibility and parent/child relationships when useful.
- Merge source evidence from all contributing probes.
- Prefer structured identifiers from sysfs/procfs over display names from command text.
- Use command text to improve user-facing names and model strings.
- Generate `OtherPci` for unconsumed PCI devices.
- Generate `OtherDevice` for unconsumed USB devices and unclassified source records.

### 5.5 Failure behavior

- Missing optional commands do not fail the scan; they produce `missing_command` warnings.
- Permission errors do not fail the scan unless they prevent any useful report from being produced.
- Parser failures are isolated to their source and produce `parse_failed` warnings.
- `ScanStatus::Complete` means the requested scan completed without material warnings.
- `ScanStatus::Partial` means a report was produced but at least one source failed, was unavailable, timed out, or produced partial data.
- `ScanStatus::Failed` means no valid report could be produced for the requested scan.

## 6. CLI and Output Contract

The installed binary remains:

```bash
qurbrix-hw
```

CLI is script/agent-first. stdout contains only structured results. Logs and diagnostics go to stderr.

### 6.1 Commands

```text
qurbrix-hw scan
qurbrix-hw summary
qurbrix-hw table
qurbrix-hw list-kinds
qurbrix-hw schema
qurbrix-hw sources
```

### 6.2 `scan`

Main scan command:

```bash
qurbrix-hw scan
qurbrix-hw scan --format json
qurbrix-hw scan --format jsonl
qurbrix-hw scan --pretty
qurbrix-hw scan --kind storage --kind network
qurbrix-hw scan --exclude-kind usb --exclude-kind pci
qurbrix-hw scan --timeout 30s
qurbrix-hw scan --no-optional-sources
```

Default:

```bash
qurbrix-hw scan --format json
```

Supported formats:

- `json`: complete flat report view.
- `jsonl`: one flat device view per line; implementation planning should decide whether a final summary line is included.
- `typed-json`: internal strongly typed report for debugging.
- `summary-json`: metadata, status, counts, and warnings only.

### 6.3 Human commands

`summary` prints a compact human-readable overview.

`table` prints a device table and supports `--kind` filtering.

`list-kinds` lists all supported device kinds and can optionally output JSON.

`schema` prints schema version or JSON Schema.

`sources` checks available data sources without running a full scan. It helps scripts and agents understand which optional tools are installed.

### 6.4 Flat JSON view

Default JSON output shape:

```json
{
  "schema_version": "qurbrix.hw.scan.v1",
  "status": "partial",
  "metadata": {
    "hostname": "host1",
    "os": "Linux",
    "kernel": "6.12.36-amd64-desktop-rolling",
    "duration_ms": 842,
    "scanner_version": "0.1.0"
  },
  "summary": {
    "device_count": 42,
    "counts_by_kind": {
      "cpu": 1,
      "memory": 2,
      "storage": 2,
      "audio": 1
    },
    "warning_count": 3
  },
  "devices": [
    {
      "id": "audio:pci:0000:00:1f.3",
      "kind": "audio",
      "name": "Intel HD Audio Controller",
      "vendor": "Intel Corporation",
      "model": "HD Audio Controller",
      "serial": null,
      "bus": {
        "kind": "pci",
        "address": "0000:00:1f.3",
        "vendor_id": "8086",
        "device_id": "a348"
      },
      "driver": {
        "name": "snd_hda_intel",
        "status": "in_use",
        "modules": ["snd_hda_intel"]
      },
      "capabilities": ["audio"],
      "identifiers": [
        {"kind": "pci_id", "value": "8086:a348"}
      ],
      "properties": {
        "card_index": 0,
        "codec": "Realtek ALC..."
      },
      "sources": [
        {
          "source": "hwinfo --sound",
          "kind": "command",
          "status": "success"
        },
        {
          "source": "lspci -nn -k",
          "kind": "command",
          "status": "success"
        }
      ],
      "warnings": []
    }
  ],
  "warnings": [
    {
      "code": "missing_command",
      "message": "hwinfo is not available",
      "source": "hwinfo --usb"
    }
  ]
}
```

Conventions:

- Device `kind` values use kebab-case, for example `other-pci` and `other-device`.
- Status values use snake_case, for example `in_use` and `permission_denied`.
- `properties` is a flat per-kind object.
- stdout never contains logs.
- stderr contains tracing logs and fatal error explanations.

### 6.5 Exit codes

| Exit code | Meaning |
| --- | --- |
| 0 | Scan succeeded, including `complete` or `partial` reports |
| 1 | CLI argument error or output serialization failure |
| 2 | Scan runtime failure; no valid report generated |
| 3 | Requested kind/source is unsupported |
| 4 | Permission failure prevents core scan |
| 124 | Timeout |

`partial` reports return exit code 0 because scripts and agents can still consume them. A later `--fail-on-warning` option can convert warnings into non-zero exits if needed.

### 6.6 `ScanConfig`

Initial config:

```rust
pub struct ScanConfig {
    pub kinds: Option<Vec<DeviceKind>>,
    pub exclude_kinds: Vec<DeviceKind>,
    pub timeout: Duration,
    pub optional_sources: bool,
    pub include_sources: bool,
    pub include_warnings: bool,
}
```

CLI mapping:

- `--kind`
- `--exclude-kind`
- `--timeout`
- `--no-optional-sources`
- `--no-sources`
- `--no-warnings`

No config file is required in Phase A/B. Profiles can be added later.

## 7. Testing Strategy

### 7.1 Parser fixtures

`hw-parser` must use fixture-driven tests. Fixtures live under `crates/hw-testdata/fixtures/`.

Suggested layout:

```text
crates/hw-testdata/fixtures/
├── deepin/
│   ├── hwinfo-sound.txt
│   ├── hwinfo-keyboard.txt
│   ├── hwinfo-mouse.txt
│   ├── hwinfo-cdrom.txt
│   └── hwinfo-usb.txt
├── kylin/
│   ├── upower-dump.txt
│   ├── service-support-hwinfo-sound.txt
│   └── ...
├── sysfs/
│   ├── power_supply/
│   ├── usb_devices/
│   └── pci_devices/
└── proc/
    ├── bus-input-devices.txt
    └── asound-cards.txt
```

### 7.2 Snapshot tests

Use `insta` snapshots for:

- Parser output.
- Probe output.
- Full `ScanReport`.
- Flat JSON view.
- CLI summary/table output.

### 7.3 Integration tests

Use a fake source runner so tests do not depend on the current machine's hardware:

```rust
FakeSourceRunner {
    command_outputs: HashMap<CommandSpec, SourceResult>,
    files: HashMap<PathBuf, String>,
    globs: HashMap<String, Vec<PathBuf>>,
}
```

### 7.4 Real-machine smoke tests

Optional smoke tests can run outside CI:

```bash
cargo run -p hw-cli -- scan --format summary-json
cargo run -p hw-cli -- sources --format json
```

### 7.5 Acceptance criteria

Phase A/B is acceptable when:

1. `ScanReport` is produced by the new collector.
2. Flat JSON is produced by the output layer.
3. Existing CPU, memory, BIOS, storage, GPU, monitor, and network capabilities are migrated into the new device model.
4. Audio, Bluetooth, input, camera, battery, printer, and CD-ROM devices are supported.
5. PCI and USB full enumeration are supported.
6. OtherPCI and OtherDevice fallback categories are supported.
7. Device driver information is exposed where available.
8. Missing commands and permission failures produce warnings instead of crashes.
9. `qurbrix-hw scan --format json` is stable for scripts and agents.
10. `qurbrix-hw list-kinds` lists all supported categories.

## 8. Migration Notes

The current top-level APIs are not compatibility targets:

- `collect(&CollectConfig) -> Inventory`
- `compute_bind_id(&Inventory) -> String`
- `to_component_rows(...) -> Vec<ComponentRow>`
- `build_formatprint_payload(...)`

They should be removed or replaced by the new general scanner API. qurbrix-specific callers are responsible for adapting to the new model.

The existing crates can be replaced or repurposed during implementation. The final shape should prioritize the new scanner platform architecture over preserving internal file layout.
