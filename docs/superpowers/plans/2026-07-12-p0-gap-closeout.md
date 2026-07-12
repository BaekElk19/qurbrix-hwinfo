# P0 Gap Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close five P0 hardware data gaps documented in `docs/component-gap-report-2026-07-11.md` §14.1: bluetooth parser fields (Q1), monitor `edid_hex` output (Q3), GPU driver version (Q4), dmidecode OEM Strings (Q6), and lsblk column extension (Q7).

**Architecture:** Purely additive — every new model field is `Option<T>` or `Vec<T>` with `#[serde(default)]`, no existing behavior changes. Each of the five gaps becomes one self-contained commit that adds fixture → failing parser test → parser fields → model fields → probe integration. All five commits live on branch `fix/p0-gap-closeout` and merge as one PR.

**Tech Stack:** Rust 2021 workspace with `crates/hw-model` (data types), `crates/hw-parser` (text parsers), `crates/hw-probe` (async probes), `crates/hw-testdata` (shared fixtures). Async runtime uses tokio via `async_trait`; test framework is stock `cargo test`.

## Global Constraints

- **Branch:** `fix/p0-gap-closeout` from `master` (repo default is `master`, not `main`; recent commits confirm).
- **Backward compatibility:** All new `hw-model` fields use `#[serde(default)]` so existing serialized reports deserialize unchanged.
- **Test discipline:** TDD — every commit lands its fixture and failing test before the implementation.
- **Verification gate:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo build --release` all green.
- **Do not touch:**
  - Files with uncommitted modifications already present in the working tree at plan start: `crates/hw-bindid/tests/devices.rs`, `crates/hw-model/src/kind.rs`, `crates/hw-parser/src/network.rs`, `crates/hw-parser/tests/network.rs`, `crates/hw-probe/tests/existing_category_probes.rs`, the deleted `docs/six-components-gap-report-2026-07-09.md`, and the untracked `crates/hw-testdata/fixtures/network/` directory. These belong to concurrent unrelated work; leave them exactly as-is (`git status` at plan start should stay identical to plan end for these paths).
  - Within `crates/hw-model/src/properties.rs`, `crates/hw-probe/src/existing.rs`: only touch the regions this plan calls out by line. Do not opportunistically refactor unrelated code.
  - `docs/component-gap-report-2026-07-11.md`: only append `(fixed in <sha>)` markers to the five §14.1 rows this plan modifies. Do not edit completion percentages or other rows.
- **Fixture crate:** All new fixtures go under `crates/hw-testdata/fixtures/**` and are loaded via `hw_testdata::fixture(...)` (returns `String`) or `hw_testdata::fixture_bytes(...)` (returns `Vec<u8>`).
- **Runner API:** `ctx.runner.run_command(&CommandSpec::new(cmd, args), ctx.timeout).await` for shell commands; `ctx.runner.read_file(&path).await` for sysfs reads (see `read_sysfs_dmi_value` at `crates/hw-probe/src/existing.rs:5089` for the convention).
- **PR:** Single PR, five commits, commit order = Q1 → Q3 → Q4 → Q6 → Q7 (dependency-free ordering; Q7 last because it rewrites `crates/hw-testdata/fixtures/storage/lsblk.json` which downstream storage tests consume).
- **Do not run:** `git push --force`, `git reset --hard`, `git commit --no-verify`.

## Spec Drift Correction

The spec at `docs/superpowers/specs/2026-07-12-p0-gap-closeout-design.md` §3.3 says "GPU probe has 11 `.with_driver(DriverInfo { ..., version: None, ... })` sites". Verified during plan authoring — inside `GpuProbe::probe` (starts `crates/hw-probe/src/existing.rs:5290`) there is exactly **one** such site at line 5414. The other 10 matches sit in network / storage / audio / usb / input / bluetooth probes and are out of scope for Q4. Q4 therefore modifies only that single call site.

---

## File Structure

**New files:**
- `crates/hw-testdata/fixtures/gpu/sys_module_i915_version.txt` — sysfs module version fixture for Q4.
- `crates/hw-testdata/fixtures/gpu/modinfo-nvidia.txt` — `modinfo nvidia` output fixture for Q4.
- `crates/hw-testdata/fixtures/dmi/dmidecode-t11.txt` — dmidecode Type 11 fixture for Q6.
- `crates/hw-parser/tests/dmi.rs` — new test file for `parse_dmi_oem_strings`; if this file already exists, append instead.
- `crates/hw-parser/tests/bluetooth.rs` — new test file for `parse_hciconfig` extended fields; if this file already exists, append instead.

**Modified files:**
- `crates/hw-model/src/properties.rs` — add fields to `BluetoothInfo` (Q1), `MonitorInfo` (Q3), `BiosInfo` (Q6). `StorageInfo` unchanged; `GpuInfo` unchanged (Q4 uses existing `DriverInfo.version`).
- `crates/hw-parser/src/bluetooth.rs` — extend `BluetoothControllerRecord` and `parse_hciconfig` (Q1).
- `crates/hw-parser/src/monitor.rs` — extend `XrandrVerboseMonitorRecord` with `edid_hex` (Q3).
- `crates/hw-parser/src/dmi.rs` — add `parse_dmi_oem_strings` function (Q6).
- `crates/hw-parser/src/storage.rs` — extend `LsblkDevice` struct with 4 optional string fields (Q7).
- `crates/hw-probe/src/bluetooth.rs` — thread new fields from record to `BluetoothInfo` (Q1).
- `crates/hw-probe/src/existing.rs`
  - `MonitorProbe` (around line 6800–7080) — thread `edid_hex` through `edids` map into `MonitorInfo` (Q3).
  - `GpuProbe::probe` (line 5290 onward; `.with_driver` at 5414) — after device assembly, populate `DriverInfo.version` (Q4).
  - `BiosProbe::probe` (line 4818 onward) — add `dmidecode -t 11` call, feed into `BiosInfo.oem_strings` (Q6).
  - `StorageProbe::probe` (line 3751 onward; lsblk invocation at 3756) — extend lsblk `-o` column list (Q7).
- `crates/hw-testdata/fixtures/storage/lsblk.json` — add `mountpoint`, `fstype`, `partuuid`, `label` on at least one partition node (Q7).
- `crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt` — verify it contains HCI Version / LMP Version / Manufacturer / Class / Features lines; if not, replace with an Intel or Realtek controller's real `hciconfig -a` output (Q1).
- `docs/component-gap-report-2026-07-11.md` — append `(fixed in <short-sha>)` to §14.1 rows Q1/Q3/Q4/Q6/Q7 (once per commit).

---

## Task 1: Q1 — Bluetooth parser recovers HCI/LMP/Manufacturer/Class/Features

**Files:**
- Modify: `crates/hw-parser/src/bluetooth.rs` (struct + `parse_hciconfig` body around lines 5–63)
- Modify: `crates/hw-model/src/properties.rs` (`BluetoothInfo` around line 396)
- Modify: `crates/hw-probe/src/bluetooth.rs` (`BluetoothInfo` construction sites around lines 76 and 209)
- Modify: `crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt` (verify or replace)
- Create or extend: `crates/hw-parser/tests/bluetooth.rs`

**Interfaces:**
- Produces: `BluetoothControllerRecord` with new optional fields `hci_version: Option<String>`, `lmp_version: Option<String>`, `manufacturer: Option<String>`, `device_class: Option<String>`, `features: Vec<String>`.
- Produces: `BluetoothInfo` with matching 5 fields.

- [ ] **Step 1.1: Cut the working branch**

```bash
git switch -c fix/p0-gap-closeout
```

Expected output: `Switched to a new branch 'fix/p0-gap-closeout'`.

- [ ] **Step 1.2: Verify the fixture contains the target lines**

```bash
grep -E "HCI Version|LMP Version|Manufacturer|Class|Features" \
  crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt
```

Expected: at least one line matching each of `HCI Version:`, `LMP Version:`, `Manufacturer:`, `Class:`, `Features:`. If any is missing, replace the fixture with the following exact content (from an Intel AX210 controller):

```
hci0:   Type: Primary  Bus: USB
        BD Address: 3C:F0:11:80:9E:19  ACL MTU: 1021:4  SCO MTU: 96:6
        UP RUNNING PSCAN ISCAN
        RX bytes:47228 acl:1090 sco:0 events:1929 errors:0
        TX bytes:16388 acl:1039 sco:0 commands:277 errors:0
        Features: 0xff 0xff 0xff 0xfe 0xdb 0xff 0x7b 0x87
        Packet type: DM1 DM3 DM5 DH1 DH3 DH5 HV1 HV2 HV3
        Link policy: RSWITCH SNIFF
        Link mode: SLAVE ACCEPT
        Name: 'thinkpad-x1'
        Class: 0x7c010c
        Service Classes: Rendering, Capturing, Object Transfer, Audio, Telephony
        Device Class: Computer, Laptop
        HCI Version: 5.3 (0xc)  Revision: 0x1234
        LMP Version: 5.3 (0xc)  Subversion: 0x100
        Manufacturer: Intel Corp. (2)
```

- [ ] **Step 1.3: Add the failing parser test**

Create or extend `crates/hw-parser/tests/bluetooth.rs`:

```rust
use hw_parser::parse_hciconfig;
use hw_testdata::fixture;

#[test]
fn hciconfig_extended_fields_are_populated() {
    let input = fixture("bluetooth/hciconfig-a.txt");
    let records = parse_hciconfig(&input);

    let record = records.first().expect("one controller parsed");
    assert_eq!(record.hci_version.as_deref(), Some("5.3 (0xc)  Revision: 0x1234"));
    assert_eq!(record.lmp_version.as_deref(), Some("5.3 (0xc)  Subversion: 0x100"));
    assert_eq!(record.manufacturer.as_deref(), Some("Intel Corp. (2)"));
    assert_eq!(record.device_class.as_deref(), Some("0x7c010c"));
    assert_eq!(record.features, vec![
        "0xff", "0xff", "0xff", "0xfe", "0xdb", "0xff", "0x7b", "0x87",
    ]);
    assert!(record.flags.contains(&"UP".to_string()));
    assert!(record.flags.contains(&"RUNNING".to_string()));
}
```

- [ ] **Step 1.4: Run the test and confirm it fails**

```bash
cargo test -p hw-parser --test bluetooth hciconfig_extended_fields_are_populated
```

Expected: compile error `no field 'hci_version' on 'BluetoothControllerRecord'` (or equivalent for whichever field the compiler picks first).

- [ ] **Step 1.5: Extend the record struct**

Edit `crates/hw-parser/src/bluetooth.rs` lines 4–10, replace the struct with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BluetoothControllerRecord {
    pub name: Option<String>,
    pub address: Option<String>,
    pub bus: Option<String>,
    pub flags: Vec<String>,
    pub hci_version: Option<String>,
    pub lmp_version: Option<String>,
    pub manufacturer: Option<String>,
    pub device_class: Option<String>,
    pub features: Vec<String>,
}
```

- [ ] **Step 1.6: Extend `parse_hciconfig` to populate the new fields**

Replace the body of the per-line `else if let Some(record) = current.as_mut()` branch (lines 42–57) with:

```rust
} else if let Some(record) = current.as_mut() {
    if let Some(caps) = address_re.captures(line) {
        record.address = Some(caps[1].to_string());
    } else if let Some(caps) = name_re.captures(line) {
        record.name = Some(caps[1].to_string());
    } else if let Some(rest) = line.trim_start().strip_prefix("HCI Version:") {
        record.hci_version = Some(rest.trim().to_string());
    } else if let Some(rest) = line.trim_start().strip_prefix("LMP Version:") {
        record.lmp_version = Some(rest.trim().to_string());
    } else if let Some(rest) = line.trim_start().strip_prefix("Manufacturer:") {
        record.manufacturer = Some(rest.trim().to_string());
    } else if let Some(rest) = line.trim_start().strip_prefix("Class:") {
        record.device_class = Some(rest.trim().to_string());
    } else if let Some(rest) = line.trim_start().strip_prefix("Features:") {
        record.features = rest
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect();
    } else {
        let flags: Vec<String> = line
            .split_whitespace()
            .filter(|v| v.chars().all(|c| c.is_ascii_uppercase()))
            .map(ToOwned::to_owned)
            .collect();
        if !flags.is_empty() {
            record.flags = flags;
        }
    }
}
```

Order matters: the explicit prefixes must be tried before the generic uppercase-flag fallback, otherwise lines like `HCI Version: 5.3 (0xc)` would be swallowed as flags (because `HCI` is all-uppercase).

- [ ] **Step 1.7: Run the parser test and confirm it passes**

```bash
cargo test -p hw-parser --test bluetooth hciconfig_extended_fields_are_populated
```

Expected: `1 passed`.

- [ ] **Step 1.8: Extend `BluetoothInfo`**

Edit `crates/hw-model/src/properties.rs`, replace the `BluetoothInfo` struct at line ~396 with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BluetoothInfo {
    pub address: Option<String>,
    pub controller_name: Option<String>,
    pub powered: Option<bool>,
    pub discoverable: Option<bool>,
    pub paired_device_count: Option<u32>,
    pub paired_devices: Vec<String>,
    #[serde(default)]
    pub hci_version: Option<String>,
    #[serde(default)]
    pub lmp_version: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub device_class: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
}
```

- [ ] **Step 1.9: Thread the fields through the probe**

Edit `crates/hw-probe/src/bluetooth.rs`. At the two `BluetoothInfo { ... }` construction sites (around lines 76 and 209), add the 5 fields sourced from `ctrl` / the controller record. Example for the first site around line 76 (before this task the block is `DeviceProperties::Bluetooth(BluetoothInfo { address: ctrl.address, controller_name: ctrl.name, ...paired... })`):

```rust
DeviceProperties::Bluetooth(BluetoothInfo {
    address: ctrl.address,
    controller_name: ctrl.name,
    powered,
    discoverable,
    paired_device_count: Some(paired_names.len() as u32),
    paired_devices: paired_names,
    hci_version: ctrl.hci_version,
    lmp_version: ctrl.lmp_version,
    manufacturer: ctrl.manufacturer,
    device_class: ctrl.device_class,
    features: ctrl.features,
})
```

For the second site around line 209 (the rfkill-only fallback path that has no `ctrl` record — controller was not seen in hciconfig), leave the 5 new fields as `None` / `Vec::new()`:

```rust
DeviceProperties::Bluetooth(BluetoothInfo {
    address: /* existing */,
    controller_name: Some(controller_name),
    powered,
    discoverable: None,
    paired_device_count: None,
    paired_devices: Vec::new(),
    hci_version: None,
    lmp_version: None,
    manufacturer: None,
    device_class: None,
    features: Vec::new(),
})
```

Preserve every existing field's original value; only append the 5 new lines.

- [ ] **Step 1.10: Workspace verify**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all green.

- [ ] **Step 1.11: Mark the gap report and commit**

Edit `docs/component-gap-report-2026-07-11.md`, append ` (fixed in <sha>)` at the end of the Q1 row in the §14.1 table (fill the sha after the commit). Then:

```bash
git add crates/hw-parser/src/bluetooth.rs \
        crates/hw-parser/tests/bluetooth.rs \
        crates/hw-model/src/properties.rs \
        crates/hw-probe/src/bluetooth.rs \
        crates/hw-testdata/fixtures/bluetooth/hciconfig-a.txt \
        docs/component-gap-report-2026-07-11.md
git commit -m "fix(bluetooth): recover HCI/LMP version, manufacturer, class, features

Bluetooth parser only extracted BD Address, Name, and generic uppercase
flags. Extends BluetoothControllerRecord and BluetoothInfo with 5 new
optional fields sourced from the existing hciconfig -a output. Fields
serialized with #[serde(default)] so historical reports still load.

Closes Q1 from component-gap-report-2026-07-11.md §14.1."
```

After the commit lands, replace `<sha>` in the gap report with `git rev-parse --short HEAD` and amend:

```bash
SHA=$(git rev-parse --short HEAD)
sed -i "s/(fixed in <sha>)/(fixed in ${SHA})/" docs/component-gap-report-2026-07-11.md
git add docs/component-gap-report-2026-07-11.md
git commit --amend --no-edit
```

---

## Task 2: Q3 — MonitorInfo.edid_hex output

**Files:**
- Modify: `crates/hw-parser/src/monitor.rs` (`XrandrVerboseMonitorRecord` around line 16, `parse_xrandr_verbose` around line 80)
- Modify: `crates/hw-model/src/properties.rs` (`MonitorInfo` around line 287)
- Modify: `crates/hw-probe/src/existing.rs` (`MonitorProbe` `edids` map plumbing around line 6900 and `MonitorInfo` construction around line 6982)
- Modify: `crates/hw-parser/tests/monitor_verbose.rs` (extend, do not replace)

**Interfaces:**
- Consumes: `parse_xrandr_verbose` currently returns `Vec<XrandrVerboseMonitorRecord { connector: String, edid: Vec<u8> }>`.
- Produces: `XrandrVerboseMonitorRecord` gains `edid_hex: String` (lowercase, no whitespace, no `0x` prefix).
- Produces: `MonitorInfo` gains `edid_hex: Option<String>`.

- [ ] **Step 2.1: Locate the xrandr verbose fixture used by existing tests**

```bash
ls crates/hw-testdata/fixtures/xrandr/
grep -n "fixture(\"xrandr/" crates/hw-parser/tests/monitor_verbose.rs
```

Pick the fixture path the existing tests already consume — the plan calls it `<XRANDR_VERBOSE_FIXTURE>` below.

- [ ] **Step 2.2: Add the failing parser test**

Append to `crates/hw-parser/tests/monitor_verbose.rs`, substituting the fixture path found in Step 2.1:

```rust
#[test]
fn xrandr_verbose_returns_lowercase_edid_hex() {
    let input = hw_testdata::fixture(/* <XRANDR_VERBOSE_FIXTURE> */);
    let records = hw_parser::parse_xrandr_verbose(&input);
    let record = records.first().expect("one record");
    assert!(!record.edid_hex.is_empty());
    assert!(record.edid_hex.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(record.edid_hex.chars().all(|c| !c.is_ascii_uppercase()));
    assert_eq!(record.edid_hex.len(), record.edid.len() * 2);
}
```

- [ ] **Step 2.3: Run the test and confirm it fails**

```bash
cargo test -p hw-parser --test monitor_verbose xrandr_verbose_returns_lowercase_edid_hex
```

Expected: compile error `no field 'edid_hex' on 'XrandrVerboseMonitorRecord'`.

- [ ] **Step 2.4: Extend `XrandrVerboseMonitorRecord`**

Edit `crates/hw-parser/src/monitor.rs`, replace the struct at line ~16 with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrandrVerboseMonitorRecord {
    pub connector: String,
    pub edid: Vec<u8>,
    pub edid_hex: String,
}
```

- [ ] **Step 2.5: Populate `edid_hex` inside `parse_xrandr_verbose`**

In `parse_xrandr_verbose` (line ~80), each site that pushes `XrandrVerboseMonitorRecord` currently reads:

```rust
let edid = hex_to_bytes(&edid_hex);
if !edid.is_empty() {
    records.push(XrandrVerboseMonitorRecord { connector, edid });
}
```

Replace both such sites (loop body around line 95–97 and post-loop tail around line 135–137) with:

```rust
let edid_bytes = hex_to_bytes(&edid_hex);
if !edid_bytes.is_empty() {
    let normalized_hex = edid_hex
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    records.push(XrandrVerboseMonitorRecord {
        connector,
        edid: edid_bytes,
        edid_hex: normalized_hex,
    });
}
```

Rename the local `edid` shadow to `edid_bytes` at both sites to avoid the name clash with the struct field.

- [ ] **Step 2.6: Run the parser test and confirm it passes**

```bash
cargo test -p hw-parser --test monitor_verbose xrandr_verbose_returns_lowercase_edid_hex
```

Expected: `1 passed`.

- [ ] **Step 2.7: Extend `MonitorInfo`**

Edit `crates/hw-model/src/properties.rs` `MonitorInfo` (line ~287). At the end of the struct, before the closing `}`, add:

```rust
    #[serde(default)]
    pub edid_hex: Option<String>,
```

- [ ] **Step 2.8: Thread `edid_hex` through the probe**

Edit `crates/hw-probe/src/existing.rs` around line 6900. The current code stores the byte vec in the `edids` map:

```rust
if verbose_result.is_success() {
    for record in parse_xrandr_verbose(&verbose_result.stdout) {
        edids
            .entry(record.connector)
            .or_default()
            .insert(0, (record.edid, verbose_result.source.clone()));
    }
}
```

Change the tuple type stored in `edids` from `(Vec<u8>, String)` to `(Vec<u8>, String, String)` (bytes, source, hex). Update the block to:

```rust
if verbose_result.is_success() {
    for record in parse_xrandr_verbose(&verbose_result.stdout) {
        edids
            .entry(record.connector)
            .or_default()
            .insert(0, (record.edid, verbose_result.source.clone(), record.edid_hex));
    }
}
```

Every other consumer of `edids` in this function (search inside `MonitorProbe::probe` for `edids.remove` and destructuring of the tuple) must be updated to accept the 3-tuple. Adjust the `MonitorInfo` construction site around line 6982 to pass the hex through:

```rust
let mut info = MonitorInfo {
    connector: Some(connector.clone()),
    interface: monitor_connector_interface(&connector),
    raw_interface: Some(connector.clone()),
    aspect_ratio: resolution.as_deref().and_then(monitor_aspect_ratio),
    resolution,
    current_refresh_hz,
    is_primary: primary,
    max_resolution,
    // ...existing fields preserved...
    edid_hex: /* the hex string popped from the edids map for this connector */,
    ..Default::default()
};
```

If a connector was seen without a verbose EDID (e.g., xrandr `--query` only), `edid_hex` is `None`. This is fine.

The exact destructuring change depends on the surrounding block that this plan cannot show verbatim without reading 100+ lines. When touching lines 6900–7080, search for every occurrence of `(edid, source)` and `edids.remove` — each must adopt the 3-tuple shape.

- [ ] **Step 2.9: Workspace verify**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all green.

- [ ] **Step 2.10: Mark and commit**

Append `(fixed in <sha>)` to Q3 row in `docs/component-gap-report-2026-07-11.md`.

```bash
git add crates/hw-parser/src/monitor.rs \
        crates/hw-parser/tests/monitor_verbose.rs \
        crates/hw-model/src/properties.rs \
        crates/hw-probe/src/existing.rs \
        docs/component-gap-report-2026-07-11.md
git commit -m "fix(monitor): expose edid_hex on MonitorInfo

parse_xrandr_verbose already assembled the raw EDID hex string
internally but only returned the decoded bytes. Adds a normalized
(lowercase, whitespace-stripped) edid_hex on XrandrVerboseMonitorRecord
and threads it through MonitorProbe into MonitorInfo.edid_hex.

Closes Q3 from component-gap-report-2026-07-11.md §14.1."
SHA=$(git rev-parse --short HEAD)
sed -i "s/(fixed in <sha>)/(fixed in ${SHA})/" docs/component-gap-report-2026-07-11.md
git add docs/component-gap-report-2026-07-11.md
git commit --amend --no-edit
```

---

## Task 3: Q4 — GPU DriverInfo.version from sysfs + modinfo

**Files:**
- Modify: `crates/hw-probe/src/existing.rs` (`GpuProbe::probe` `.with_driver` at line 5414; new helper function after `apply_gpu_xrandr_enrichment`)
- Create: `crates/hw-testdata/fixtures/gpu/sys_module_i915_version.txt`
- Create: `crates/hw-testdata/fixtures/gpu/modinfo-nvidia.txt`
- Modify: `crates/hw-parser/tests/gpu.rs` (extend)
- Consider: adding a small `parse_modinfo_version` in `crates/hw-parser/src/gpu.rs` so the fallback path is unit-testable without a probe.

**Interfaces:**
- Produces: `pub fn parse_modinfo_version(input: &str) -> Option<String>` in `hw-parser`.
- Produces: `async fn gpu_driver_version(ctx: &ProbeContext<'_>, driver_name: &str) -> Option<String>` in `existing.rs`. Reads `/sys/module/<name>/version` via `ctx.runner.read_file`, falls back to `modinfo <name>` via `ctx.runner.run_command`.

- [ ] **Step 3.1: Create the fixtures**

Write `crates/hw-testdata/fixtures/gpu/sys_module_i915_version.txt` — exact content, no trailing whitespace beyond newline:

```
6.6.30
```

Write `crates/hw-testdata/fixtures/gpu/modinfo-nvidia.txt`:

```
filename:       /lib/modules/6.6.30-1-desktop/updates/nvidia.ko.zst
alias:          char-major-195-*
version:        550.90.07
supported:      external
license:        NVIDIA
srcversion:     ABCDEF0123456789
```

- [ ] **Step 3.2: Add the failing parser test**

Append to `crates/hw-parser/tests/gpu.rs`:

```rust
#[test]
fn modinfo_version_extracts_first_version_line() {
    let input = hw_testdata::fixture("gpu/modinfo-nvidia.txt");
    assert_eq!(
        hw_parser::parse_modinfo_version(&input).as_deref(),
        Some("550.90.07"),
    );
}

#[test]
fn modinfo_version_returns_none_when_missing() {
    assert_eq!(hw_parser::parse_modinfo_version("filename: /foo\nlicense: GPL"), None);
}
```

- [ ] **Step 3.3: Run the test and confirm it fails**

```bash
cargo test -p hw-parser --test gpu modinfo_version
```

Expected: compile error `cannot find function 'parse_modinfo_version' in crate 'hw_parser'`.

- [ ] **Step 3.4: Implement `parse_modinfo_version`**

Add to `crates/hw-parser/src/gpu.rs` (append at the bottom, or place next to any existing sibling parser):

```rust
pub fn parse_modinfo_version(input: &str) -> Option<String> {
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("version:") {
            let trimmed = rest.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
```

Confirm `parse_modinfo_version` is re-exported from `crates/hw-parser/src/lib.rs`. If the crate uses `pub use gpu::*;` no action needed; otherwise add the explicit `pub use gpu::parse_modinfo_version;`.

- [ ] **Step 3.5: Run parser tests and confirm they pass**

```bash
cargo test -p hw-parser --test gpu modinfo_version
```

Expected: `2 passed`.

- [ ] **Step 3.6: Add the probe helper**

Edit `crates/hw-probe/src/existing.rs`. Find a suitable spot just after the `apply_gpu_xrandr_enrichment` function definition (line 5664+ area). Add:

```rust
async fn gpu_driver_version(ctx: &ProbeContext<'_>, driver_name: &str) -> Option<String> {
    let sysfs_path = Path::new("/sys/module").join(driver_name).join("version");
    let sysfs_result = ctx.runner.read_file(&sysfs_path).await;
    if sysfs_result.is_success() {
        let value = sysfs_result.stdout.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    let modinfo_result = ctx
        .runner
        .run_command(&CommandSpec::new("modinfo", [driver_name]), ctx.timeout)
        .await;
    if modinfo_result.is_success() {
        return parse_modinfo_version(&modinfo_result.stdout);
    }
    None
}
```

Ensure `parse_modinfo_version` is imported at the top of the file alongside other `hw_parser::` imports (near line 28).

- [ ] **Step 3.7: Use the helper at the sole `with_driver` site inside `GpuProbe::probe`**

Around line 5414 the current code is:

```rust
.with_driver(DriverInfo {
    name: gpu.kernel_driver,
    version: None,
    modules: gpu.kernel_modules,
    provider: None,
    status: DriverStatus::InUse,
})
```

Replace with:

```rust
.with_driver(DriverInfo {
    name: gpu.kernel_driver.clone(),
    version: match gpu.kernel_driver.as_deref() {
        Some(driver) => gpu_driver_version(ctx, driver).await,
        None => None,
    },
    modules: gpu.kernel_modules,
    provider: None,
    status: DriverStatus::InUse,
})
```

`gpu.kernel_driver` is `Option<String>`. The `.clone()` on the `name:` field is necessary because we now borrow it in the `match`.

If this call site is inside a synchronous block (e.g., a `.map()` closure), refactor the enclosing loop to `for` / `while let` so the `.await` is legal. Search from line 5390 backwards to line 5290 (top of `GpuProbe::probe`) to identify the loop shape. Do NOT wrap in `block_on` or `tokio::spawn` — the surrounding function is already `async`.

- [ ] **Step 3.8: Workspace verify**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all green.

- [ ] **Step 3.9: Mark and commit**

Append `(fixed in <sha>)` to Q4 row in `docs/component-gap-report-2026-07-11.md`.

```bash
git add crates/hw-parser/src/gpu.rs \
        crates/hw-parser/src/lib.rs \
        crates/hw-parser/tests/gpu.rs \
        crates/hw-probe/src/existing.rs \
        crates/hw-testdata/fixtures/gpu/sys_module_i915_version.txt \
        crates/hw-testdata/fixtures/gpu/modinfo-nvidia.txt \
        docs/component-gap-report-2026-07-11.md
git commit -m "fix(gpu): populate DriverInfo.version from sysfs with modinfo fallback

GpuProbe's DriverInfo previously left the version field None. Adds
gpu_driver_version helper that reads /sys/module/<driver>/version first
(fast, no subprocess) and falls back to modinfo <driver> when the module
is builtin. parse_modinfo_version lives in hw-parser for standalone
testability.

Closes Q4 from component-gap-report-2026-07-11.md §14.1."
SHA=$(git rev-parse --short HEAD)
sed -i "s/(fixed in <sha>)/(fixed in ${SHA})/" docs/component-gap-report-2026-07-11.md
git add docs/component-gap-report-2026-07-11.md
git commit --amend --no-edit
```

---

## Task 4: Q6 — dmidecode -t 11 OEM Strings

**Files:**
- Create: `crates/hw-testdata/fixtures/dmi/dmidecode-t11.txt`
- Modify: `crates/hw-parser/src/dmi.rs` (append `parse_dmi_oem_strings`)
- Modify: `crates/hw-parser/src/lib.rs` (re-export if needed)
- Modify: `crates/hw-model/src/properties.rs` (`BiosInfo` around line 81)
- Modify: `crates/hw-probe/src/existing.rs` (`BiosProbe::probe` around line 4818; `bios_board_devices` around line 5183)
- Create or extend: `crates/hw-parser/tests/dmi.rs`

**Interfaces:**
- Produces: `pub fn parse_dmi_oem_strings(input: &str) -> Vec<String>` in `hw-parser`.
- Produces: `BiosInfo` gains `oem_strings: Vec<String>`.
- Produces: `bios_board_devices` gains a new parameter carrying the OEM strings so the sysfs-fallback path can still surface OEM data as an empty `Vec`.

- [ ] **Step 4.1: Create the fixture**

Write `crates/hw-testdata/fixtures/dmi/dmidecode-t11.txt`:

```
# dmidecode 3.3
Getting SMBIOS data from sysfs.
SMBIOS 3.2.0 present.

Handle 0x000E, DMI type 11, 5 bytes
OEM Strings
	String 1: Default string
	String 2: LENOVO_MT_20UAS0LK00_BU_Think_FM_ThinkPad X1 Carbon Gen 9
	String 3: LENOVO_BIOS: N32ET75W (1.50 )
	String 4: Not Specified

Handle 0x000F, DMI type 32, 20 bytes
System Boot Information
	Status: No errors detected
```

Preserve the leading tabs — dmidecode indents with real tabs.

- [ ] **Step 4.2: Add the failing parser test**

Create or extend `crates/hw-parser/tests/dmi.rs`:

```rust
use hw_parser::parse_dmi_oem_strings;
use hw_testdata::fixture;

#[test]
fn dmi_oem_strings_are_parsed_and_filtered() {
    let input = fixture("dmi/dmidecode-t11.txt");
    let strings = parse_dmi_oem_strings(&input);
    assert_eq!(strings, vec![
        "Default string".to_string(),
        "LENOVO_MT_20UAS0LK00_BU_Think_FM_ThinkPad X1 Carbon Gen 9".to_string(),
        "LENOVO_BIOS: N32ET75W (1.50 )".to_string(),
    ]);
    // "Not Specified" is filtered out.
}

#[test]
fn dmi_oem_strings_returns_empty_when_section_missing() {
    assert!(parse_dmi_oem_strings("Handle 0x0001, DMI type 0, 20 bytes\nBIOS Information").is_empty());
}
```

- [ ] **Step 4.3: Run the test and confirm it fails**

```bash
cargo test -p hw-parser --test dmi
```

Expected: `cannot find function 'parse_dmi_oem_strings'`.

- [ ] **Step 4.4: Implement `parse_dmi_oem_strings`**

Append to `crates/hw-parser/src/dmi.rs`:

```rust
pub fn parse_dmi_oem_strings(input: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut in_section = false;
    for line in input.lines() {
        if line.starts_with("Handle ") {
            in_section = line.contains("DMI type 11");
            continue;
        }
        if !in_section {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("String ") {
            if let Some((_, value)) = rest.split_once(':') {
                let value = value.trim();
                if !value.is_empty()
                    && !value.eq_ignore_ascii_case("Not Specified")
                    && !value.eq_ignore_ascii_case("Default string is not present")
                {
                    strings.push(value.to_string());
                }
            }
        }
    }
    strings
}
```

Note: the first fixture entry `"Default string"` is intentionally preserved — it's a common OEM placeholder that IS present on real machines and represents the actual OEM's default value. Only `"Not Specified"` (dmidecode's sentinel for absent data) is filtered.

Re-export from `crates/hw-parser/src/lib.rs` alongside other `dmi::*` re-exports:

```rust
pub use dmi::parse_dmi_oem_strings;
```

If `dmi::*` is already glob-re-exported, no action needed.

- [ ] **Step 4.5: Run parser tests and confirm they pass**

```bash
cargo test -p hw-parser --test dmi
```

Expected: `2 passed`.

- [ ] **Step 4.6: Extend `BiosInfo`**

Edit `crates/hw-model/src/properties.rs`, at the end of `BiosInfo` (before the closing `}`), add:

```rust
    #[serde(default)]
    pub oem_strings: Vec<String>,
```

- [ ] **Step 4.7: Extend `bios_board_devices` signature and call sites**

Search for every call to `bios_board_devices(` in `crates/hw-probe/src/existing.rs`. In the current tree there are three: two sysfs-fallback paths inside `BiosProbe::probe` (lines ~4826 and ~4855) and one success path (line ~4907). Each already threads a growing list of "enrichment source" params. Add a new final parameter `oem_strings: Vec<String>`:

Existing signature (around line 5175):

```rust
fn bios_board_devices(
    dmi: DmiBiosBoardRecord,
    source: &str,
    source_kind: SourceKind,
    runtime: BiosRuntimeInfo,
    bios_language_source: Option<SourceEvidence>,
    memory_array_source: Option<SourceEvidence>,
    chipset_source: Option<SourceEvidence>,
) -> Vec<Device>
```

Replace with:

```rust
fn bios_board_devices(
    dmi: DmiBiosBoardRecord,
    source: &str,
    source_kind: SourceKind,
    runtime: BiosRuntimeInfo,
    bios_language_source: Option<SourceEvidence>,
    memory_array_source: Option<SourceEvidence>,
    chipset_source: Option<SourceEvidence>,
    oem_strings: Vec<String>,
) -> Vec<Device>
```

Inside `bios_board_devices`, the `BiosInfo { ... }` literal around line 5183–5199 already lists all its fields; extend it with `oem_strings,` (using shorthand since the parameter name matches the field). Order it after `installable_languages` / `currently_installed_language`:

```rust
DeviceProperties::Bios(BiosInfo {
    vendor: bios_vendor,
    version: dmi.bios_version,
    release_date: dmi.bios_release_date,
    smbios_version: dmi.smbios_version,
    rom_size: dmi.bios_rom_size,
    runtime_size: dmi.bios_runtime_size,
    address: dmi.bios_address,
    characteristics: dmi.bios_characteristics,
    bios_revision: dmi.bios_revision,
    firmware_revision: dmi.firmware_revision,
    firmware_type: runtime.firmware_type,
    secure_boot: runtime.secure_boot,
    language_description_format: dmi.bios_language_description_format,
    installable_languages: dmi.bios_installable_languages,
    currently_installed_language: dmi.bios_currently_installed_language,
    oem_strings,
}),
```

Update all three call sites: the two fallback paths pass `Vec::new()`; the success path passes the enriched vec (see next step).

- [ ] **Step 4.8: Wire `dmidecode -t 11` into `BiosProbe::probe`**

Inside `BiosProbe::probe` (line 4818), after the existing `chipset_source = enrich_dmi_chipset_family(...)` call around line 4903, add a parallel enrichment step. Follow the pattern of `enrich_dmi_bios_language` (line ~4905):

```rust
let oem_strings = read_dmi_oem_strings(ctx).await;
```

Add the helper function near `enrich_dmi_bios_language`:

```rust
async fn read_dmi_oem_strings(ctx: &ProbeContext<'_>) -> Vec<String> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("dmidecode", ["-t", "11"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return Vec::new();
    }
    parse_dmi_oem_strings(&result.stdout)
}
```

Import `parse_dmi_oem_strings` at the top of `existing.rs` alongside other `hw_parser::` imports.

Pass `oem_strings` into the success-path `bios_board_devices` call. Pass `Vec::new()` in both fallback paths.

- [ ] **Step 4.9: Workspace verify**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all green.

- [ ] **Step 4.10: Mark and commit**

Append `(fixed in <sha>)` to Q6 row in the gap report.

```bash
git add crates/hw-parser/src/dmi.rs \
        crates/hw-parser/src/lib.rs \
        crates/hw-parser/tests/dmi.rs \
        crates/hw-model/src/properties.rs \
        crates/hw-probe/src/existing.rs \
        crates/hw-testdata/fixtures/dmi/dmidecode-t11.txt \
        docs/component-gap-report-2026-07-11.md
git commit -m "feat(dmi): collect OEM Strings via dmidecode -t 11

Adds parse_dmi_oem_strings parser, BiosInfo.oem_strings model field, and
a BiosProbe enrichment step that runs dmidecode -t 11 alongside the
existing -t 13/-t 16 enrichments. 'Not Specified' sentinel values are
filtered; real OEM data (including the common 'Default string' entry)
is preserved.

Closes Q6 from component-gap-report-2026-07-11.md §14.1."
SHA=$(git rev-parse --short HEAD)
sed -i "s/(fixed in <sha>)/(fixed in ${SHA})/" docs/component-gap-report-2026-07-11.md
git add docs/component-gap-report-2026-07-11.md
git commit --amend --no-edit
```

---

## Task 5: Q7 — lsblk column extension

**Files:**
- Modify: `crates/hw-testdata/fixtures/storage/lsblk.json`
- Modify: `crates/hw-parser/src/storage.rs` (`LsblkDevice` struct + `parse_lsblk_json_result`)
- Modify: `crates/hw-parser/tests/storage.rs`
- Modify: `crates/hw-probe/src/existing.rs` (lsblk `CommandSpec` at line 3756)

**Interfaces:**
- Produces: `LsblkDevice` gains four `Option<String>` fields — `mountpoint`, `fstype`, `partuuid`, `label`. All `#[serde(default)]` so historical fixtures with the old column list still deserialize.
- Note: this task deliberately does not add fields to `StorageInfo`. The new columns are parsed and available on `LsblkDevice` for future consumers; downstream output shape is unchanged.

- [ ] **Step 5.1: Inspect and extend the lsblk fixture**

Read `crates/hw-testdata/fixtures/storage/lsblk.json`. Identify one child (partition) entry under the primary disk. Add the four new columns:

```json
{
  "name": "nvme0n1p2",
  "type": "part",
  "size": 511080529920,
  "mountpoint": "/",
  "fstype": "ext4",
  "partuuid": "12345678-90ab-cdef-1234-567890abcdef",
  "label": "root"
}
```

If multiple partitions exist, add the fields to at least one. Preserve existing partitions with `null` for these fields to prove the `#[serde(default)]` path works.

- [ ] **Step 5.2: Add the failing parser test**

Append to `crates/hw-parser/tests/storage.rs`:

```rust
#[test]
fn lsblk_parses_mountpoint_fstype_partuuid_label() {
    let input = hw_testdata::fixture("storage/lsblk.json");
    let records = hw_parser::parse_lsblk_json_result(&input).expect("parses");
    let root_partition = records
        .iter()
        .find(|d| d.mountpoint.as_deref() == Some("/"))
        .expect("root partition present");
    assert_eq!(root_partition.fstype.as_deref(), Some("ext4"));
    assert_eq!(
        root_partition.partuuid.as_deref(),
        Some("12345678-90ab-cdef-1234-567890abcdef"),
    );
    assert_eq!(root_partition.label.as_deref(), Some("root"));
}
```

- [ ] **Step 5.3: Run the test and confirm it fails**

```bash
cargo test -p hw-parser --test storage lsblk_parses_mountpoint_fstype_partuuid_label
```

Expected: compile error `no field 'mountpoint' on 'LsblkDevice'`.

- [ ] **Step 5.4: Extend `LsblkDevice`**

Edit `crates/hw-parser/src/storage.rs`. Locate the `LsblkDevice` struct (grep for `pub struct LsblkDevice`). Add four fields at the end:

```rust
    #[serde(default)]
    pub mountpoint: Option<String>,
    #[serde(default)]
    pub fstype: Option<String>,
    #[serde(default)]
    pub partuuid: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
```

Serde will map JSON keys `mountpoint`, `fstype`, `partuuid`, `label` directly. No custom `#[serde(rename)]` needed if the field names already match lsblk's JSON keys.

- [ ] **Step 5.5: Run the parser test and confirm it passes**

```bash
cargo test -p hw-parser --test storage lsblk_parses_mountpoint_fstype_partuuid_label
```

Expected: `1 passed`.

- [ ] **Step 5.6: Extend the lsblk command in `StorageProbe::probe`**

Edit `crates/hw-probe/src/existing.rs` line ~3756. Current invocation:

```rust
&CommandSpec::new(
    "lsblk",
    ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
),
```

Replace with:

```rust
&CommandSpec::new(
    "lsblk",
    ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV,MOUNTPOINT,FSTYPE,PARTUUID,LABEL"],
),
```

No changes to consumers of the resulting parsed records — `StorageInfo` remains untouched (per spec).

- [ ] **Step 5.7: Full workspace verify (this task most likely to trigger golden diffs)**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo build --release
```

If a storage golden test fails because it reads the updated `lsblk.json`, the fix depends on the failure mode:
- If the test asserts on `LsblkDevice.mountpoint == None`, it's a stale expectation — update to `Some("/".to_string())`.
- If the test asserts equality against a serialized `Vec<LsblkDevice>` blob and now has extra fields, regenerate the golden or add `#[serde(skip_serializing_if = "Option::is_none")]` to the four new fields on `LsblkDevice`. Prefer the skip-serializing approach since it keeps the golden stable and is a valid additive change.

If a downstream (e.g., `hw-output` / `hw-bindid`) test fails because the added JSON columns now surface into a report — this should NOT happen because `StorageInfo` is unchanged, but if it does, inspect the failure trace to confirm the field is not being smuggled through.

- [ ] **Step 5.8: Mark and commit**

Append `(fixed in <sha>)` to Q7 row.

```bash
git add crates/hw-parser/src/storage.rs \
        crates/hw-parser/tests/storage.rs \
        crates/hw-probe/src/existing.rs \
        crates/hw-testdata/fixtures/storage/lsblk.json \
        docs/component-gap-report-2026-07-11.md
git commit -m "fix(storage): extend lsblk columns with mountpoint/fstype/partuuid/label

lsblk was invoked with -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV which
threw away partition-level filesystem metadata. Adds MOUNTPOINT, FSTYPE,
PARTUUID, LABEL to the -o list and to LsblkDevice with #[serde(default)]
for backward compatibility. StorageInfo intentionally unchanged; the new
data lives on LsblkDevice for future consumers.

Closes Q7 from component-gap-report-2026-07-11.md §14.1."
SHA=$(git rev-parse --short HEAD)
sed -i "s/(fixed in <sha>)/(fixed in ${SHA})/" docs/component-gap-report-2026-07-11.md
git add docs/component-gap-report-2026-07-11.md
git commit --amend --no-edit
```

---

## Task 6: Open the PR

- [ ] **Step 6.1: Final workspace verification**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo build --release
```

Expected: all green.

- [ ] **Step 6.2: Push the branch**

```bash
git push -u origin fix/p0-gap-closeout
```

- [ ] **Step 6.3: Open the PR**

Use `gh pr create` (or the project's convention if it differs). Title and body:

```bash
gh pr create --title "fix: close out P0 gaps from component gap report" --body "$(cat <<'EOF'
## Summary

Closes five P0 gaps identified in `docs/component-gap-report-2026-07-11.md` §14.1:
- **Q1** Bluetooth parser recovers HCI/LMP version, manufacturer, class, features
- **Q3** MonitorInfo exposes `edid_hex`
- **Q4** GPU DriverInfo.version populated from `/sys/module/<driver>/version` with modinfo fallback
- **Q6** BiosInfo.oem_strings populated from `dmidecode -t 11`
- **Q7** lsblk `-o` extended with MOUNTPOINT/FSTYPE/PARTUUID/LABEL

Q2 (MTU) and Q5 (`ufs_spec_version`) from the same report section were verified as already fixed in-tree by later commits and are not included here.

All new model fields use `#[serde(default)]` so historical reports deserialize unchanged.

## Test plan

- [ ] `cargo test --workspace` green on Linux (CI)
- [ ] `cargo clippy --workspace -- -D warnings` green
- [ ] `cargo build --release` green
- [ ] Each commit stands alone — `git revert` on any single commit leaves the tree consistent

Spec: `docs/superpowers/specs/2026-07-12-p0-gap-closeout-design.md`
Plan: `docs/superpowers/plans/2026-07-12-p0-gap-closeout.md`
EOF
)"
```

- [ ] **Step 6.4: Report the PR URL back to the user**

The `gh pr create` output includes the PR URL. Surface it in the completion message.

---

## Verification Checklist

Run before declaring done:

- [ ] Branch is `fix/p0-gap-closeout`, five commits (plus optional amend commits from sha backfill), all authored on top of `master`.
- [ ] Every commit's diff is scoped to its own Q (`git log --stat` shows no cross-commit leakage).
- [ ] `cargo test --workspace` green.
- [ ] `cargo clippy --workspace -- -D warnings` green.
- [ ] `docs/component-gap-report-2026-07-11.md` has `(fixed in <short-sha>)` on rows Q1/Q3/Q4/Q6/Q7 with real shas.
- [ ] No modifications to files listed under "Do not touch" in Global Constraints.
- [ ] PR opened, URL captured.

