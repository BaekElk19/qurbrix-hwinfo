# General-Purpose Hardware Scanner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `qurbrix-hwinfo` into a general-purpose Linux hardware scanning library and `qurbrix-hw` CLI with a strongly typed internal model and script/agent-friendly output.

**Architecture:** Replace the old qurbrix-specific `Inventory`/`ComponentRow` shape with a scanner platform split into `hw-model`, `hw-source`, `hw-parser`, `hw-probe`, `hw-collect`, `hw-output`, `hw-cli`, and `hw-testdata`. Collection starts from PCI/USB/driver enumeration, runs category probes, merges backing devices, emits typed `ScanReport`, then converts to flat JSON/JSONL/summary/table views.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `tokio`, `async-trait`, `anyhow`, `regex`, `clap`, `tracing`, `tracing-subscriber`, `insta`.

## Global Constraints

- Target platform is Linux.
- Scanner is read-only: do not implement device enable/disable, driver install/remove, CPU governor writes, or destructive actions.
- Do not add backward compatibility adapters for old `Inventory`, `ComponentRow`, or `formatprint` callers.
- Default CLI output is machine-readable and goes to stdout; logs and diagnostics go to stderr.
- Missing commands, permission errors, timeouts, and parser failures produce warnings whenever a usable report can still be produced.
- `partial` scan reports exit with code `0`.
- Device kind strings in flat output use kebab-case, for example `other-pci` and `other-device`.
- Status strings in flat output use snake_case, for example `in_use` and `permission_denied`.
- Full raw command output is not included in default JSON output.
- Phase A/B does not require config files or profiles.

---

## File Structure

Create or replace these files. Existing crates may be overwritten because the project intentionally breaks old API compatibility.

### Workspace and facade

- Modify `Cargo.toml`: workspace members, dependencies, root facade dependency list, `[[bin]]` pointing to `crates/hw-cli/src/main.rs` or top-level `src/bin/cli.rs` if keeping the current binary shim.
- Modify `src/lib.rs`: re-export new public API from `hw-model`, `hw-collect`, and `hw-output`.
- Delete or stop referencing `src/normalize.rs` and old `src/bin/cli.rs` APIs after the new CLI crate is wired.

### `crates/hw-model`

- Modify `crates/hw-model/Cargo.toml`.
- Replace `crates/hw-model/src/lib.rs` with module exports.
- Create `crates/hw-model/src/kind.rs`: `DeviceKind` and string parsing/serialization helpers.
- Create `crates/hw-model/src/report.rs`: `ScanReport`, `ScanMetadata`, `SystemInfo`, `ScanStatus`, `ScanConfig`.
- Create `crates/hw-model/src/device.rs`: `Device`, `DeviceIdentifier`, `DeviceRef`.
- Create `crates/hw-model/src/bus.rs`: `BusInfo`.
- Create `crates/hw-model/src/driver.rs`: `DriverInfo`, `DriverStatus`.
- Create `crates/hw-model/src/evidence.rs`: `SourceEvidence`, `SourceKind`, `SourceStatus`, `ScanWarning`.
- Create `crates/hw-model/src/properties.rs`: `DeviceProperties` and all `*Info` property structs.
- Create `crates/hw-model/src/id.rs`: stable device ID constructors.

### `crates/hw-source`

- Modify `crates/hw-source/Cargo.toml`.
- Replace `crates/hw-source/src/lib.rs` with module exports.
- Create `crates/hw-source/src/command.rs`: command spec and command runner.
- Create `crates/hw-source/src/files.rs`: file read and glob helpers.
- Create `crates/hw-source/src/result.rs`: `SourceResult`, `SourceErrorKind`.
- Create `crates/hw-source/src/runner.rs`: `SourceRunner` trait, `RealSourceRunner`, `FakeSourceRunner`.

### `crates/hw-parser`

- Modify `crates/hw-parser/Cargo.toml`.
- Replace `crates/hw-parser/src/lib.rs` with module exports.
- Create `crates/hw-parser/src/util.rs`: parser helpers.
- Create `crates/hw-parser/src/pci.rs`: `parse_lspci_nn_k`.
- Create `crates/hw-parser/src/usb.rs`: `parse_lsusb` and sysfs USB record helpers.
- Create `crates/hw-parser/src/input.rs`: `parse_proc_bus_input_devices`.
- Create `crates/hw-parser/src/audio.rs`: `parse_proc_asound_cards`, `parse_proc_asound_modules`, simple `hwinfo --sound` parser.
- Create `crates/hw-parser/src/power.rs`: `parse_upower_dump`, sysfs power-supply helpers.
- Create `crates/hw-parser/src/printer.rs`: `parse_lpstat_a`, `parse_lpstat_v`.
- Create `crates/hw-parser/src/cdrom.rs`: `parse_proc_cdrom_info`.
- Create `crates/hw-parser/src/bluetooth.rs`: `parse_hciconfig`, `parse_bluetoothctl_paired_devices`.
- Create `crates/hw-parser/src/video.rs`: `parse_v4l2_list_devices`.

### `crates/hw-probe`

- Create `crates/hw-probe/Cargo.toml`.
- Create `crates/hw-probe/src/lib.rs`.
- Create `crates/hw-probe/src/context.rs`: `ProbeContext`, shared indexes.
- Create `crates/hw-probe/src/result.rs`: `ProbeResult`.
- Create `crates/hw-probe/src/traits.rs`: `Probe` trait.
- Create `crates/hw-probe/src/pci.rs`, `usb.rs`, `audio.rs`, `bluetooth.rs`, `input.rs`, `camera.rs`, `battery.rs`, `printer.rs`, `cdrom.rs`, `existing.rs`, `other.rs`.

### `crates/hw-collect`

- Modify `crates/hw-collect/Cargo.toml`.
- Replace `crates/hw-collect/src/lib.rs`.
- Create `crates/hw-collect/src/collector.rs`: `collect_scan_report`.
- Create `crates/hw-collect/src/merge.rs`: dedup, parent/child association, fallback generation.
- Create `crates/hw-collect/src/status.rs`: report status calculation.

### `crates/hw-output`

- Create `crates/hw-output/Cargo.toml`.
- Create `crates/hw-output/src/lib.rs`.
- Create `crates/hw-output/src/flat.rs`: `FlatScanReportView`, `FlatDeviceView`, conversion from `ScanReport`.
- Create `crates/hw-output/src/jsonl.rs`: JSONL rendering.
- Create `crates/hw-output/src/summary.rs`: summary text and summary JSON.
- Create `crates/hw-output/src/table.rs`: table rendering.
- Create `crates/hw-output/src/schema.rs`: schema version and kind list.

### `crates/hw-cli`

- Create `crates/hw-cli/Cargo.toml`.
- Create `crates/hw-cli/src/main.rs`.
- Create `crates/hw-cli/src/args.rs`.
- Create `crates/hw-cli/src/exit.rs`.

### `crates/hw-testdata`

- Create `crates/hw-testdata/Cargo.toml`.
- Create `crates/hw-testdata/src/lib.rs`.
- Create fixture files under `crates/hw-testdata/fixtures/` as each parser task needs them.

---

### Task 1: Workspace Reshape and Crate Skeleton

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `crates/hw-probe/Cargo.toml`
- Create: `crates/hw-probe/src/lib.rs`
- Create: `crates/hw-output/Cargo.toml`
- Create: `crates/hw-output/src/lib.rs`
- Create: `crates/hw-cli/Cargo.toml`
- Create: `crates/hw-cli/src/main.rs`
- Create: `crates/hw-testdata/Cargo.toml`
- Create: `crates/hw-testdata/src/lib.rs`

**Interfaces:**
- Consumes: current Cargo workspace.
- Produces: buildable empty crate graph with public placeholders: `hw_collect::collect_scan_report`, `hw_output::schema_version`, and `qurbrix_hw::schema_version`.

- [ ] **Step 1: Write the failing facade test**

Create `tests/facade_exports.rs`:

```rust
#[test]
fn facade_exports_schema_version() {
    assert_eq!(qurbrix_hw::schema_version(), "qurbrix.hw.scan.v1");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test facade_exports
```

Expected: FAIL because `qurbrix_hw::schema_version` is not defined.

- [ ] **Step 3: Update root workspace manifest**

Replace the root `Cargo.toml` with:

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT OR Apache-2.0"
publish = false

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
clap = { version = "4", features = ["derive"] }
insta = "1"
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[package]
name = "qurbrix-hw"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true
autobins = false

[lib]
path = "src/lib.rs"

[[bin]]
name = "qurbrix-hw"
path = "crates/hw-cli/src/main.rs"

[dependencies]
anyhow = { workspace = true }
hw-collect = { path = "crates/hw-collect" }
hw-model = { path = "crates/hw-model" }
hw-output = { path = "crates/hw-output" }
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 4: Create new crate manifests and minimal source files**

Create `crates/hw-probe/Cargo.toml`:

```toml
[package]
name = "hw-probe"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow = { workspace = true }
async-trait = { workspace = true }
hw-model = { path = "../hw-model" }
hw-parser = { path = "../hw-parser" }
hw-source = { path = "../hw-source" }
serde = { workspace = true }
```

Create `crates/hw-probe/src/lib.rs`:

```rust
pub fn crate_ready() -> bool {
    true
}
```

Create `crates/hw-output/Cargo.toml`:

```toml
[package]
name = "hw-output"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow = { workspace = true }
hw-model = { path = "../hw-model" }
serde = { workspace = true }
serde_json = { workspace = true }
```

Create `crates/hw-output/src/lib.rs`:

```rust
pub fn schema_version() -> &'static str {
    "qurbrix.hw.scan.v1"
}
```

Create `crates/hw-cli/Cargo.toml`:

```toml
[package]
name = "hw-cli"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow = { workspace = true }
clap = { workspace = true }
hw-collect = { path = "../hw-collect" }
hw-model = { path = "../hw-model" }
hw-output = { path = "../hw-output" }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

Create `crates/hw-cli/src/main.rs`:

```rust
fn main() {
    println!("{}", hw_output::schema_version());
}
```

Create `crates/hw-testdata/Cargo.toml`:

```toml
[package]
name = "hw-testdata"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
```

Create `crates/hw-testdata/src/lib.rs`:

```rust
use std::path::{Path, PathBuf};

pub fn fixture_path(relative: impl AsRef<Path>) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(relative)
}

pub fn fixture(relative: impl AsRef<Path>) -> String {
    std::fs::read_to_string(fixture_path(relative)).expect("fixture exists")
}
```

- [ ] **Step 5: Replace facade**

Replace `src/lib.rs` with:

```rust
pub use hw_collect::collect_scan_report;
pub use hw_model::*;
pub use hw_output::schema_version;
```

- [ ] **Step 6: Add temporary collector symbol**

Replace `crates/hw-collect/Cargo.toml` with:

```toml
[package]
name = "hw-collect"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow = { workspace = true }
hw-model = { path = "../hw-model" }
hw-probe = { path = "../hw-probe" }
tokio = { workspace = true }
```

Replace `crates/hw-collect/src/lib.rs` with:

```rust
pub async fn collect_scan_report() -> anyhow::Result<hw_model::ScanReport> {
    Ok(hw_model::ScanReport::empty())
}
```

This uses `ScanReport::empty()` that Task 2 will define. To keep this task compiling before Task 2, add this temporary minimal model in `crates/hw-model/src/lib.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
}

impl ScanReport {
    pub fn empty() -> Self {
        Self {
            schema_version: "qurbrix.hw.scan.v1".to_string(),
        }
    }
}
```

Ensure `crates/hw-model/Cargo.toml` has:

```toml
[package]
name = "hw-model"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test --test facade_exports
cargo check --workspace
```

Expected: PASS for the test and successful workspace check.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml src/lib.rs crates/hw-probe crates/hw-output crates/hw-cli crates/hw-testdata crates/hw-collect crates/hw-model tests/facade_exports.rs
git commit -m "chore: scaffold scanner workspace"
```

---

### Task 2: Strongly Typed `hw-model`

**Files:**
- Replace: `crates/hw-model/src/lib.rs`
- Create: `crates/hw-model/src/kind.rs`
- Create: `crates/hw-model/src/report.rs`
- Create: `crates/hw-model/src/device.rs`
- Create: `crates/hw-model/src/bus.rs`
- Create: `crates/hw-model/src/driver.rs`
- Create: `crates/hw-model/src/evidence.rs`
- Create: `crates/hw-model/src/properties.rs`
- Create: `crates/hw-model/src/id.rs`
- Test: `crates/hw-model/tests/model_serialization.rs`

**Interfaces:**
- Consumes: skeleton `hw-model` from Task 1.
- Produces: `ScanReport`, `Device`, `DeviceKind`, `DeviceProperties`, `BusInfo`, `DriverInfo`, `SourceEvidence`, `ScanWarning`, `ScanConfig`, and ID helpers used by all later tasks.

- [ ] **Step 1: Write failing model serialization tests**

Create `crates/hw-model/tests/model_serialization.rs`:

```rust
use hw_model::{
    device_id, BusInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus,
    PciInfo, ScanReport, SourceEvidence, SourceKind, SourceStatus,
};

#[test]
fn device_kind_serializes_as_kebab_case() {
    assert_eq!(serde_json::to_string(&DeviceKind::OtherPci).unwrap(), "\"other-pci\"");
    assert_eq!("other-device".parse::<DeviceKind>().unwrap(), DeviceKind::OtherDevice);
}

#[test]
fn pci_device_has_stable_flat_fields() {
    let device = Device::new(
        device_id::pci("0000:00:1f.3"),
        DeviceKind::Pci,
        "Intel HD Audio Controller",
        DeviceProperties::Pci(PciInfo {
            address: "0000:00:1f.3".to_string(),
            class_name: Some("Audio device".to_string()),
            class_id: Some("0403".to_string()),
            vendor: Some("Intel Corporation".to_string()),
            vendor_id: Some("8086".to_string()),
            device: Some("HD Audio Controller".to_string()),
            device_id: Some("a348".to_string()),
            subsystem_vendor_id: None,
            subsystem_device_id: None,
        }),
    )
    .with_bus(BusInfo::Pci {
        address: "0000:00:1f.3".to_string(),
        vendor_id: Some("8086".to_string()),
        device_id: Some("a348".to_string()),
        subsystem_vendor_id: None,
        subsystem_device_id: None,
        class: Some("0403".to_string()),
    })
    .with_driver(DriverInfo {
        name: Some("snd_hda_intel".to_string()),
        version: None,
        modules: vec!["snd_hda_intel".to_string()],
        provider: None,
        status: DriverStatus::InUse,
    })
    .with_source(SourceEvidence {
        source: "lspci -nn -k".to_string(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    });

    let json = serde_json::to_value(device).unwrap();
    assert_eq!(json["kind"], "pci");
    assert_eq!(json["driver"]["status"], "in_use");
    assert_eq!(json["sources"][0]["status"], "success");
}

#[test]
fn empty_report_uses_schema_v1() {
    let report = ScanReport::empty();
    assert_eq!(report.schema_version, "qurbrix.hw.scan.v1");
    assert_eq!(report.devices.len(), 0);
    assert_eq!(serde_json::to_value(report.status).unwrap(), "complete");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-model --test model_serialization
```

Expected: FAIL because the new modules and types are not defined.

- [ ] **Step 3: Implement module exports**

Replace `crates/hw-model/src/lib.rs` with:

```rust
pub mod bus;
pub mod device;
pub mod driver;
pub mod evidence;
pub mod id;
pub mod kind;
pub mod properties;
pub mod report;

pub use bus::*;
pub use device::*;
pub use driver::*;
pub use evidence::*;
pub use id as device_id;
pub use kind::*;
pub use properties::*;
pub use report::*;
```

- [ ] **Step 4: Implement `DeviceKind`**

Create `crates/hw-model/src/kind.rs`:

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl DeviceKind {
    pub const ALL: &'static [DeviceKind] = &[
        DeviceKind::System,
        DeviceKind::Motherboard,
        DeviceKind::Bios,
        DeviceKind::Cpu,
        DeviceKind::Memory,
        DeviceKind::Storage,
        DeviceKind::Gpu,
        DeviceKind::Monitor,
        DeviceKind::Network,
        DeviceKind::Audio,
        DeviceKind::Bluetooth,
        DeviceKind::Input,
        DeviceKind::Camera,
        DeviceKind::Battery,
        DeviceKind::Printer,
        DeviceKind::Cdrom,
        DeviceKind::Usb,
        DeviceKind::Pci,
        DeviceKind::OtherPci,
        DeviceKind::OtherDevice,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::System => "system",
            DeviceKind::Motherboard => "motherboard",
            DeviceKind::Bios => "bios",
            DeviceKind::Cpu => "cpu",
            DeviceKind::Memory => "memory",
            DeviceKind::Storage => "storage",
            DeviceKind::Gpu => "gpu",
            DeviceKind::Monitor => "monitor",
            DeviceKind::Network => "network",
            DeviceKind::Audio => "audio",
            DeviceKind::Bluetooth => "bluetooth",
            DeviceKind::Input => "input",
            DeviceKind::Camera => "camera",
            DeviceKind::Battery => "battery",
            DeviceKind::Printer => "printer",
            DeviceKind::Cdrom => "cdrom",
            DeviceKind::Usb => "usb",
            DeviceKind::Pci => "pci",
            DeviceKind::OtherPci => "other-pci",
            DeviceKind::OtherDevice => "other-device",
        }
    }
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DeviceKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "system" => Ok(DeviceKind::System),
            "motherboard" => Ok(DeviceKind::Motherboard),
            "bios" => Ok(DeviceKind::Bios),
            "cpu" => Ok(DeviceKind::Cpu),
            "memory" => Ok(DeviceKind::Memory),
            "storage" => Ok(DeviceKind::Storage),
            "gpu" => Ok(DeviceKind::Gpu),
            "monitor" => Ok(DeviceKind::Monitor),
            "network" => Ok(DeviceKind::Network),
            "audio" => Ok(DeviceKind::Audio),
            "bluetooth" => Ok(DeviceKind::Bluetooth),
            "input" => Ok(DeviceKind::Input),
            "camera" => Ok(DeviceKind::Camera),
            "battery" => Ok(DeviceKind::Battery),
            "printer" => Ok(DeviceKind::Printer),
            "cdrom" => Ok(DeviceKind::Cdrom),
            "usb" => Ok(DeviceKind::Usb),
            "pci" => Ok(DeviceKind::Pci),
            "other-pci" => Ok(DeviceKind::OtherPci),
            "other-device" => Ok(DeviceKind::OtherDevice),
            other => Err(format!("unsupported device kind: {other}")),
        }
    }
}

impl Serialize for DeviceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DeviceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}
```

- [ ] **Step 5: Implement bus and driver types**

Create `crates/hw-model/src/bus.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
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
```

Create `crates/hw-model/src/driver.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub modules: Vec<String>,
    pub provider: Option<String>,
    pub status: DriverStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriverStatus {
    InUse,
    Available,
    Missing,
    Unknown,
}
```

- [ ] **Step 6: Implement evidence and warnings**

Create `crates/hw-model/src/evidence.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub source: String,
    pub kind: SourceKind,
    pub status: SourceStatus,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    Command,
    File,
    Directory,
    Sysfs,
    Procfs,
    Dbus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Success,
    Missing,
    PermissionDenied,
    Timeout,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanWarning {
    pub code: String,
    pub message: String,
    pub source: Option<String>,
    pub device_id: Option<String>,
}

impl ScanWarning {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            source: None,
            device_id: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }
}
```

- [ ] **Step 7: Implement properties**

Create `crates/hw-model/src/properties.rs` with the following complete first-pass structs:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SystemDeviceInfo {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MotherboardInfo {
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub serial: Option<String>,
    pub asset_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BiosInfo {
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub release_date: Option<String>,
    pub firmware_type: Option<String>,
    pub secure_boot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CpuInfo {
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub architecture: Option<String>,
    pub cores: Option<u32>,
    pub threads: Option<u32>,
    pub sockets: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    pub min_freq_mhz: Option<u32>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryInfo {
    pub size_bytes: Option<u64>,
    pub vendor: Option<String>,
    pub memory_type: Option<String>,
    pub speed_mtps: Option<u32>,
    pub locator: Option<String>,
    pub serial: Option<String>,
    pub part_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StorageInfo {
    pub device_node: Option<String>,
    pub size_bytes: Option<u64>,
    pub media_type: Option<String>,
    pub firmware: Option<String>,
    pub wwn: Option<String>,
    pub smart_status: Option<String>,
    pub temperature_celsius: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    pub memory_bytes: Option<u64>,
    pub current_resolution: Option<String>,
    pub max_resolution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MonitorInfo {
    pub connector: Option<String>,
    pub resolution: Option<String>,
    pub size_mm: Option<(u32, u32)>,
    pub production_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NetworkInfo {
    pub interface: Option<String>,
    pub mac: Option<String>,
    pub operstate: Option<String>,
    pub speed_mbps: Option<u32>,
    pub duplex: Option<String>,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioInfo {
    pub card_index: Option<u32>,
    pub card_name: Option<String>,
    pub codec: Option<String>,
    pub subsystem: Option<String>,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BluetoothInfo {
    pub address: Option<String>,
    pub controller_name: Option<String>,
    pub powered: Option<bool>,
    pub discoverable: Option<bool>,
    pub paired_device_count: Option<u32>,
    pub paired_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InputInfo {
    pub input_kind: InputKind,
    pub event_node: Option<String>,
    pub phys: Option<String>,
    pub uniq: Option<String>,
    pub handlers: Vec<String>,
    pub bus_type: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InputKind {
    Keyboard,
    Mouse,
    Touchpad,
    Touchscreen,
    Tablet,
    #[default]
    UnknownInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CameraInfo {
    pub video_node: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BatteryInfo {
    pub power_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub technology: Option<String>,
    pub state: Option<String>,
    pub capacity_percent: Option<f32>,
    pub energy_full_wh: Option<f32>,
    pub energy_design_wh: Option<f32>,
    pub energy_now_wh: Option<f32>,
    pub voltage_v: Option<f32>,
    pub cycle_count: Option<u32>,
    pub present: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrinterInfo {
    pub queue_name: Option<String>,
    pub accepting: Option<bool>,
    pub device_uri: Option<String>,
    pub make_model: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CdromInfo {
    pub device_node: Option<String>,
    pub media_present: Option<bool>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UsbInfo {
    pub bus_number: Option<String>,
    pub device_number: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub class: Option<String>,
    pub subclass: Option<String>,
    pub protocol: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub speed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PciInfo {
    pub address: String,
    pub class_name: Option<String>,
    pub class_id: Option<String>,
    pub vendor: Option<String>,
    pub vendor_id: Option<String>,
    pub device: Option<String>,
    pub device_id: Option<String>,
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OtherPciInfo {
    pub original_class: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OtherDeviceInfo {
    pub original_kind: Option<String>,
    pub reason: String,
}
```

- [ ] **Step 8: Implement `Device`, report, config, and ID helpers**

Create `crates/hw-model/src/device.rs`:

```rust
use crate::{BusInfo, DeviceKind, DeviceProperties, DriverInfo, ScanWarning, SourceEvidence};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentifier {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceRef {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl Device {
    pub fn new(
        id: impl Into<String>,
        kind: DeviceKind,
        name: impl Into<String>,
        properties: DeviceProperties,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            name: name.into(),
            vendor: None,
            model: None,
            serial: None,
            bus: None,
            driver: None,
            parent_id: None,
            children: Vec::new(),
            capabilities: Vec::new(),
            identifiers: Vec::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
            properties,
        }
    }

    pub fn with_bus(mut self, bus: BusInfo) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn with_driver(mut self, driver: DriverInfo) -> Self {
        self.driver = Some(driver);
        self
    }

    pub fn with_source(mut self, source: SourceEvidence) -> Self {
        self.sources.push(source);
        self
    }
}
```

Create `crates/hw-model/src/report.rs`:

```rust
use crate::{Device, DeviceKind, ScanWarning};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const SCHEMA_VERSION: &str = "qurbrix.hw.scan.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
    pub metadata: ScanMetadata,
    pub system: SystemInfo,
    pub devices: Vec<Device>,
    pub warnings: Vec<ScanWarning>,
    pub status: ScanStatus,
}

impl ScanReport {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            metadata: ScanMetadata::default(),
            system: SystemInfo::default(),
            devices: Vec::new(),
            warnings: Vec::new(),
            status: ScanStatus::Complete,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScanMetadata {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
    pub scanner_version: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Complete,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    pub kinds: Option<Vec<DeviceKind>>,
    pub exclude_kinds: Vec<DeviceKind>,
    pub timeout: Duration,
    pub optional_sources: bool,
    pub include_sources: bool,
    pub include_warnings: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            kinds: None,
            exclude_kinds: Vec::new(),
            timeout: Duration::from_secs(30),
            optional_sources: true,
            include_sources: true,
            include_warnings: true,
        }
    }
}
```

Create `crates/hw-model/src/id.rs`:

```rust
pub fn pci(address: &str) -> String {
    format!("pci:{}", address.trim())
}

pub fn usb(bus: Option<&str>, device: Option<&str>, vendor_id: Option<&str>, product_id: Option<&str>, serial: Option<&str>) -> String {
    if let (Some(vendor_id), Some(product_id), Some(serial)) = (vendor_id, product_id, serial) {
        let serial = serial.trim();
        if !serial.is_empty() {
            return format!("usb:{}:{}:{}", vendor_id.trim(), product_id.trim(), serial);
        }
    }
    format!("usb:{}:{}", bus.unwrap_or("unknown").trim(), device.unwrap_or("unknown").trim())
}

pub fn network(mac: Option<&str>, iface: &str) -> String {
    match mac.map(str::trim).filter(|v| !v.is_empty()) {
        Some(mac) => format!("net:mac:{}", mac),
        None => format!("net:iface:{}", iface.trim()),
    }
}

pub fn storage(wwn: Option<&str>, serial: Option<&str>, node: &str) -> String {
    if let Some(wwn) = wwn.map(str::trim).filter(|v| !v.is_empty()) {
        return format!("storage:wwn:{}", wwn);
    }
    if let Some(serial) = serial.map(str::trim).filter(|v| !v.is_empty()) {
        return format!("storage:serial:{}", serial);
    }
    format!("storage:dev:{}", node.trim())
}

pub fn battery(name: &str) -> String {
    format!("battery:{}", name.trim())
}

pub fn input_event(event: &str) -> String {
    format!("input:event:{}", event.trim())
}

pub fn camera(video_node: &str) -> String {
    format!("camera:{}", video_node.trim())
}

pub fn printer(queue: &str) -> String {
    format!("printer:{}", queue.trim())
}

pub fn other(prefix: &str, value: &str) -> String {
    format!("{}:{}", prefix.trim(), value.trim())
}
```

- [ ] **Step 9: Run tests**

Run:

```bash
cargo test -p hw-model --test model_serialization
cargo test -p hw-model
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/hw-model
git commit -m "feat: add scanner data model"
```

---

### Task 3: Source Runner with Real and Fake Implementations

**Files:**
- Replace: `crates/hw-source/src/lib.rs`
- Create: `crates/hw-source/src/command.rs`
- Create: `crates/hw-source/src/files.rs`
- Create: `crates/hw-source/src/result.rs`
- Create: `crates/hw-source/src/runner.rs`
- Test: `crates/hw-source/tests/fake_runner.rs`

**Interfaces:**
- Consumes: `hw_model::SourceKind`, `SourceStatus`, `ScanWarning` concepts indirectly through compatible source statuses.
- Produces: `SourceRunner` trait used by probes and collector.

- [ ] **Step 1: Write failing fake runner tests**

Create `crates/hw-source/tests/fake_runner.rs`:

```rust
use hw_source::{CommandSpec, FakeSourceRunner, SourceErrorKind, SourceRunner};
use std::{path::Path, time::Duration};

#[tokio::test]
async fn fake_runner_returns_registered_command_output() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:1f.3 Audio device [0403]: Intel [8086:a348]\n");

    let result = runner
        .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), Duration::from_secs(1))
        .await;

    assert!(result.is_success());
    assert!(result.stdout.contains("Audio device"));
}

#[tokio::test]
async fn fake_runner_reports_missing_file() {
    let runner = FakeSourceRunner::new();
    let result = runner.read_file(Path::new("/sys/missing")).await;
    assert_eq!(result.error_kind, Some(SourceErrorKind::Missing));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-source --test fake_runner
```

Expected: FAIL because source runner types are not defined.

- [ ] **Step 3: Implement source modules**

Replace `crates/hw-source/Cargo.toml` with:

```toml
[package]
name = "hw-source"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow = { workspace = true }
async-trait = { workspace = true }
tokio = { workspace = true }
```

Replace `crates/hw-source/src/lib.rs` with:

```rust
pub mod command;
pub mod files;
pub mod result;
pub mod runner;

pub use command::*;
pub use files::*;
pub use result::*;
pub use runner::*;
```

Create `crates/hw-source/src/command.rs`:

```rust
use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub fn display_name(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }

    pub fn os_args(&self) -> Vec<OsString> {
        self.args.iter().map(OsString::from).collect()
    }
}
```

Create `crates/hw-source/src/result.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceErrorKind {
    Missing,
    PermissionDenied,
    Timeout,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResult {
    pub source: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_status: Option<i32>,
    pub error_kind: Option<SourceErrorKind>,
}

impl SourceResult {
    pub fn success(source: impl Into<String>, stdout: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            stdout: stdout.into(),
            stderr: String::new(),
            exit_status: Some(0),
            error_kind: None,
        }
    }

    pub fn error(source: impl Into<String>, kind: SourceErrorKind, stderr: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            stdout: String::new(),
            stderr: stderr.into(),
            exit_status: None,
            error_kind: Some(kind),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error_kind.is_none() && self.exit_status == Some(0)
    }
}
```

Create `crates/hw-source/src/files.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobResult {
    pub pattern: String,
    pub paths: Vec<PathBuf>,
}
```

Create `crates/hw-source/src/runner.rs`:

```rust
use crate::{CommandSpec, GlobResult, SourceErrorKind, SourceResult};
use async_trait::async_trait;
use std::{collections::HashMap, path::{Path, PathBuf}, time::Duration};
use tokio::{fs, process::Command, time};

#[async_trait]
pub trait SourceRunner: Send + Sync {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult;
    async fn read_file(&self, path: &Path) -> SourceResult;
    async fn glob(&self, pattern: &str) -> GlobResult;
}

#[derive(Debug, Default)]
pub struct RealSourceRunner;

#[async_trait]
impl SourceRunner for RealSourceRunner {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        let display = command.display_name();
        let mut cmd = Command::new(&command.program);
        cmd.args(&command.args);
        match time::timeout(timeout, cmd.output()).await {
            Ok(Ok(output)) => SourceResult {
                source: display,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_status: output.status.code(),
                error_kind: if output.status.success() { None } else { Some(SourceErrorKind::Failed) },
            },
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                SourceResult::error(display, SourceErrorKind::Missing, err.to_string())
            }
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                SourceResult::error(display, SourceErrorKind::PermissionDenied, err.to_string())
            }
            Ok(Err(err)) => SourceResult::error(display, SourceErrorKind::Failed, err.to_string()),
            Err(_) => SourceResult::error(display, SourceErrorKind::Timeout, "command timed out"),
        }
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        let source = path.display().to_string();
        match fs::read_to_string(path).await {
            Ok(text) => SourceResult::success(source, text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                SourceResult::error(source, SourceErrorKind::Missing, err.to_string())
            }
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                SourceResult::error(source, SourceErrorKind::PermissionDenied, err.to_string())
            }
            Err(err) => SourceResult::error(source, SourceErrorKind::Failed, err.to_string()),
        }
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        let prefix = pattern.trim_end_matches('*');
        let dir = Path::new(prefix).parent().unwrap_or_else(|| Path::new(prefix));
        let mut paths = Vec::new();
        if let Ok(mut entries) = fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                paths.push(entry.path());
            }
        }
        GlobResult { pattern: pattern.to_string(), paths }
    }
}

#[derive(Debug, Default, Clone)]
pub struct FakeSourceRunner {
    commands: HashMap<CommandSpec, SourceResult>,
    files: HashMap<PathBuf, SourceResult>,
    globs: HashMap<String, Vec<PathBuf>>,
}

impl FakeSourceRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_command(mut self, program: impl Into<String>, args: impl IntoIterator<Item = impl Into<String>>, stdout: impl Into<String>) -> Self {
        let spec = CommandSpec::new(program, args);
        self.commands.insert(spec.clone(), SourceResult::success(spec.display_name(), stdout));
        self
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>, contents: impl Into<String>) -> Self {
        let path = path.into();
        self.files.insert(path.clone(), SourceResult::success(path.display().to_string(), contents));
        self
    }

    pub fn with_glob(mut self, pattern: impl Into<String>, paths: Vec<PathBuf>) -> Self {
        self.globs.insert(pattern.into(), paths);
        self
    }
}

#[async_trait]
impl SourceRunner for FakeSourceRunner {
    async fn run_command(&self, command: &CommandSpec, _timeout: Duration) -> SourceResult {
        self.commands
            .get(command)
            .cloned()
            .unwrap_or_else(|| SourceResult::error(command.display_name(), SourceErrorKind::Missing, "fake command not registered"))
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        self.files
            .get(path)
            .cloned()
            .unwrap_or_else(|| SourceResult::error(path.display().to_string(), SourceErrorKind::Missing, "fake file not registered"))
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        GlobResult {
            pattern: pattern.to_string(),
            paths: self.globs.get(pattern).cloned().unwrap_or_default(),
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p hw-source --test fake_runner
cargo test -p hw-source
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/hw-source
git commit -m "feat: add hardware source runner"
```

---

### Task 4: PCI and USB Parsers

**Files:**
- Modify: `crates/hw-parser/Cargo.toml`
- Replace: `crates/hw-parser/src/lib.rs`
- Create: `crates/hw-parser/src/util.rs`
- Create: `crates/hw-parser/src/pci.rs`
- Create: `crates/hw-parser/src/usb.rs`
- Create: `crates/hw-testdata/fixtures/pci/lspci-nn-k.txt`
- Create: `crates/hw-testdata/fixtures/usb/lsusb.txt`
- Test: `crates/hw-parser/tests/pci_usb.rs`

**Interfaces:**
- Consumes: fixture helper from `hw-testdata`.
- Produces: `parse_lspci_nn_k(input: &str) -> Vec<PciRecord>` and `parse_lsusb(input: &str) -> Vec<UsbRecord>`.

- [ ] **Step 1: Create fixtures**

Create `crates/hw-testdata/fixtures/pci/lspci-nn-k.txt`:

```text
00:1f.3 Audio device [0403]: Intel Corporation Cannon Lake PCH cAVS [8086:a348] (rev 10)
	Subsystem: Lenovo Device [17aa:2292]
	Kernel driver in use: snd_hda_intel
	Kernel modules: snd_hda_intel, snd_soc_avs
02:00.0 Network controller [0280]: Intel Corporation Wireless-AC 9560 [8086:a370]
	Subsystem: Intel Corporation Device [8086:0034]
	Kernel driver in use: iwlwifi
	Kernel modules: iwlwifi
```

Create `crates/hw-testdata/fixtures/usb/lsusb.txt`:

```text
Bus 001 Device 004: ID 0bda:5689 Realtek Semiconductor Corp. Integrated Camera
Bus 001 Device 005: ID 8087:0aaa Intel Corp. Bluetooth 9460/9560 Jefferson Peak (JfP)
```

- [ ] **Step 2: Write failing parser tests**

Create `crates/hw-parser/tests/pci_usb.rs`:

```rust
use hw_parser::{parse_lspci_nn_k, parse_lsusb};

#[test]
fn parses_lspci_driver_and_modules() {
    let input = hw_testdata::fixture("pci/lspci-nn-k.txt");
    let records = parse_lspci_nn_k(&input);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].address, "0000:00:1f.3");
    assert_eq!(records[0].class_name.as_deref(), Some("Audio device"));
    assert_eq!(records[0].class_id.as_deref(), Some("0403"));
    assert_eq!(records[0].vendor_id.as_deref(), Some("8086"));
    assert_eq!(records[0].device_id.as_deref(), Some("a348"));
    assert_eq!(records[0].kernel_driver.as_deref(), Some("snd_hda_intel"));
    assert_eq!(records[0].kernel_modules, vec!["snd_hda_intel", "snd_soc_avs"]);
}

#[test]
fn parses_lsusb_basic_records() {
    let input = hw_testdata::fixture("usb/lsusb.txt");
    let records = parse_lsusb(&input);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].bus.as_deref(), Some("001"));
    assert_eq!(records[0].device.as_deref(), Some("004"));
    assert_eq!(records[0].vendor_id.as_deref(), Some("0bda"));
    assert_eq!(records[0].product_id.as_deref(), Some("5689"));
    assert!(records[0].product.as_deref().unwrap().contains("Integrated Camera"));
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
cargo test -p hw-parser --test pci_usb
```

Expected: FAIL because parser symbols are not defined.

- [ ] **Step 4: Implement parser crate**

Replace `crates/hw-parser/Cargo.toml` with:

```toml
[package]
name = "hw-parser"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
regex = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
hw-testdata = { path = "../hw-testdata" }
```

Replace `crates/hw-parser/src/lib.rs` with:

```rust
pub mod pci;
pub mod usb;
pub mod util;

pub use pci::*;
pub use usb::*;
```

Create `crates/hw-parser/src/util.rs`:

```rust
pub fn clean_hex(value: &str) -> String {
    value.trim().trim_start_matches("0x").to_ascii_lowercase()
}

pub fn split_csv_words(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
```

Create `crates/hw-parser/src/pci.rs`:

```rust
use crate::util::split_csv_words;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PciRecord {
    pub address: String,
    pub class_name: Option<String>,
    pub class_id: Option<String>,
    pub vendor: Option<String>,
    pub vendor_id: Option<String>,
    pub device: Option<String>,
    pub device_id: Option<String>,
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
    pub kernel_driver: Option<String>,
    pub kernel_modules: Vec<String>,
}

pub fn parse_lspci_nn_k(input: &str) -> Vec<PciRecord> {
    let header = Regex::new(r"^(?P<addr>[0-9a-fA-F:.]+)\s+(?P<class>.+?)\s+\[(?P<class_id>[0-9a-fA-F]{4})\]:\s+(?P<vendor>.+?)\s+\[(?P<vendor_id>[0-9a-fA-F]{4})\](?:\s+(?P<device>.+?)\s+\[(?P<device_id>[0-9a-fA-F]{4})\])?(?:\s+\(rev .+\))?$").unwrap();
    let subsystem = Regex::new(r"^\s*Subsystem:.*\[(?P<sub_vendor>[0-9a-fA-F]{4}):(?P<sub_device>[0-9a-fA-F]{4})\]").unwrap();
    let driver = Regex::new(r"^\s*Kernel driver in use:\s*(?P<driver>.+)$").unwrap();
    let modules = Regex::new(r"^\s*Kernel modules:\s*(?P<modules>.+)$").unwrap();

    let mut records = Vec::new();
    let mut current: Option<PciRecord> = None;

    for line in input.lines() {
        if let Some(caps) = header.captures(line) {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let mut address = caps["addr"].to_string();
            if !address.contains(':') || address.matches(':').count() == 1 {
                address = format!("0000:{address}");
            }
            current = Some(PciRecord {
                address,
                class_name: Some(caps["class"].trim().to_string()),
                class_id: Some(caps["class_id"].to_ascii_lowercase()),
                vendor: Some(caps["vendor"].trim().to_string()),
                vendor_id: Some(caps["vendor_id"].to_ascii_lowercase()),
                device: caps.name("device").map(|m| m.as_str().trim().to_string()),
                device_id: caps.name("device_id").map(|m| m.as_str().to_ascii_lowercase()),
                ..Default::default()
            });
            continue;
        }

        let Some(record) = current.as_mut() else { continue };
        if let Some(caps) = subsystem.captures(line) {
            record.subsystem_vendor_id = Some(caps["sub_vendor"].to_ascii_lowercase());
            record.subsystem_device_id = Some(caps["sub_device"].to_ascii_lowercase());
        } else if let Some(caps) = driver.captures(line) {
            record.kernel_driver = Some(caps["driver"].trim().to_string());
        } else if let Some(caps) = modules.captures(line) {
            record.kernel_modules = split_csv_words(&caps["modules"]);
        }
    }

    if let Some(record) = current.take() {
        records.push(record);
    }

    records
}
```

Create `crates/hw-parser/src/usb.rs`:

```rust
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsbRecord {
    pub bus: Option<String>,
    pub device: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub class: Option<String>,
    pub subclass: Option<String>,
    pub protocol: Option<String>,
    pub speed: Option<String>,
}

pub fn parse_lsusb(input: &str) -> Vec<UsbRecord> {
    let re = Regex::new(r"^Bus\s+(?P<bus>\d+)\s+Device\s+(?P<device>\d+):\s+ID\s+(?P<vid>[0-9a-fA-F]{4}):(?P<pid>[0-9a-fA-F]{4})\s*(?P<product>.*)$").unwrap();
    input
        .lines()
        .filter_map(|line| {
            let caps = re.captures(line)?;
            Some(UsbRecord {
                bus: Some(caps["bus"].to_string()),
                device: Some(caps["device"].to_string()),
                vendor_id: Some(caps["vid"].to_ascii_lowercase()),
                product_id: Some(caps["pid"].to_ascii_lowercase()),
                product: Some(caps["product"].trim().to_string()).filter(|v| !v.is_empty()),
                ..Default::default()
            })
        })
        .collect()
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p hw-parser --test pci_usb
cargo test -p hw-parser
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/hw-parser crates/hw-testdata
git commit -m "feat: parse pci and usb hardware sources"
```

---

### Task 5: Peripheral Parsers

**Files:**
- Modify: `crates/hw-parser/src/lib.rs`
- Create: `crates/hw-parser/src/input.rs`
- Create: `crates/hw-parser/src/audio.rs`
- Create: `crates/hw-parser/src/power.rs`
- Create: `crates/hw-parser/src/printer.rs`
- Create: `crates/hw-parser/src/cdrom.rs`
- Create: `crates/hw-parser/src/bluetooth.rs`
- Create: `crates/hw-parser/src/video.rs`
- Create fixtures under `crates/hw-testdata/fixtures/proc/`, `power/`, `printer/`, `bluetooth/`, `video/`
- Test: `crates/hw-parser/tests/peripherals.rs`

**Interfaces:**
- Consumes: `hw_parser::util` helpers.
- Produces parser functions for probes: `parse_proc_bus_input_devices`, `parse_proc_asound_cards`, `parse_upower_dump`, `parse_lpstat_a`, `parse_lpstat_v`, `parse_proc_cdrom_info`, `parse_hciconfig`, `parse_bluetoothctl_paired_devices`, `parse_v4l2_list_devices`.

- [ ] **Step 1: Create representative fixtures**

Create `crates/hw-testdata/fixtures/proc/bus-input-devices.txt`:

```text
I: Bus=0011 Vendor=0001 Product=0001 Version=ab41
N: Name="AT Translated Set 2 keyboard"
P: Phys=isa0060/serio0/input0
S: Sysfs=/devices/platform/i8042/serio0/input/input0
H: Handlers=sysrq kbd event0 leds
B: PROP=0
B: EV=120013

I: Bus=0003 Vendor=046d Product=c077 Version=0111
N: Name="Logitech USB Optical Mouse"
P: Phys=usb-0000:00:14.0-1/input0
H: Handlers=mouse0 event1
```

Create `crates/hw-testdata/fixtures/proc/asound-cards.txt`:

```text
 0 [PCH            ]: HDA-Intel - HDA Intel PCH
                      HDA Intel PCH at 0xa1230000 irq 145
```

Create `crates/hw-testdata/fixtures/power/upower-dump.txt`:

```text
Device: /org/freedesktop/UPower/devices/battery_BAT0
  native-path:          BAT0
  vendor:               LGC
  model:                LNV-5B10W139
  serial:               1234
  power supply:         yes
  updated:              Sat 04 Jul 2026 07:00:00 PM CST (10 seconds ago)
  has history:          yes
  has statistics:       yes
  battery
    present:             yes
    rechargeable:        yes
    state:               discharging
    energy:              42.1 Wh
    energy-full:         50.2 Wh
    energy-full-design:  57 Wh
    voltage:             11.52 V
    percentage:          83%
    capacity:            88.0702%
    technology:          lithium-polymer
```

Create `crates/hw-testdata/fixtures/printer/lpstat-a.txt`:

```text
Office_Printer accepting requests since Sat 04 Jul 2026 18:00:00 CST
PDF disabled since Sat 04 Jul 2026 18:01:00 CST - reason unknown
```

Create `crates/hw-testdata/fixtures/printer/lpstat-v.txt`:

```text
device for Office_Printer: ipp://printer.local/ipp/print
device for PDF: cups-pdf:/
```

Create `crates/hw-testdata/fixtures/proc/cdrom-info.txt`:

```text
CD-ROM information, Id: cdrom.c 3.20 2003/12/17

drive name:		sr0
drive speed:		24
Can close tray:		1
Can open tray:		1
Can lock tray:		1
Can read DVD:		1
Can write CD-R:		1
```

Create `crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt`:

```text
hci0:   Type: Primary  Bus: USB
        BD Address: 00:11:22:33:44:55  ACL MTU: 1021:4  SCO MTU: 96:6
        UP RUNNING PSCAN ISCAN
        Name: 'host-bluetooth'
```

Create `crates/hw-testdata/fixtures/bluetooth/paired-devices.txt`:

```text
Device AA:BB:CC:DD:EE:FF MX Master 3
Device 11:22:33:44:55:66 Keyboard K380
```

Create `crates/hw-testdata/fixtures/video/v4l2-list-devices.txt`:

```text
Integrated Camera: Integrated C (usb-0000:00:14.0-8):
	/dev/video0
	/dev/video1
```

- [ ] **Step 2: Write failing parser tests**

Create `crates/hw-parser/tests/peripherals.rs`:

```rust
use hw_parser::*;

#[test]
fn parses_input_devices() {
    let records = parse_proc_bus_input_devices(&hw_testdata::fixture("proc/bus-input-devices.txt"));
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].name.as_deref(), Some("AT Translated Set 2 keyboard"));
    assert_eq!(records[0].handlers, vec!["sysrq", "kbd", "event0", "leds"]);
    assert_eq!(records[1].vendor_id.as_deref(), Some("046d"));
}

#[test]
fn parses_asound_cards() {
    let cards = parse_proc_asound_cards(&hw_testdata::fixture("proc/asound-cards.txt"));
    assert_eq!(cards[0].index, 0);
    assert_eq!(cards[0].id.as_deref(), Some("PCH"));
    assert!(cards[0].name.as_deref().unwrap().contains("HDA Intel PCH"));
}

#[test]
fn parses_upower_battery() {
    let devices = parse_upower_dump(&hw_testdata::fixture("power/upower-dump.txt"));
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].native_path.as_deref(), Some("BAT0"));
    assert_eq!(devices[0].state.as_deref(), Some("discharging"));
    assert_eq!(devices[0].capacity_percent, Some(88.0702));
}

#[test]
fn parses_printer_status_and_uri() {
    let statuses = parse_lpstat_a(&hw_testdata::fixture("printer/lpstat-a.txt"));
    let uris = parse_lpstat_v(&hw_testdata::fixture("printer/lpstat-v.txt"));
    assert_eq!(statuses[0].queue, "Office_Printer");
    assert_eq!(statuses[0].accepting, true);
    assert_eq!(uris[0].device_uri.as_deref(), Some("ipp://printer.local/ipp/print"));
}

#[test]
fn parses_cdrom_capabilities() {
    let info = parse_proc_cdrom_info(&hw_testdata::fixture("proc/cdrom-info.txt"));
    assert_eq!(info.drive_names, vec!["sr0"]);
    assert!(info.capabilities.contains(&"read-dvd".to_string()));
}

#[test]
fn parses_bluetooth_and_video() {
    let controllers = parse_hciconfig(&hw_testdata::fixture("bluetooth/hciconfig-a.txt"));
    let paired = parse_bluetoothctl_paired_devices(&hw_testdata::fixture("bluetooth/paired-devices.txt"));
    let cameras = parse_v4l2_list_devices(&hw_testdata::fixture("video/v4l2-list-devices.txt"));
    assert_eq!(controllers[0].address.as_deref(), Some("00:11:22:33:44:55"));
    assert_eq!(paired.len(), 2);
    assert_eq!(cameras[0].nodes, vec!["/dev/video0", "/dev/video1"]);
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
cargo test -p hw-parser --test peripherals
```

Expected: FAIL because parser modules are missing.

- [ ] **Step 4: Implement parser module exports**

Append to `crates/hw-parser/src/lib.rs`:

```rust
pub mod audio;
pub mod bluetooth;
pub mod cdrom;
pub mod input;
pub mod power;
pub mod printer;
pub mod video;

pub use audio::*;
pub use bluetooth::*;
pub use cdrom::*;
pub use input::*;
pub use power::*;
pub use printer::*;
pub use video::*;
```

- [ ] **Step 5: Implement compact peripheral parsers**

Create `crates/hw-parser/src/input.rs`:

```rust
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InputRecord {
    pub bus: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub version: Option<String>,
    pub name: Option<String>,
    pub phys: Option<String>,
    pub uniq: Option<String>,
    pub handlers: Vec<String>,
}

pub fn parse_proc_bus_input_devices(input: &str) -> Vec<InputRecord> {
    let id_re = Regex::new(r"Bus=(\S+)\s+Vendor=(\S+)\s+Product=(\S+)\s+Version=(\S+)").unwrap();
    let mut records = Vec::new();
    let mut current = InputRecord::default();
    let mut seen = false;

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if seen {
                records.push(current);
                current = InputRecord::default();
                seen = false;
            }
            continue;
        }
        seen = true;
        if let Some(rest) = line.strip_prefix("I: ") {
            if let Some(caps) = id_re.captures(rest) {
                current.bus = Some(caps[1].to_string());
                current.vendor_id = Some(caps[2].to_ascii_lowercase());
                current.product_id = Some(caps[3].to_ascii_lowercase());
                current.version = Some(caps[4].to_string());
            }
        } else if let Some(rest) = line.strip_prefix("N: Name=") {
            current.name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = line.strip_prefix("P: Phys=") {
            current.phys = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("U: Uniq=") {
            current.uniq = Some(rest.to_string()).filter(|v| !v.is_empty());
        } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
            current.handlers = rest.split_whitespace().map(ToOwned::to_owned).collect();
        }
    }
    records
}
```

Create `crates/hw-parser/src/audio.rs`:

```rust
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsoundCardRecord {
    pub index: u32,
    pub id: Option<String>,
    pub name: Option<String>,
    pub detail: Option<String>,
}

pub fn parse_proc_asound_cards(input: &str) -> Vec<AsoundCardRecord> {
    let re = Regex::new(r"^\s*(\d+)\s+\[(.*?)\s*\]:\s*(.*?)\s+-\s+(.*)$").unwrap();
    let mut cards = Vec::new();
    let mut lines = input.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(caps) = re.captures(line) {
            let detail = lines.peek().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
            cards.push(AsoundCardRecord {
                index: caps[1].parse().unwrap_or(0),
                id: Some(caps[2].trim().to_string()),
                name: Some(caps[4].trim().to_string()),
                detail,
            });
        }
    }
    cards
}
```

Create `crates/hw-parser/src/power.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PowerRecord {
    pub native_path: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub state: Option<String>,
    pub technology: Option<String>,
    pub capacity_percent: Option<f32>,
    pub energy_full_wh: Option<f32>,
    pub energy_design_wh: Option<f32>,
    pub energy_now_wh: Option<f32>,
    pub voltage_v: Option<f32>,
    pub present: Option<bool>,
}

pub fn parse_upower_dump(input: &str) -> Vec<PowerRecord> {
    let mut records = Vec::new();
    let mut current: Option<PowerRecord> = None;
    for line in input.lines() {
        if line.starts_with("Device: ") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            current = Some(PowerRecord::default());
            continue;
        }
        let Some(record) = current.as_mut() else { continue };
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim() {
                "native-path" => record.native_path = Some(value.to_string()),
                "vendor" => record.vendor = Some(value.to_string()),
                "model" => record.model = Some(value.to_string()),
                "serial" => record.serial = Some(value.to_string()),
                "present" => record.present = Some(value == "yes"),
                "state" => record.state = Some(value.to_string()),
                "technology" => record.technology = Some(value.to_string()),
                "capacity" => record.capacity_percent = parse_number(value),
                "energy-full" => record.energy_full_wh = parse_number(value),
                "energy-full-design" => record.energy_design_wh = parse_number(value),
                "energy" => record.energy_now_wh = parse_number(value),
                "voltage" => record.voltage_v = parse_number(value),
                _ => {}
            }
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}

fn parse_number(value: &str) -> Option<f32> {
    value.split_whitespace().next()?.trim_end_matches('%').parse().ok()
}
```

Create `crates/hw-parser/src/printer.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrinterStatusRecord {
    pub queue: String,
    pub accepting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrinterUriRecord {
    pub queue: String,
    pub device_uri: Option<String>,
}

pub fn parse_lpstat_a(input: &str) -> Vec<PrinterStatusRecord> {
    input.lines().filter_map(|line| {
        let mut parts = line.split_whitespace();
        let queue = parts.next()?.to_string();
        let state = parts.next()?;
        Some(PrinterStatusRecord { queue, accepting: state == "accepting" })
    }).collect()
}

pub fn parse_lpstat_v(input: &str) -> Vec<PrinterUriRecord> {
    input.lines().filter_map(|line| {
        let rest = line.strip_prefix("device for ")?;
        let (queue, uri) = rest.split_once(':')?;
        Some(PrinterUriRecord { queue: queue.trim().to_string(), device_uri: Some(uri.trim().to_string()) })
    }).collect()
}
```

Create `crates/hw-parser/src/cdrom.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CdromProcInfo {
    pub drive_names: Vec<String>,
    pub capabilities: Vec<String>,
}

pub fn parse_proc_cdrom_info(input: &str) -> CdromProcInfo {
    let mut info = CdromProcInfo::default();
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("drive name:") {
            info.drive_names = rest.split_whitespace().map(ToOwned::to_owned).collect();
        } else if line.starts_with("Can read DVD:") && line.ends_with('1') {
            info.capabilities.push("read-dvd".to_string());
        } else if line.starts_with("Can write CD-R:") && line.ends_with('1') {
            info.capabilities.push("write-cd-r".to_string());
        } else if line.starts_with("Can open tray:") && line.ends_with('1') {
            info.capabilities.push("open-tray".to_string());
        }
    }
    info
}
```

Create `crates/hw-parser/src/bluetooth.rs`:

```rust
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BluetoothControllerRecord {
    pub name: Option<String>,
    pub address: Option<String>,
    pub bus: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothPairedDeviceRecord {
    pub address: String,
    pub name: String,
}

pub fn parse_hciconfig(input: &str) -> Vec<BluetoothControllerRecord> {
    let address_re = Regex::new(r"BD Address:\s*([0-9A-Fa-f:]{17})").unwrap();
    let name_re = Regex::new(r"Name:\s*'(.+)'").unwrap();
    let mut records = Vec::new();
    let mut current: Option<BluetoothControllerRecord> = None;
    for line in input.lines() {
        if line.starts_with("hci") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let bus = line.split("Bus:").nth(1).map(|v| v.trim().to_string());
            current = Some(BluetoothControllerRecord { bus, ..Default::default() });
        } else if let Some(record) = current.as_mut() {
            if let Some(caps) = address_re.captures(line) {
                record.address = Some(caps[1].to_string());
            } else if let Some(caps) = name_re.captures(line) {
                record.name = Some(caps[1].to_string());
            } else {
                let flags: Vec<String> = line.split_whitespace().filter(|v| v.chars().all(|c| c.is_ascii_uppercase())).map(ToOwned::to_owned).collect();
                if !flags.is_empty() {
                    record.flags = flags;
                }
            }
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}

pub fn parse_bluetoothctl_paired_devices(input: &str) -> Vec<BluetoothPairedDeviceRecord> {
    input.lines().filter_map(|line| {
        let rest = line.strip_prefix("Device ")?;
        let (address, name) = rest.split_once(' ')?;
        Some(BluetoothPairedDeviceRecord { address: address.to_string(), name: name.to_string() })
    }).collect()
}
```

Create `crates/hw-parser/src/video.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VideoDeviceRecord {
    pub name: String,
    pub bus_hint: Option<String>,
    pub nodes: Vec<String>,
}

pub fn parse_v4l2_list_devices(input: &str) -> Vec<VideoDeviceRecord> {
    let mut records = Vec::new();
    let mut current: Option<VideoDeviceRecord> = None;
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') && line.ends_with(':') {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let header = line.trim_end_matches(':');
            let (name, bus_hint) = match header.rsplit_once('(') {
                Some((name, bus)) => (name.trim().to_string(), Some(bus.trim_end_matches(')').to_string())),
                None => (header.to_string(), None),
            };
            current = Some(VideoDeviceRecord { name, bus_hint, nodes: Vec::new() });
        } else if let Some(record) = current.as_mut() {
            let node = line.trim();
            if node.starts_with("/dev/video") {
                record.nodes.push(node.to_string());
            }
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p hw-parser --test peripherals
cargo test -p hw-parser
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/hw-parser crates/hw-testdata
git commit -m "feat: parse peripheral hardware sources"
```

---

### Task 6: PCI, USB, and Other Device Probes

**Files:**
- Replace: `crates/hw-probe/src/lib.rs`
- Create: `crates/hw-probe/src/context.rs`
- Create: `crates/hw-probe/src/result.rs`
- Create: `crates/hw-probe/src/traits.rs`
- Create: `crates/hw-probe/src/pci.rs`
- Create: `crates/hw-probe/src/usb.rs`
- Create: `crates/hw-probe/src/other.rs`
- Test: `crates/hw-probe/tests/base_probes.rs`

**Interfaces:**
- Consumes: `SourceRunner`, `parse_lspci_nn_k`, `parse_lsusb`, `Device` model.
- Produces: `PciProbe`, `UsbProbe`, `OtherDeviceBuilder`, `ProbeContext`, `ProbeResult`.

- [ ] **Step 1: Write failing base probe test**

Create `crates/hw-probe/tests/base_probes.rs`:

```rust
use hw_model::{DeviceKind, DeviceProperties};
use hw_probe::{PciProbe, Probe, ProbeContext, UsbProbe};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn pci_probe_builds_devices_with_driver_info() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:1f.3 Audio device [0403]: Intel Corporation Cannon Lake PCH cAVS [8086:a348]\n\tKernel driver in use: snd_hda_intel\n\tKernel modules: snd_hda_intel\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = PciProbe.probe(&ctx).await;
    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Pci);
    assert_eq!(result.devices[0].driver.as_ref().unwrap().name.as_deref(), Some("snd_hda_intel"));
    assert!(matches!(result.devices[0].properties, DeviceProperties::Pci(_)));
}

#[tokio::test]
async fn usb_probe_builds_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "lsusb",
        [],
        "Bus 001 Device 004: ID 0bda:5689 Realtek Semiconductor Corp. Integrated Camera\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = UsbProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].id, "usb:001:004");
    assert_eq!(result.devices[0].kind, DeviceKind::Usb);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-probe --test base_probes
```

Expected: FAIL because probe modules are not defined.

- [ ] **Step 3: Implement probe exports and shared types**

Replace `crates/hw-probe/src/lib.rs` with:

```rust
pub mod context;
pub mod other;
pub mod pci;
pub mod result;
pub mod traits;
pub mod usb;

pub use context::*;
pub use other::*;
pub use pci::*;
pub use result::*;
pub use traits::*;
pub use usb::*;
```

Create `crates/hw-probe/src/context.rs`:

```rust
use hw_source::SourceRunner;
use std::time::Duration;

pub struct ProbeContext<'a> {
    pub runner: &'a dyn SourceRunner,
    pub timeout: Duration,
}

impl<'a> ProbeContext<'a> {
    pub fn new(runner: &'a dyn SourceRunner, timeout: Duration) -> Self {
        Self { runner, timeout }
    }
}
```

Create `crates/hw-probe/src/result.rs`:

```rust
use hw_model::{Device, DeviceRef, ScanWarning};

#[derive(Debug, Clone, Default)]
pub struct ProbeResult {
    pub devices: Vec<Device>,
    pub warnings: Vec<ScanWarning>,
    pub consumed: Vec<DeviceRef>,
}

impl ProbeResult {
    pub fn with_devices(devices: Vec<Device>) -> Self {
        Self { devices, warnings: Vec::new(), consumed: Vec::new() }
    }
}
```

Create `crates/hw-probe/src/traits.rs`:

```rust
use crate::{ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::DeviceKind;

#[async_trait]
pub trait Probe: Send + Sync {
    fn name(&self) -> &'static str;
    fn kinds(&self) -> &'static [DeviceKind];
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult;
}
```

- [ ] **Step 4: Implement PCI and USB probes**

Create `crates/hw-probe/src/pci.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, BusInfo, Device, DeviceKind, DeviceProperties, DriverInfo, DriverStatus, PciInfo, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::parse_lspci_nn_k;
use hw_source::CommandSpec;

pub struct PciProbe;

#[async_trait]
impl Probe for PciProbe {
    fn name(&self) -> &'static str { "pci" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Pci] }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout).await;
        if !result.is_success() {
            return ProbeResult::default();
        }
        let devices = parse_lspci_nn_k(&result.stdout).into_iter().map(|record| {
            let name = format!("{} {}", record.vendor.clone().unwrap_or_default(), record.device.clone().unwrap_or_default()).trim().to_string();
            Device::new(
                device_id::pci(&record.address),
                DeviceKind::Pci,
                if name.is_empty() { record.address.clone() } else { name },
                DeviceProperties::Pci(PciInfo {
                    address: record.address.clone(),
                    class_name: record.class_name.clone(),
                    class_id: record.class_id.clone(),
                    vendor: record.vendor.clone(),
                    vendor_id: record.vendor_id.clone(),
                    device: record.device.clone(),
                    device_id: record.device_id.clone(),
                    subsystem_vendor_id: record.subsystem_vendor_id.clone(),
                    subsystem_device_id: record.subsystem_device_id.clone(),
                }),
            )
            .with_bus(BusInfo::Pci {
                address: record.address,
                vendor_id: record.vendor_id,
                device_id: record.device_id,
                subsystem_vendor_id: record.subsystem_vendor_id,
                subsystem_device_id: record.subsystem_device_id,
                class: record.class_id,
            })
            .with_driver(DriverInfo {
                name: record.kernel_driver,
                version: None,
                modules: record.kernel_modules,
                provider: None,
                status: DriverStatus::InUse,
            })
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/usb.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, BusInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind, SourceStatus, UsbInfo};
use hw_parser::parse_lsusb;
use hw_source::CommandSpec;

pub struct UsbProbe;

#[async_trait]
impl Probe for UsbProbe {
    fn name(&self) -> &'static str { "usb" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Usb] }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("lsusb", std::iter::empty::<&str>()), ctx.timeout).await;
        if !result.is_success() {
            return ProbeResult::default();
        }
        let devices = parse_lsusb(&result.stdout).into_iter().map(|record| {
            let id = device_id::usb(record.bus.as_deref(), record.device.as_deref(), record.vendor_id.as_deref(), record.product_id.as_deref(), record.serial.as_deref());
            Device::new(
                id,
                DeviceKind::Usb,
                record.product.clone().unwrap_or_else(|| "USB device".to_string()),
                DeviceProperties::Usb(UsbInfo {
                    bus_number: record.bus.clone(),
                    device_number: record.device.clone(),
                    vendor_id: record.vendor_id.clone(),
                    product_id: record.product_id.clone(),
                    class: record.class.clone(),
                    subclass: record.subclass.clone(),
                    protocol: record.protocol.clone(),
                    manufacturer: record.manufacturer.clone(),
                    product: record.product.clone(),
                    serial: record.serial.clone(),
                    speed: record.speed.clone(),
                }),
            )
            .with_bus(BusInfo::Usb {
                bus: record.bus,
                device: record.device,
                vendor_id: record.vendor_id,
                product_id: record.product_id,
                interface: None,
                class: record.class,
            })
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/other.rs`:

```rust
use hw_model::{Device, DeviceKind, DeviceProperties, OtherDeviceInfo, OtherPciInfo};

pub fn other_pci_from(device: &Device) -> Device {
    Device::new(
        device.id.replace("pci:", "other-pci:"),
        DeviceKind::OtherPci,
        device.name.clone(),
        DeviceProperties::OtherPci(OtherPciInfo {
            original_class: match &device.properties {
                DeviceProperties::Pci(pci) => pci.class_name.clone(),
                _ => None,
            },
            reason: "unclassified-pci-device".to_string(),
        }),
    )
}

pub fn other_device_from(device: &Device) -> Device {
    Device::new(
        device.id.replace("usb:", "other-device:"),
        DeviceKind::OtherDevice,
        device.name.clone(),
        DeviceProperties::OtherDevice(OtherDeviceInfo {
            original_kind: Some(device.kind.to_string()),
            reason: "unclassified-device".to_string(),
        }),
    )
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p hw-probe --test base_probes
cargo test -p hw-probe
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/hw-probe
git commit -m "feat: add pci and usb probes"
```

---

### Task 7: Peripheral Probes

**Files:**
- Modify: `crates/hw-probe/src/lib.rs`
- Create: `crates/hw-probe/src/audio.rs`
- Create: `crates/hw-probe/src/bluetooth.rs`
- Create: `crates/hw-probe/src/input.rs`
- Create: `crates/hw-probe/src/camera.rs`
- Create: `crates/hw-probe/src/battery.rs`
- Create: `crates/hw-probe/src/printer.rs`
- Create: `crates/hw-probe/src/cdrom.rs`
- Test: `crates/hw-probe/tests/peripheral_probes.rs`

**Interfaces:**
- Consumes: peripheral parser functions and `ProbeContext`.
- Produces: category probes required by A2.

- [ ] **Step 1: Write failing peripheral probe smoke tests**

Create `crates/hw-probe/tests/peripheral_probes.rs`:

```rust
use hw_model::DeviceKind;
use hw_probe::{AudioProbe, BatteryProbe, CameraProbe, CdromProbe, InputProbe, PrinterProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn audio_probe_reads_proc_asound() {
    let runner = FakeSourceRunner::new().with_file(
        "/proc/asound/cards",
        " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1230000 irq 145\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = AudioProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Audio);
}

#[tokio::test]
async fn battery_probe_reads_upower() {
    let runner = FakeSourceRunner::new().with_command(
        "upower",
        ["--dump"],
        "Device: /org/freedesktop/UPower/devices/battery_BAT0\n  native-path: BAT0\n  battery\n    state: discharging\n    capacity: 88%\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BatteryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Battery);
}

#[tokio::test]
async fn input_camera_printer_and_cdrom_probes_create_devices() {
    let runner = FakeSourceRunner::new()
        .with_file("/proc/bus/input/devices", "N: Name=\"AT Keyboard\"\nH: Handlers=sysrq kbd event0 leds\n\n")
        .with_command("v4l2-ctl", ["--list-devices"], "Integrated Camera:\n\t/dev/video0\n")
        .with_command("lpstat", ["-a"], "Office accepting requests since now\n")
        .with_command("lpstat", ["-v"], "device for Office: ipp://printer.local/ipp/print\n")
        .with_file("/proc/sys/dev/cdrom/info", "drive name:\t\tsr0\nCan read DVD:\t\t1\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    assert_eq!(InputProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Input);
    assert_eq!(CameraProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Camera);
    assert_eq!(PrinterProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Printer);
    assert_eq!(CdromProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Cdrom);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-probe --test peripheral_probes
```

Expected: FAIL because peripheral probes are not defined.

- [ ] **Step 3: Export probe modules**

Append to `crates/hw-probe/src/lib.rs`:

```rust
pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod camera;
pub mod cdrom;
pub mod input;
pub mod printer;

pub use audio::*;
pub use battery::*;
pub use bluetooth::*;
pub use camera::*;
pub use cdrom::*;
pub use input::*;
pub use printer::*;
```

- [ ] **Step 4: Implement peripheral probe files**

Create `crates/hw-probe/src/audio.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, AudioInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::parse_proc_asound_cards;
use std::path::Path;

pub struct AudioProbe;

#[async_trait]
impl Probe for AudioProbe {
    fn name(&self) -> &'static str { "audio" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Audio] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.read_file(Path::new("/proc/asound/cards")).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_proc_asound_cards(&result.stdout).into_iter().map(|card| {
            Device::new(
                device_id::other("audio:card", &card.index.to_string()),
                DeviceKind::Audio,
                card.name.clone().unwrap_or_else(|| format!("Audio card {}", card.index)),
                DeviceProperties::Audio(AudioInfo { card_index: Some(card.index), card_name: card.name, ..Default::default() }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Procfs, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/battery.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, BatteryInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::parse_upower_dump;
use hw_source::CommandSpec;

pub struct BatteryProbe;

#[async_trait]
impl Probe for BatteryProbe {
    fn name(&self) -> &'static str { "battery" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Battery] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("upower", ["--dump"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_upower_dump(&result.stdout).into_iter().map(|power| {
            let name = power.native_path.clone().unwrap_or_else(|| "battery".to_string());
            Device::new(
                device_id::battery(&name),
                DeviceKind::Battery,
                name.clone(),
                DeviceProperties::Battery(BatteryInfo {
                    power_type: Some("battery".to_string()),
                    vendor: power.vendor,
                    model: power.model,
                    serial: power.serial,
                    technology: power.technology,
                    state: power.state,
                    capacity_percent: power.capacity_percent,
                    energy_full_wh: power.energy_full_wh,
                    energy_design_wh: power.energy_design_wh,
                    energy_now_wh: power.energy_now_wh,
                    voltage_v: power.voltage_v,
                    cycle_count: None,
                    present: power.present,
                }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/input.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, Device, DeviceKind, DeviceProperties, InputInfo, InputKind, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::parse_proc_bus_input_devices;
use std::path::Path;

pub struct InputProbe;

#[async_trait]
impl Probe for InputProbe {
    fn name(&self) -> &'static str { "input" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Input] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.read_file(Path::new("/proc/bus/input/devices")).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_proc_bus_input_devices(&result.stdout).into_iter().enumerate().map(|(idx, input)| {
            let event = input.handlers.iter().find(|v| v.starts_with("event")).cloned().unwrap_or_else(|| idx.to_string());
            let name = input.name.clone().unwrap_or_else(|| "Input device".to_string());
            let lower = name.to_ascii_lowercase();
            let input_kind = if input.handlers.iter().any(|h| h == "kbd") || lower.contains("keyboard") { InputKind::Keyboard }
                else if lower.contains("touchpad") { InputKind::Touchpad }
                else if lower.contains("touchscreen") { InputKind::Touchscreen }
                else if input.handlers.iter().any(|h| h.starts_with("mouse")) || lower.contains("mouse") { InputKind::Mouse }
                else { InputKind::UnknownInput };
            Device::new(
                device_id::input_event(&event),
                DeviceKind::Input,
                name,
                DeviceProperties::Input(InputInfo {
                    input_kind,
                    event_node: Some(format!("/dev/input/{event}")),
                    phys: input.phys,
                    uniq: input.uniq,
                    handlers: input.handlers,
                    bus_type: input.bus,
                    vendor_id: input.vendor_id,
                    product_id: input.product_id,
                    version: input.version,
                }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Procfs, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/camera.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, CameraInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::parse_v4l2_list_devices;
use hw_source::CommandSpec;

pub struct CameraProbe;

#[async_trait]
impl Probe for CameraProbe {
    fn name(&self) -> &'static str { "camera" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Camera] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("v4l2-ctl", ["--list-devices"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_v4l2_list_devices(&result.stdout).into_iter().flat_map(|cam| {
            let source = result.source.clone();
            cam.nodes.into_iter().map(move |node| {
                Device::new(
                    device_id::camera(&node),
                    DeviceKind::Camera,
                    cam.name.clone(),
                    DeviceProperties::Camera(CameraInfo { video_node: Some(node), capabilities: Vec::new() }),
                ).with_source(SourceEvidence { source: source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
            }).collect::<Vec<_>>()
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/printer.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, Device, DeviceKind, DeviceProperties, PrinterInfo, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::{parse_lpstat_a, parse_lpstat_v};
use hw_source::CommandSpec;

pub struct PrinterProbe;

#[async_trait]
impl Probe for PrinterProbe {
    fn name(&self) -> &'static str { "printer" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Printer] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let status = ctx.runner.run_command(&CommandSpec::new("lpstat", ["-a"]), ctx.timeout).await;
        if !status.is_success() { return ProbeResult::default(); }
        let uri_result = ctx.runner.run_command(&CommandSpec::new("lpstat", ["-v"]), ctx.timeout).await;
        let uris = if uri_result.is_success() { parse_lpstat_v(&uri_result.stdout) } else { Vec::new() };
        let devices = parse_lpstat_a(&status.stdout).into_iter().map(|printer| {
            let uri = uris.iter().find(|u| u.queue == printer.queue).and_then(|u| u.device_uri.clone());
            Device::new(
                device_id::printer(&printer.queue),
                DeviceKind::Printer,
                printer.queue.clone(),
                DeviceProperties::Printer(PrinterInfo { queue_name: Some(printer.queue), accepting: Some(printer.accepting), device_uri: uri, make_model: None, is_default: None }),
            ).with_source(SourceEvidence { source: status.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/cdrom.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, CdromInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::parse_proc_cdrom_info;
use std::path::Path;

pub struct CdromProbe;

#[async_trait]
impl Probe for CdromProbe {
    fn name(&self) -> &'static str { "cdrom" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Cdrom] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.read_file(Path::new("/proc/sys/dev/cdrom/info")).await;
        if !result.is_success() { return ProbeResult::default(); }
        let info = parse_proc_cdrom_info(&result.stdout);
        let devices = info.drive_names.into_iter().map(|drive| {
            Device::new(
                device_id::other("cdrom", &drive),
                DeviceKind::Cdrom,
                drive.clone(),
                DeviceProperties::Cdrom(CdromInfo { device_node: Some(format!("/dev/{drive}")), media_present: None, capabilities: info.capabilities.clone() }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Procfs, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

Create `crates/hw-probe/src/bluetooth.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, BluetoothInfo, Device, DeviceKind, DeviceProperties, SourceEvidence, SourceKind, SourceStatus};
use hw_parser::{parse_bluetoothctl_paired_devices, parse_hciconfig};
use hw_source::CommandSpec;

pub struct BluetoothProbe;

#[async_trait]
impl Probe for BluetoothProbe {
    fn name(&self) -> &'static str { "bluetooth" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Bluetooth] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let hci = ctx.runner.run_command(&CommandSpec::new("hciconfig", ["-a"]), ctx.timeout).await;
        if !hci.is_success() { return ProbeResult::default(); }
        let paired = ctx.runner.run_command(&CommandSpec::new("bluetoothctl", ["paired-devices"]), ctx.timeout).await;
        let paired_names: Vec<String> = if paired.is_success() {
            parse_bluetoothctl_paired_devices(&paired.stdout).into_iter().map(|p| p.name).collect()
        } else { Vec::new() };
        let devices = parse_hciconfig(&hci.stdout).into_iter().enumerate().map(|(idx, ctrl)| {
            let id_value = ctrl.address.clone().unwrap_or_else(|| idx.to_string());
            Device::new(
                device_id::other("bluetooth", &id_value),
                DeviceKind::Bluetooth,
                ctrl.name.clone().unwrap_or_else(|| "Bluetooth controller".to_string()),
                DeviceProperties::Bluetooth(BluetoothInfo {
                    address: ctrl.address,
                    controller_name: ctrl.name,
                    powered: Some(ctrl.flags.iter().any(|f| f == "UP")),
                    discoverable: Some(ctrl.flags.iter().any(|f| f == "ISCAN")),
                    paired_device_count: Some(paired_names.len() as u32),
                    paired_devices: paired_names.clone(),
                }),
            ).with_source(SourceEvidence { source: hci.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p hw-probe --test peripheral_probes
cargo test -p hw-probe
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/hw-probe
git commit -m "feat: add peripheral hardware probes"
```

---

### Task 8: Collector, Merge, Status, and Fallback Devices

**Files:**
- Replace: `crates/hw-collect/src/lib.rs`
- Create: `crates/hw-collect/src/collector.rs`
- Create: `crates/hw-collect/src/merge.rs`
- Create: `crates/hw-collect/src/status.rs`
- Test: `crates/hw-collect/tests/collector.rs`

**Interfaces:**
- Consumes: all probes, `ScanConfig`, `ScanReport`.
- Produces: `collect_scan_report_with_runner(runner, config)` and public `collect_scan_report(config)`.

- [ ] **Step 1: Write failing collector test**

Create `crates/hw-collect/tests/collector.rs`:

```rust
use hw_collect::collect_scan_report_with_runner;
use hw_model::{DeviceKind, ScanConfig, ScanStatus};
use hw_source::FakeSourceRunner;

#[tokio::test]
async fn collector_runs_base_and_peripheral_probes() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:1f.3 Audio device [0403]: Intel Corporation HD Audio [8086:a348]\n\tKernel driver in use: snd_hda_intel\n")
        .with_command("lsusb", [], "Bus 001 Device 004: ID 0bda:5689 Realtek Integrated Camera\n")
        .with_file("/proc/asound/cards", " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n")
        .with_file("/proc/bus/input/devices", "N: Name=\"AT Keyboard\"\nH: Handlers=sysrq kbd event0 leds\n\n");

    let report = collect_scan_report_with_runner(&runner, ScanConfig::default()).await.unwrap();
    assert_eq!(report.status, ScanStatus::Complete);
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Pci));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Usb));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Audio));
    assert!(report.devices.iter().any(|d| d.kind == DeviceKind::Input));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-collect --test collector
```

Expected: FAIL because collector functions are incomplete.

- [ ] **Step 3: Implement collector modules**

Replace `crates/hw-collect/Cargo.toml` with:

```toml
[package]
name = "hw-collect"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow = { workspace = true }
hw-model = { path = "../hw-model" }
hw-probe = { path = "../hw-probe" }
hw-source = { path = "../hw-source" }
tokio = { workspace = true }

[dev-dependencies]
hw-source = { path = "../hw-source" }
```

Replace `crates/hw-collect/src/lib.rs` with:

```rust
pub mod collector;
pub mod merge;
pub mod status;

pub use collector::*;
```

Create `crates/hw-collect/src/status.rs`:

```rust
use hw_model::{ScanStatus, ScanWarning};

pub fn status_from_warnings(warnings: &[ScanWarning], device_count: usize) -> ScanStatus {
    if device_count == 0 {
        ScanStatus::Failed
    } else if warnings.is_empty() {
        ScanStatus::Complete
    } else {
        ScanStatus::Partial
    }
}
```

Create `crates/hw-collect/src/merge.rs`:

```rust
use hw_model::Device;
use std::collections::BTreeMap;

pub fn dedup_devices(devices: Vec<Device>) -> Vec<Device> {
    let mut by_id: BTreeMap<String, Device> = BTreeMap::new();
    for mut device in devices {
        by_id
            .entry(device.id.clone())
            .and_modify(|existing| {
                existing.sources.append(&mut device.sources);
                existing.warnings.append(&mut device.warnings);
                for capability in device.capabilities.drain(..) {
                    if !existing.capabilities.contains(&capability) {
                        existing.capabilities.push(capability);
                    }
                }
            })
            .or_insert(device);
    }
    by_id.into_values().collect()
}
```

Create `crates/hw-collect/src/collector.rs`:

```rust
use crate::{merge::dedup_devices, status::status_from_warnings};
use anyhow::Result;
use hw_model::{ScanConfig, ScanReport};
use hw_probe::{AudioProbe, BluetoothProbe, CdromProbe, InputProbe, PciProbe, Probe, ProbeContext, UsbProbe, BatteryProbe, CameraProbe, PrinterProbe};
use hw_source::{RealSourceRunner, SourceRunner};

pub async fn collect_scan_report(config: ScanConfig) -> Result<ScanReport> {
    let runner = RealSourceRunner::default();
    collect_scan_report_with_runner(&runner, config).await
}

pub async fn collect_scan_report_with_runner(runner: &dyn SourceRunner, config: ScanConfig) -> Result<ScanReport> {
    let ctx = ProbeContext::new(runner, config.timeout);
    let probes: Vec<Box<dyn Probe>> = vec![
        Box::new(PciProbe),
        Box::new(UsbProbe),
        Box::new(AudioProbe),
        Box::new(BluetoothProbe),
        Box::new(InputProbe),
        Box::new(CameraProbe),
        Box::new(BatteryProbe),
        Box::new(PrinterProbe),
        Box::new(CdromProbe),
    ];

    let mut devices = Vec::new();
    let mut warnings = Vec::new();
    for probe in probes {
        if let Some(kinds) = &config.kinds {
            if !probe.kinds().iter().any(|kind| kinds.contains(kind)) {
                continue;
            }
        }
        if probe.kinds().iter().any(|kind| config.exclude_kinds.contains(kind)) {
            continue;
        }
        let mut result = probe.probe(&ctx).await;
        devices.append(&mut result.devices);
        warnings.append(&mut result.warnings);
    }

    let devices = dedup_devices(devices);
    let mut report = ScanReport::empty();
    report.devices = devices;
    report.warnings = warnings;
    report.status = status_from_warnings(&report.warnings, report.devices.len());
    Ok(report)
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p hw-collect --test collector
cargo test -p hw-collect
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/hw-collect
git commit -m "feat: orchestrate hardware probes"
```

---

### Task 9: Flat Output, JSONL, Summary, Table, and Schema Helpers

**Files:**
- Replace: `crates/hw-output/src/lib.rs`
- Create: `crates/hw-output/src/flat.rs`
- Create: `crates/hw-output/src/jsonl.rs`
- Create: `crates/hw-output/src/summary.rs`
- Create: `crates/hw-output/src/table.rs`
- Create: `crates/hw-output/src/schema.rs`
- Test: `crates/hw-output/tests/output_views.rs`

**Interfaces:**
- Consumes: `ScanReport`, `Device`, `DeviceKind`.
- Produces: `to_flat_report`, `to_jsonl`, `summary_text`, `table_text`, `schema_version`, `list_kinds`.

- [ ] **Step 1: Write failing output tests**

Create `crates/hw-output/tests/output_views.rs`:

```rust
use hw_model::{Device, DeviceKind, DeviceProperties, PciInfo, ScanReport};
use hw_output::{list_kinds, summary_text, table_text, to_flat_report, to_jsonl};

fn sample_report() -> ScanReport {
    let mut report = ScanReport::empty();
    report.devices.push(Device::new(
        "pci:0000:00:1f.3",
        DeviceKind::Pci,
        "Intel HD Audio",
        DeviceProperties::Pci(PciInfo { address: "0000:00:1f.3".to_string(), ..Default::default() }),
    ));
    report
}

#[test]
fn flat_report_counts_devices_by_kind() {
    let flat = to_flat_report(&sample_report());
    assert_eq!(flat.summary.device_count, 1);
    assert_eq!(flat.summary.counts_by_kind.get("pci"), Some(&1));
}

#[test]
fn jsonl_outputs_one_device_line() {
    let text = to_jsonl(&sample_report()).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("pci:0000:00:1f.3"));
}

#[test]
fn human_outputs_include_device_name() {
    assert!(summary_text(&sample_report()).contains("Devices: 1"));
    assert!(table_text(&sample_report(), None).contains("Intel HD Audio"));
    assert!(list_kinds().contains(&"other-pci".to_string()));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-output --test output_views
```

Expected: FAIL because output functions are missing.

- [ ] **Step 3: Implement output modules**

Replace `crates/hw-output/src/lib.rs` with:

```rust
pub mod flat;
pub mod jsonl;
pub mod schema;
pub mod summary;
pub mod table;

pub use flat::*;
pub use jsonl::*;
pub use schema::*;
pub use summary::*;
pub use table::*;
```

Create `crates/hw-output/src/schema.rs`:

```rust
use hw_model::{DeviceKind, SCHEMA_VERSION};

pub fn schema_version() -> &'static str {
    SCHEMA_VERSION
}

pub fn list_kinds() -> Vec<String> {
    DeviceKind::ALL.iter().map(|kind| kind.to_string()).collect()
}
```

Create `crates/hw-output/src/flat.rs`:

```rust
use hw_model::{Device, ScanMetadata, ScanReport, ScanStatus, ScanWarning};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatScanReportView {
    pub schema_version: String,
    pub status: ScanStatus,
    pub metadata: ScanMetadata,
    pub summary: FlatSummary,
    pub devices: Vec<FlatDeviceView>,
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatSummary {
    pub device_count: usize,
    pub counts_by_kind: BTreeMap<String, usize>,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatDeviceView {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub bus: Option<Value>,
    pub driver: Option<Value>,
    pub capabilities: Vec<String>,
    pub identifiers: Vec<Value>,
    pub properties: Value,
    pub sources: Vec<Value>,
    pub warnings: Vec<ScanWarning>,
}

pub fn to_flat_report(report: &ScanReport) -> FlatScanReportView {
    let mut counts_by_kind = BTreeMap::new();
    for device in &report.devices {
        *counts_by_kind.entry(device.kind.to_string()).or_insert(0) += 1;
    }
    FlatScanReportView {
        schema_version: report.schema_version.clone(),
        status: report.status,
        metadata: report.metadata.clone(),
        summary: FlatSummary {
            device_count: report.devices.len(),
            counts_by_kind,
            warning_count: report.warnings.len(),
        },
        devices: report.devices.iter().map(to_flat_device).collect(),
        warnings: report.warnings.clone(),
    }
}

pub fn to_flat_device(device: &Device) -> FlatDeviceView {
    FlatDeviceView {
        id: device.id.clone(),
        kind: device.kind.to_string(),
        name: device.name.clone(),
        vendor: device.vendor.clone(),
        model: device.model.clone(),
        serial: device.serial.clone(),
        bus: device.bus.as_ref().map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
        driver: device.driver.as_ref().map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
        capabilities: device.capabilities.clone(),
        identifiers: device.identifiers.iter().map(|v| serde_json::to_value(v).unwrap_or(Value::Null)).collect(),
        properties: serde_json::to_value(&device.properties).unwrap_or(Value::Null),
        sources: device.sources.iter().map(|v| serde_json::to_value(v).unwrap_or(Value::Null)).collect(),
        warnings: device.warnings.clone(),
    }
}
```

Create `crates/hw-output/src/jsonl.rs`:

```rust
use crate::flat::to_flat_device;
use anyhow::Result;
use hw_model::ScanReport;

pub fn to_jsonl(report: &ScanReport) -> Result<String> {
    let mut lines = Vec::new();
    for device in &report.devices {
        lines.push(serde_json::to_string(&to_flat_device(device))?);
    }
    Ok(lines.join("\n"))
}
```

Create `crates/hw-output/src/summary.rs`:

```rust
use hw_model::ScanReport;
use std::collections::BTreeMap;

pub fn summary_text(report: &ScanReport) -> String {
    let mut counts = BTreeMap::new();
    for device in &report.devices {
        *counts.entry(device.kind.to_string()).or_insert(0usize) += 1;
    }
    let mut text = format!("Status: {:?}\nDevices: {}\nWarnings: {}\n", report.status, report.devices.len(), report.warnings.len());
    for (kind, count) in counts {
        text.push_str(&format!("{}: {}\n", kind, count));
    }
    text
}
```

Create `crates/hw-output/src/table.rs`:

```rust
use hw_model::{DeviceKind, ScanReport};

pub fn table_text(report: &ScanReport, filter: Option<DeviceKind>) -> String {
    let mut out = String::from("KIND       ID                           NAME\n");
    for device in &report.devices {
        if filter.is_some_and(|kind| device.kind != kind) {
            continue;
        }
        out.push_str(&format!("{:<10} {:<28} {}\n", device.kind, device.id, device.name));
    }
    out
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p hw-output --test output_views
cargo test -p hw-output
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/hw-output
git commit -m "feat: add scanner output views"
```

---

### Task 10: CLI Commands and Exit Codes

**Files:**
- Replace: `crates/hw-cli/src/main.rs`
- Create: `crates/hw-cli/src/args.rs`
- Create: `crates/hw-cli/src/exit.rs`
- Test: `crates/hw-cli/tests/args.rs`

**Interfaces:**
- Consumes: `hw_collect::collect_scan_report`, `hw_output::*`, `ScanConfig`, `DeviceKind`.
- Produces: `qurbrix-hw scan`, `summary`, `table`, `list-kinds`, `schema`, `sources` command shape.

- [ ] **Step 1: Write failing CLI argument tests**

Create `crates/hw-cli/tests/args.rs`:

```rust
use clap::Parser;
use hw_cli::args::{Cli, Command, OutputFormat};
use hw_model::DeviceKind;

#[test]
fn parses_scan_json_kind_filter() {
    let cli = Cli::parse_from(["qurbrix-hw", "scan", "--format", "json", "--kind", "storage"]);
    match cli.command {
        Command::Scan(scan) => {
            assert_eq!(scan.format, OutputFormat::Json);
            assert_eq!(scan.kind, vec![DeviceKind::Storage]);
        }
        _ => panic!("expected scan"),
    }
}

#[test]
fn parses_list_kinds() {
    let cli = Cli::parse_from(["qurbrix-hw", "list-kinds"]);
    assert!(matches!(cli.command, Command::ListKinds { .. }));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-cli --test args
```

Expected: FAIL because `args` module is not exposed.

- [ ] **Step 3: Make `hw-cli` testable as a library and binary**

Modify `crates/hw-cli/Cargo.toml`:

```toml
[package]
name = "hw-cli"
edition.workspace = true
version.workspace = true
license.workspace = true
publish.workspace = true

[lib]
path = "src/lib.rs"

[[bin]]
name = "hw-cli"
path = "src/main.rs"

[dependencies]
anyhow = { workspace = true }
clap = { workspace = true }
hw-collect = { path = "../hw-collect" }
hw-model = { path = "../hw-model" }
hw-output = { path = "../hw-output" }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

Create `crates/hw-cli/src/lib.rs`:

```rust
pub mod args;
pub mod exit;
```

- [ ] **Step 4: Implement CLI args and exit codes**

Create `crates/hw-cli/src/args.rs`:

```rust
use clap::{Args, Parser, Subcommand, ValueEnum};
use hw_model::DeviceKind;
use std::{str::FromStr, time::Duration};

#[derive(Debug, Parser)]
#[command(name = "qurbrix-hw", version, about = "General-purpose Linux hardware scanner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Scan(ScanArgs),
    Summary,
    Table(TableArgs),
    ListKinds {
        #[arg(long, value_enum, default_value_t = ListFormat::Text)]
        format: ListFormat,
    },
    Schema {
        #[arg(long)]
        version: bool,
    },
    Sources {
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
    #[arg(long)]
    pub pretty: bool,
    #[arg(long, value_parser = parse_kind)]
    pub kind: Vec<DeviceKind>,
    #[arg(long, value_parser = parse_kind)]
    pub exclude_kind: Vec<DeviceKind>,
    #[arg(long, default_value = "30s", value_parser = parse_duration)]
    pub timeout: Duration,
    #[arg(long)]
    pub no_optional_sources: bool,
    #[arg(long)]
    pub no_sources: bool,
    #[arg(long)]
    pub no_warnings: bool,
}

#[derive(Debug, Args)]
pub struct TableArgs {
    #[arg(long, value_parser = parse_kind)]
    pub kind: Option<DeviceKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Jsonl,
    TypedJson,
    SummaryJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListFormat {
    Text,
    Json,
}

pub fn parse_kind(value: &str) -> Result<DeviceKind, String> {
    DeviceKind::from_str(value)
}

pub fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds.parse::<u64>().map(Duration::from_secs).map_err(|err| err.to_string());
    }
    value.parse::<u64>().map(Duration::from_secs).map_err(|err| err.to_string())
}
```

Create `crates/hw-cli/src/exit.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Ok = 0,
    CliOrSerialization = 1,
    ScanFailed = 2,
    Unsupported = 3,
    Permission = 4,
    Timeout = 124,
}

impl ExitCode {
    pub fn code(self) -> i32 {
        self as i32
    }
}
```

Replace `crates/hw-cli/src/main.rs`:

```rust
use anyhow::Result;
use clap::Parser;
use hw_cli::args::{Cli, Command, ListFormat, OutputFormat};
use hw_model::ScanConfig;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => {
            let config = ScanConfig {
                kinds: if args.kind.is_empty() { None } else { Some(args.kind) },
                exclude_kinds: args.exclude_kind,
                timeout: args.timeout,
                optional_sources: !args.no_optional_sources,
                include_sources: !args.no_sources,
                include_warnings: !args.no_warnings,
            };
            let report = hw_collect::collect_scan_report(config).await?;
            match args.format {
                OutputFormat::Json => {
                    let flat = hw_output::to_flat_report(&report);
                    if args.pretty { println!("{}", serde_json::to_string_pretty(&flat)?); }
                    else { println!("{}", serde_json::to_string(&flat)?); }
                }
                OutputFormat::Jsonl => println!("{}", hw_output::to_jsonl(&report)?),
                OutputFormat::TypedJson => {
                    if args.pretty { println!("{}", serde_json::to_string_pretty(&report)?); }
                    else { println!("{}", serde_json::to_string(&report)?); }
                }
                OutputFormat::SummaryJson => {
                    let flat = hw_output::to_flat_report(&report);
                    println!("{}", serde_json::to_string(&flat.summary)?);
                }
            }
        }
        Command::Summary => {
            let report = hw_collect::collect_scan_report(ScanConfig::default()).await?;
            print!("{}", hw_output::summary_text(&report));
        }
        Command::Table(args) => {
            let report = hw_collect::collect_scan_report(ScanConfig::default()).await?;
            print!("{}", hw_output::table_text(&report, args.kind));
        }
        Command::ListKinds { format } => match format {
            ListFormat::Text => println!("{}", hw_output::list_kinds().join("\n")),
            ListFormat::Json => println!("{}", serde_json::to_string(&hw_output::list_kinds())?),
        },
        Command::Schema { version } => {
            if version { println!("{}", hw_output::schema_version()); }
            else { println!("{{\"schema_version\":\"{}\"}}", hw_output::schema_version()); }
        }
        Command::Sources { format: _ } => {
            println!("{{\"sources\":[]}}");
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run tests and smoke commands**

Run:

```bash
cargo test -p hw-cli --test args
cargo test -p hw-cli
cargo run --bin qurbrix-hw -- list-kinds
cargo run --bin qurbrix-hw -- schema --version
cargo check --workspace
```

Expected: tests PASS, `list-kinds` prints supported kinds, schema prints `qurbrix.hw.scan.v1`.

- [ ] **Step 6: Commit**

```bash
git add crates/hw-cli Cargo.toml
git commit -m "feat: add scanner cli commands"
```

---

### Task 11: Migrate Existing Core Categories into New Device Model

**Files:**
- Create: `crates/hw-parser/src/cpu.rs`
- Create: `crates/hw-parser/src/network.rs`
- Create: `crates/hw-parser/src/storage.rs`
- Modify: `crates/hw-parser/src/lib.rs`
- Create: `crates/hw-probe/src/existing.rs`
- Modify: `crates/hw-probe/src/lib.rs`
- Modify: `crates/hw-collect/src/collector.rs`
- Test: `crates/hw-probe/tests/existing_category_probes.rs`

**Interfaces:**
- Consumes: current parser ideas from old code, but emits new `Device` model only.
- Produces: `CpuProbe`, `NetworkProbe`, and `StorageProbe`. Task 12 adds `MemoryProbe`, `BiosProbe`, `GpuProbe`, and `MonitorProbe` with explicit parser and probe code.

- [ ] **Step 1: Write failing tests for migrated core probes**

Create `crates/hw-probe/tests/existing_category_probes.rs`:

```rust
use hw_model::DeviceKind;
use hw_probe::{CpuProbe, NetworkProbe, Probe, ProbeContext, StorageProbe};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn cpu_probe_outputs_cpu_device() {
    let runner = FakeSourceRunner::new().with_command(
        "lscpu",
        [],
        "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Cpu);
}

#[tokio::test]
async fn network_probe_outputs_network_device() {
    let runner = FakeSourceRunner::new().with_command(
        "ip",
        ["-j", "link"],
        r#"[{"ifname":"eth0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Network);
}

#[tokio::test]
async fn storage_probe_outputs_storage_device() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN"],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Storage);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-probe --test existing_category_probes
```

Expected: FAIL because migrated probes are not defined.

- [ ] **Step 3: Implement minimal parsers for first migrated batch**

Add to `crates/hw-parser/Cargo.toml` dependencies:

```toml
serde_json = { workspace = true }
```

Create `crates/hw-parser/src/cpu.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CpuRecord {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub model_name: Option<String>,
    pub vendor: Option<String>,
    pub cores_per_socket: Option<u32>,
    pub sockets: Option<u32>,
}

pub fn parse_lscpu(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match key.trim() {
            "Architecture" => record.architecture = Some(value.to_string()),
            "CPU(s)" => record.threads = value.parse().ok(),
            "Model name" => record.model_name = Some(value.to_string()),
            "Vendor ID" => record.vendor = Some(value.to_string()),
            "Core(s) per socket" => record.cores_per_socket = value.parse().ok(),
            "Socket(s)" => record.sockets = value.parse().ok(),
            _ => {}
        }
    }
    record
}
```

Create `crates/hw-parser/src/network.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpLinkRecord {
    pub ifname: String,
    pub address: Option<String>,
    pub operstate: Option<String>,
    pub mtu: Option<u32>,
}

pub fn parse_ip_j_link(input: &str) -> Vec<IpLinkRecord> {
    serde_json::from_str(input).unwrap_or_default()
}
```

Create `crates/hw-parser/src/storage.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsblkReport {
    #[serde(default)]
    pub blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsblkDevice {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub size: Option<u64>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub tran: Option<String>,
}

pub fn parse_lsblk_json(input: &str) -> Vec<LsblkDevice> {
    serde_json::from_str::<LsblkReport>(input).map(|r| r.blockdevices).unwrap_or_default()
}
```

Append to `crates/hw-parser/src/lib.rs`:

```rust
pub mod cpu;
pub mod network;
pub mod storage;

pub use cpu::*;
pub use network::*;
pub use storage::*;
```

- [ ] **Step 4: Implement migrated probes**

Append to `crates/hw-probe/src/lib.rs`:

```rust
pub mod existing;
pub use existing::*;
```

Create `crates/hw-probe/src/existing.rs`:

```rust
use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{device_id, CpuInfo, Device, DeviceKind, DeviceProperties, NetworkInfo, SourceEvidence, SourceKind, SourceStatus, StorageInfo};
use hw_parser::{parse_ip_j_link, parse_lsblk_json, parse_lscpu};
use hw_source::CommandSpec;

pub struct CpuProbe;
pub struct NetworkProbe;
pub struct StorageProbe;

#[async_trait]
impl Probe for CpuProbe {
    fn name(&self) -> &'static str { "cpu" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Cpu] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("lscpu", std::iter::empty::<&str>()), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let cpu = parse_lscpu(&result.stdout);
        let cores = match (cpu.cores_per_socket, cpu.sockets) {
            (Some(c), Some(s)) => Some(c * s),
            _ => None,
        };
        let device = Device::new(
            "cpu:0",
            DeviceKind::Cpu,
            cpu.model_name.clone().unwrap_or_else(|| "CPU".to_string()),
            DeviceProperties::Cpu(CpuInfo {
                name: cpu.model_name,
                vendor: cpu.vendor,
                architecture: cpu.architecture,
                cores,
                threads: cpu.threads,
                sockets: cpu.sockets,
                ..Default::default()
            }),
        ).with_source(SourceEvidence { source: result.source, kind: SourceKind::Command, status: SourceStatus::Success, summary: None });
        ProbeResult::with_devices(vec![device])
    }
}

#[async_trait]
impl Probe for NetworkProbe {
    fn name(&self) -> &'static str { "network" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Network] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("ip", ["-j", "link"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_ip_j_link(&result.stdout).into_iter().map(|net| {
            Device::new(
                device_id::network(net.address.as_deref(), &net.ifname),
                DeviceKind::Network,
                net.ifname.clone(),
                DeviceProperties::Network(NetworkInfo { interface: Some(net.ifname), mac: net.address, operstate: net.operstate, mtu: net.mtu, ..Default::default() }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}

#[async_trait]
impl Probe for StorageProbe {
    fn name(&self) -> &'static str { "storage" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Storage] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("lsblk", ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_lsblk_json(&result.stdout).into_iter().filter(|dev| dev.device_type.as_deref() == Some("disk")).map(|dev| {
            let node = format!("/dev/{}", dev.name);
            Device::new(
                device_id::storage(None, dev.serial.as_deref(), &node),
                DeviceKind::Storage,
                dev.model.clone().unwrap_or_else(|| node.clone()),
                DeviceProperties::Storage(StorageInfo { device_node: Some(node), size_bytes: dev.size, media_type: dev.tran, serial: dev.serial, ..Default::default() }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

- [ ] **Step 5: Wire migrated probes into collector**

In `crates/hw-collect/src/collector.rs`, add `CpuProbe`, `NetworkProbe`, and `StorageProbe` to the existing `use hw_probe::{AudioProbe, BluetoothProbe, CdromProbe, InputProbe, PciProbe, Probe, ProbeContext, UsbProbe, BatteryProbe, CameraProbe, PrinterProbe};` import so it reads:

```rust
use hw_probe::{AudioProbe, BatteryProbe, BluetoothProbe, CameraProbe, CdromProbe, CpuProbe, InputProbe, NetworkProbe, PciProbe, PrinterProbe, Probe, ProbeContext, StorageProbe, UsbProbe};
```

Then add these entries after USB in the probe list:

```rust
Box::new(CpuProbe),
Box::new(StorageProbe),
Box::new(NetworkProbe),
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p hw-probe --test existing_category_probes
cargo test -p hw-collect --test collector
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/hw-parser crates/hw-probe crates/hw-collect
git commit -m "feat: migrate core hardware categories"
```

---


### Task 12: Complete Remaining Existing Category Migration

**Files:**
- Create: `crates/hw-parser/src/dmi.rs`
- Create: `crates/hw-parser/src/gpu.rs`
- Create: `crates/hw-parser/src/monitor.rs`
- Modify: `crates/hw-parser/src/lib.rs`
- Modify: `crates/hw-probe/src/existing.rs`
- Modify: `crates/hw-collect/src/collector.rs`
- Test: `crates/hw-probe/tests/remaining_category_probes.rs`

**Interfaces:**
- Consumes: `ProbeContext`, `DeviceProperties::{Memory,Bios,Motherboard,Gpu,Monitor}`, and command outputs from `dmidecode`, `lspci`, and `xrandr`.
- Produces: `MemoryProbe`, `BiosProbe`, `GpuProbe`, and `MonitorProbe` wired into the collector.

- [ ] **Step 1: Write failing tests for remaining migrated categories**

Create `crates/hw-probe/tests/remaining_category_probes.rs`:

```rust
use hw_model::DeviceKind;
use hw_probe::{BiosProbe, GpuProbe, MemoryProbe, MonitorProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn memory_probe_outputs_dimm_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "memory"],
        "Memory Device\n\tSize: 16 GB\n\tLocator: ChannelA-DIMM0\n\tManufacturer: Samsung\n\tSerial Number: ABCD\n\tPart Number: M471A2K43\n\tType: DDR4\n\tSpeed: 3200 MT/s\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
}

#[tokio::test]
async fn bios_probe_outputs_bios_and_motherboard_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\tVendor: LENOVO\n\tVersion: N2IET98W\n\tRelease Date: 01/01/2026\nBase Board Information\n\tManufacturer: LENOVO\n\tProduct Name: 20XX\n\tSerial Number: BOARD123\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;
    assert!(result.devices.iter().any(|d| d.kind == DeviceKind::Bios));
    assert!(result.devices.iter().any(|d| d.kind == DeviceKind::Motherboard));
}

#[tokio::test]
async fn gpu_and_monitor_probes_output_devices() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n\tKernel driver in use: i915\n")
        .with_command("xrandr", ["--query"], "eDP-1 connected primary 1920x1080+0+0\n   1920x1080     60.00*+\nHDMI-1 disconnected\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    assert_eq!(GpuProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Gpu);
    assert_eq!(MonitorProbe.probe(&ctx).await.devices[0].kind, DeviceKind::Monitor);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p hw-probe --test remaining_category_probes
```

Expected: FAIL because `MemoryProbe`, `BiosProbe`, `GpuProbe`, and `MonitorProbe` are not defined.

- [ ] **Step 3: Implement DMI, GPU, and monitor parsers**

Create `crates/hw-parser/src/dmi.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiMemoryRecord {
    pub size: Option<String>,
    pub locator: Option<String>,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub part_number: Option<String>,
    pub memory_type: Option<String>,
    pub speed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiBiosBoardRecord {
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub bios_release_date: Option<String>,
    pub board_manufacturer: Option<String>,
    pub board_product_name: Option<String>,
    pub board_serial: Option<String>,
}

pub fn parse_dmidecode_memory(input: &str) -> Vec<DmiMemoryRecord> {
    let mut records = Vec::new();
    let mut current: Option<DmiMemoryRecord> = None;
    for line in input.lines().chain(std::iter::once("")) {
        if line.trim() == "Memory Device" {
            if let Some(record) = current.take() {
                if record.size.as_deref() != Some("No Module Installed") {
                    records.push(record);
                }
            }
            current = Some(DmiMemoryRecord::default());
            continue;
        }
        let Some(record) = current.as_mut() else { continue };
        let Some((key, value)) = line.trim().split_once(':') else { continue };
        let value = value.trim();
        match key {
            "Size" => record.size = Some(value.to_string()),
            "Locator" => record.locator = Some(value.to_string()),
            "Manufacturer" => record.manufacturer = Some(value.to_string()),
            "Serial Number" => record.serial = Some(value.to_string()),
            "Part Number" => record.part_number = Some(value.to_string()),
            "Type" => record.memory_type = Some(value.to_string()),
            "Speed" => record.speed = Some(value.to_string()),
            _ => {}
        }
    }
    if let Some(record) = current.take() {
        if record.size.as_deref() != Some("No Module Installed") {
            records.push(record);
        }
    }
    records
}

pub fn parse_dmidecode_bios_board(input: &str) -> DmiBiosBoardRecord {
    let mut record = DmiBiosBoardRecord::default();
    let mut section = "";
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed == "BIOS Information" || trimmed == "Base Board Information" {
            section = trimmed;
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else { continue };
        let value = value.trim().to_string();
        match (section, key) {
            ("BIOS Information", "Vendor") => record.bios_vendor = Some(value),
            ("BIOS Information", "Version") => record.bios_version = Some(value),
            ("BIOS Information", "Release Date") => record.bios_release_date = Some(value),
            ("Base Board Information", "Manufacturer") => record.board_manufacturer = Some(value),
            ("Base Board Information", "Product Name") => record.board_product_name = Some(value),
            ("Base Board Information", "Serial Number") => record.board_serial = Some(value),
            _ => {}
        }
    }
    record
}

pub fn parse_size_to_bytes(value: Option<&str>) -> Option<u64> {
    let value = value?;
    let mut parts = value.split_whitespace();
    let number = parts.next()?.parse::<u64>().ok()?;
    let unit = parts.next().unwrap_or("").to_ascii_lowercase();
    match unit.as_str() {
        "kb" | "kib" => Some(number * 1024),
        "mb" | "mib" => Some(number * 1024 * 1024),
        "gb" | "gib" => Some(number * 1024 * 1024 * 1024),
        "tb" | "tib" => Some(number * 1024 * 1024 * 1024 * 1024),
        _ => Some(number),
    }
}

pub fn parse_speed_mtps(value: Option<&str>) -> Option<u32> {
    value?.split_whitespace().next()?.parse().ok()
}
```

Create `crates/hw-parser/src/gpu.rs`:

```rust
use crate::{parse_lspci_nn_k, PciRecord};

pub fn parse_gpu_lspci(input: &str) -> Vec<PciRecord> {
    parse_lspci_nn_k(input)
        .into_iter()
        .filter(|record| {
            let class = record.class_name.as_deref().unwrap_or("").to_ascii_lowercase();
            class.contains("vga") || class.contains("3d controller") || class.contains("display")
        })
        .collect()
}
```

Create `crates/hw-parser/src/monitor.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct XrandrMonitorRecord {
    pub connector: String,
    pub connected: bool,
    pub primary: bool,
    pub resolution: Option<String>,
}

pub fn parse_xrandr_query(input: &str) -> Vec<XrandrMonitorRecord> {
    input
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let connector = parts.next()?;
            let state = parts.next()?;
            if state != "connected" && state != "disconnected" {
                return None;
            }
            let rest: Vec<&str> = parts.collect();
            let primary = rest.contains(&"primary");
            let resolution = rest.iter().find(|part| part.contains('x') && part.contains('+')).map(|value| value.split('+').next().unwrap_or(value).to_string());
            Some(XrandrMonitorRecord { connector: connector.to_string(), connected: state == "connected", primary, resolution })
        })
        .collect()
}
```

Append to `crates/hw-parser/src/lib.rs`:

```rust
pub mod dmi;
pub mod gpu;
pub mod monitor;

pub use dmi::*;
pub use gpu::*;
pub use monitor::*;
```

- [ ] **Step 4: Add remaining probes to `existing.rs`**

Append this code to `crates/hw-probe/src/existing.rs`:

```rust
use hw_model::{BiosInfo, BusInfo, DriverInfo, DriverStatus, GpuInfo, MemoryInfo, MonitorInfo, MotherboardInfo};
use hw_parser::{parse_dmidecode_bios_board, parse_dmidecode_memory, parse_gpu_lspci, parse_size_to_bytes, parse_speed_mtps, parse_xrandr_query};

pub struct MemoryProbe;
pub struct BiosProbe;
pub struct GpuProbe;
pub struct MonitorProbe;

#[async_trait]
impl Probe for MemoryProbe {
    fn name(&self) -> &'static str { "memory" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Memory] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("dmidecode", ["-t", "memory"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_dmidecode_memory(&result.stdout).into_iter().enumerate().map(|(idx, mem)| {
            let id = mem.serial.as_ref().filter(|v| !v.trim().is_empty()).map(|serial| format!("memory:serial:{serial}")).unwrap_or_else(|| format!("memory:slot:{}", mem.locator.clone().unwrap_or_else(|| idx.to_string())));
            Device::new(
                id,
                DeviceKind::Memory,
                mem.locator.clone().unwrap_or_else(|| format!("Memory DIMM {idx}")),
                DeviceProperties::Memory(MemoryInfo {
                    size_bytes: parse_size_to_bytes(mem.size.as_deref()),
                    vendor: mem.manufacturer,
                    memory_type: mem.memory_type,
                    speed_mtps: parse_speed_mtps(mem.speed.as_deref()),
                    locator: mem.locator,
                    serial: mem.serial,
                    part_number: mem.part_number,
                }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}

#[async_trait]
impl Probe for BiosProbe {
    fn name(&self) -> &'static str { "bios" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Bios, DeviceKind::Motherboard] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("dmidecode", ["-t", "0,1,2,3"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let dmi = parse_dmidecode_bios_board(&result.stdout);
        let bios = Device::new(
            "bios:0",
            DeviceKind::Bios,
            dmi.bios_version.clone().unwrap_or_else(|| "BIOS".to_string()),
            DeviceProperties::Bios(BiosInfo { vendor: dmi.bios_vendor, version: dmi.bios_version, release_date: dmi.bios_release_date, ..Default::default() }),
        ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None });
        let board = Device::new(
            dmi.board_serial.as_ref().map(|serial| format!("motherboard:serial:{serial}")).unwrap_or_else(|| "motherboard:0".to_string()),
            DeviceKind::Motherboard,
            dmi.board_product_name.clone().unwrap_or_else(|| "Motherboard".to_string()),
            DeviceProperties::Motherboard(MotherboardInfo { manufacturer: dmi.board_manufacturer, product_name: dmi.board_product_name, serial: dmi.board_serial, ..Default::default() }),
        ).with_source(SourceEvidence { source: result.source, kind: SourceKind::Command, status: SourceStatus::Success, summary: None });
        ProbeResult::with_devices(vec![bios, board])
    }
}

#[async_trait]
impl Probe for GpuProbe {
    fn name(&self) -> &'static str { "gpu" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Gpu] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_gpu_lspci(&result.stdout).into_iter().map(|gpu| {
            Device::new(
                device_id::other("gpu:pci", &gpu.address),
                DeviceKind::Gpu,
                gpu.device.clone().or(gpu.vendor.clone()).unwrap_or_else(|| "GPU".to_string()),
                DeviceProperties::Gpu(GpuInfo::default()),
            )
            .with_bus(BusInfo::Pci { address: gpu.address, vendor_id: gpu.vendor_id, device_id: gpu.device_id, subsystem_vendor_id: gpu.subsystem_vendor_id, subsystem_device_id: gpu.subsystem_device_id, class: gpu.class_id })
            .with_driver(DriverInfo { name: gpu.kernel_driver, version: None, modules: gpu.kernel_modules, provider: None, status: DriverStatus::InUse })
            .with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}

#[async_trait]
impl Probe for MonitorProbe {
    fn name(&self) -> &'static str { "monitor" }
    fn kinds(&self) -> &'static [DeviceKind] { &[DeviceKind::Monitor] }
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx.runner.run_command(&CommandSpec::new("xrandr", ["--query"]), ctx.timeout).await;
        if !result.is_success() { return ProbeResult::default(); }
        let devices = parse_xrandr_query(&result.stdout).into_iter().filter(|mon| mon.connected).map(|mon| {
            Device::new(
                device_id::other("monitor", &mon.connector),
                DeviceKind::Monitor,
                mon.connector.clone(),
                DeviceProperties::Monitor(MonitorInfo { connector: Some(mon.connector), resolution: mon.resolution, ..Default::default() }),
            ).with_source(SourceEvidence { source: result.source.clone(), kind: SourceKind::Command, status: SourceStatus::Success, summary: None })
        }).collect();
        ProbeResult::with_devices(devices)
    }
}
```

- [ ] **Step 5: Wire remaining probes into collector**

In `crates/hw-collect/src/collector.rs`, include these imports from `hw_probe`:

```rust
MemoryProbe, BiosProbe, GpuProbe, MonitorProbe,
```

Add these entries to the probe list after `CpuProbe`:

```rust
Box::new(MemoryProbe),
Box::new(BiosProbe),
Box::new(GpuProbe),
Box::new(MonitorProbe),
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p hw-probe --test remaining_category_probes
cargo test -p hw-collect --test collector
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/hw-parser crates/hw-probe crates/hw-collect
git commit -m "feat: migrate remaining core hardware categories"
```

---

### Task 13: Documentation, Acceptance Smoke Tests, and Cleanup

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Create: `docs/cli-output-contract.md`
- Modify or remove: stale references to old `Inventory`, `ComponentRow`, `formatprint` in docs.
- Test: workspace commands below.

**Interfaces:**
- Consumes: completed crates and CLI.
- Produces: documented scanner usage and verified workspace state.

- [ ] **Step 1: Update README usage**

Replace the old Basic Usage section in `README.md` with:

```markdown
## Basic Usage

Run a machine-readable scan:

```bash
qurbrix-hw scan --format json --pretty
```

Stream one JSON object per device:

```bash
qurbrix-hw scan --format jsonl
```

Show a human summary:

```bash
qurbrix-hw summary
qurbrix-hw table --kind storage
```

List supported device kinds:

```bash
qurbrix-hw list-kinds
```

Library usage:

```rust
use hw_model::ScanConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let report = qurbrix_hw::collect_scan_report(ScanConfig::default()).await?;
    let flat = hw_output::to_flat_report(&report);
    println!("{}", serde_json::to_string_pretty(&flat)?);
    Ok(())
}
```
```

- [ ] **Step 2: Create CLI output contract doc**

Create `docs/cli-output-contract.md`:

```markdown
# qurbrix-hw CLI Output Contract

`qurbrix-hw` is script/agent-first. Structured results are written to stdout. Logs and diagnostics are written to stderr.

## Default command

```bash
qurbrix-hw scan --format json
```

## Status

- `complete`: requested scan completed without material warnings.
- `partial`: usable report was produced, but at least one source was missing, failed, timed out, or produced partial data.
- `failed`: no valid report was produced.

`partial` returns exit code `0`.

## Device kind strings

Kind strings use kebab-case. Examples:

- `cpu`
- `storage`
- `audio`
- `bluetooth`
- `other-pci`
- `other-device`

## Exit codes

| Exit code | Meaning |
| --- | --- |
| 0 | Scan succeeded, including partial reports |
| 1 | CLI argument error or serialization error |
| 2 | Scan failed and no valid report was generated |
| 3 | Requested kind/source is unsupported |
| 4 | Permission failure prevents core scan |
| 124 | Timeout |
```

- [ ] **Step 3: Update Chinese README**

Add this section to `README.zh-CN.md`:

```markdown
## 命令行用法

默认输出面向脚本和 agent，结构化结果写入 stdout，日志写入 stderr：

```bash
qurbrix-hw scan --format json --pretty
qurbrix-hw scan --format jsonl
qurbrix-hw summary
qurbrix-hw table --kind storage
qurbrix-hw list-kinds
```

扫描状态：

- `complete`：扫描完成且没有重要 warning。
- `partial`：生成了可用报告，但部分数据源缺失、失败、超时或权限不足。
- `failed`：无法生成有效报告。

`partial` 仍返回退出码 `0`，方便脚本继续消费已有结果。
```

- [ ] **Step 4: Run acceptance commands**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo run --bin qurbrix-hw -- list-kinds
cargo run --bin qurbrix-hw -- schema --version
cargo run --bin qurbrix-hw -- scan --format summary-json
```

Expected:

- `cargo fmt --all -- --check`: PASS.
- `cargo test --workspace`: PASS.
- `list-kinds`: includes `audio`, `bluetooth`, `other-pci`, `other-device`.
- `schema --version`: prints `qurbrix.hw.scan.v1`.
- `scan --format summary-json`: prints valid JSON summary. It may show zero devices or partial data on minimal systems only if test environment lacks sources; it must not panic.

- [ ] **Step 5: Inspect git status**

Run:

```bash
git status --short
```

Expected: only intentional changes from this task are listed.

- [ ] **Step 6: Commit**

```bash
git add README.md README.zh-CN.md docs/cli-output-contract.md
git commit -m "docs: document scanner cli contract"
```

---

## Self-Review Notes

### Spec coverage

- Architecture split into `hw-model`, `hw-source`, `hw-parser`, `hw-probe`, `hw-collect`, `hw-output`, `hw-cli`, `hw-testdata`: covered by Tasks 1-3 and crate-specific tasks.
- Strong typed model and flat output: covered by Tasks 2 and 9.
- PCI/USB/driver foundational enumeration: covered by Tasks 4 and 6.
- High-priority A2 peripherals: parser coverage in Task 5, probe coverage in Task 7.
- Existing CPU/storage/network migration batch: covered by Task 11. Memory, BIOS, GPU, and monitor migration: covered by Task 12.
- Collector orchestration, dedup, warnings, status: covered by Task 8.
- CLI commands and script/agent output behavior: covered by Task 10.
- Documentation and acceptance commands: covered by Task 12.

### Type consistency

- `ScanReport::empty()` is defined in Task 2 and consumed by Tasks 1, 8, and 9.
- `DeviceKind` string serialization is defined in Task 2 and consumed by Tasks 9 and 10.
- `SourceRunner` is defined in Task 3 and consumed by Tasks 6, 7, 8, and 11.
- `Probe` interface is defined in Task 6 and consumed by Tasks 7, 8, and 11.
- Output functions `to_flat_report`, `to_jsonl`, `summary_text`, `table_text`, `schema_version`, and `list_kinds` are defined in Task 9 and consumed by Task 10.

### Implementation caution

Task 11 migrates CPU, storage, and network first because they provide representative command JSON/text patterns and unblock end-to-end CLI smoke tests. Task 12 completes the remaining existing categories with memory, BIOS, GPU, and monitor probes.
