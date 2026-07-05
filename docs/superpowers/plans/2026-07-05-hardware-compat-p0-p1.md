# Hardware Compat P0/P1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the hardware compatibility P0/P1 work: CPU three-source merge, architecture and vendor normalization, and monitor EDID enrichment.

**Architecture:** Keep the existing qurbrix layering: `hw-source` performs IO, `hw-parser` contains pure parsers and string/byte normalization, `hw-probe` orchestrates sources and builds `Device`, and `hw-model` owns serializable output structs. Deliver in three serial phases: P0 CPU merge, P1a normalization, P1b monitor EDID.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `tokio`, `async-trait`, existing `FakeSourceRunner`, no new runtime dependencies.

---

## Starting Point

This plan is written for the current workspace state after the gap report/spec work. The working tree may already contain a partial P0 implementation in:

- `crates/hw-model/src/properties.rs`
- `crates/hw-parser/src/cpu.rs`
- `crates/hw-probe/src/existing.rs`
- `crates/hw-probe/tests/existing_category_probes.rs`

When executing this plan, do not discard those changes. Treat Task 1 and Task 2 as a reconciliation step: add the tests first, then adjust the existing partial implementation until the tests pass.

## File Structure

### P0 CPU Three-Source Merge

- Modify `crates/hw-model/src/properties.rs`
  - Add `CpuInfo.current_freq_mhz`.
- Modify `crates/hw-parser/src/cpu.rs`
  - Extend `CpuRecord`.
  - Add `LshwCpuRecord`, `DmidecodeCpuRecord`, `MergedCpu`.
  - Add `parse_lshw_processor`, `parse_dmidecode_processor`, `merge_cpu_records`.
- Modify `crates/hw-probe/src/existing.rs`
  - Change `CpuProbe` from single-source `lscpu` to optional `lscpu`, `lshw -class processor`, and `dmidecode -t 4`.
- Modify `crates/hw-probe/Cargo.toml`
  - Add `hw-testdata` as a dev-dependency if probe tests use fixture files.
- Create `crates/hw-parser/tests/cpu_sources.rs`
  - Parser and merge tests using fixtures.
- Modify `crates/hw-probe/tests/existing_category_probes.rs`
  - CPU probe integration tests.
- Create `crates/hw-testdata/fixtures/cpu/*.txt`
  - Small hand-written CPU fixtures.

### P1a Normalization

- Create `crates/hw-parser/src/normalize/mod.rs`
- Create `crates/hw-parser/src/normalize/arch.rs`
- Create `crates/hw-parser/src/normalize/cpu_vendor.rs`
- Create `crates/hw-parser/src/normalize/gpu_vendor.rs`
- Modify `crates/hw-parser/src/lib.rs`
  - Export `normalize`.
- Modify `crates/hw-model/src/properties.rs`
  - Add `GpuInfo.vendor`.
- Modify `crates/hw-probe/src/existing.rs`
  - Apply CPU arch/vendor normalization and GPU vendor normalization.
- Create `crates/hw-parser/tests/normalize.rs`
  - Table-driven normalization tests.
- Create `crates/hw-testdata/fixtures/normalize/*.cases.txt`
  - Tab-separated normalization cases.

### P1b Monitor EDID

- Modify `crates/hw-source/src/result.rs`
  - Add `SourceBytesResult`.
- Modify `crates/hw-source/src/runner.rs`
  - Add `read_file_bytes` to `SourceRunner`, `RealSourceRunner`, and `FakeSourceRunner`.
- Modify `crates/hw-testdata/src/lib.rs`
  - Add `fixture_bytes`.
- Create `crates/hw-parser/src/edid.rs`
  - Add EDID parser.
- Modify `crates/hw-parser/src/lib.rs`
  - Export `edid`.
- Modify `crates/hw-parser/src/monitor.rs`
  - Add `parse_xrandr_verbose`.
- Create `crates/hw-parser/src/normalize/pnp.rs`
  - PNP ID lookup.
- Modify `crates/hw-model/src/properties.rs`
  - Add optional EDID fields to `MonitorInfo`.
- Modify `crates/hw-probe/src/existing.rs`
  - Change `MonitorProbe` to merge `xrandr --query`, `xrandr --verbose`, and `/sys/class/drm/*/edid`.
- Create `crates/hw-parser/tests/edid.rs`
- Create `crates/hw-parser/tests/monitor_verbose.rs`
- Modify `crates/hw-probe/tests/remaining_category_probes.rs`
  - Add monitor EDID integration coverage.

---

## Phase P0 - CPU Three-Source Merge

### Task 1: CPU Parser Fixtures and Tests

**Files:**
- Create: `crates/hw-testdata/fixtures/cpu/lscpu-intel-x86_64.txt`
- Create: `crates/hw-testdata/fixtures/cpu/lscpu-loongson-loongarch64.txt`
- Create: `crates/hw-testdata/fixtures/cpu/lshw-product-null.txt`
- Create: `crates/hw-testdata/fixtures/cpu/dmidecode-4-dual-socket.txt`
- Create: `crates/hw-parser/tests/cpu_sources.rs`

- [ ] **Step 1: Add CPU fixtures**

Create `crates/hw-testdata/fixtures/cpu/lscpu-intel-x86_64.txt`:

```text
# source: synthetic lscpu sample, redacted: yes
Architecture:            x86_64
CPU(s):                  16
Vendor ID:               GenuineIntel
Model name:              Intel(R) Core(TM) i7-1185G7
CPU family:              6
Model:                   140
Stepping:                1
CPU MHz:                 1800.000
CPU max MHz:             4800.0000
CPU min MHz:             400.0000
BogoMIPS:                5990.40
Virtualization:          VT-x
Core(s) per socket:      4
Socket(s):               1
Flags:                   fpu vme de pse tsc msr pae mce cx8 apic sep
```

Create `crates/hw-testdata/fixtures/cpu/lscpu-loongson-loongarch64.txt`:

```text
# source: synthetic lscpu sample, redacted: yes
Architecture:            loongarch64
CPU(s):                  32
Vendor ID:               Loongson
Model name:              Loongson-3A5000
Core(s) per socket:      16
Socket(s):               1
Flags:                   cpucfg lam ual fpu lsx lasx
```

Create `crates/hw-testdata/fixtures/cpu/lshw-product-null.txt`:

```text
# source: synthetic lshw sample, redacted: yes
  *-cpu
       description: CPU
       product: null
       vendor: Phytium
       version: Phytium D2000/8
```

Create `crates/hw-testdata/fixtures/cpu/dmidecode-4-dual-socket.txt`:

```text
# source: synthetic dmidecode sample, redacted: yes
Handle 0x0041, DMI type 4, 48 bytes
Processor Information
        Socket Designation: CPU 0
        Manufacturer: HiSilicon
        Version: Kunpeng 920
        Family: ARMv8
        Max Speed: 2600 MHz
        Current Speed: 2400 MHz
        Core Count: 48
        Thread Count: 48

Handle 0x0042, DMI type 4, 48 bytes
Processor Information
        Socket Designation: CPU 1
        Manufacturer: HiSilicon
        Version: Kunpeng 920
        Family: ARMv8
        Max Speed: 2600 MHz
        Current Speed: 2400 MHz
        Core Count: 48
        Thread Count: 48
```

- [ ] **Step 2: Write failing parser and merge tests**

Create `crates/hw-parser/tests/cpu_sources.rs`:

```rust
use hw_parser::{
    merge_cpu_records, parse_dmidecode_processor, parse_lscpu, parse_lshw_processor,
};
use hw_testdata::fixture;

#[test]
fn parse_lscpu_reads_extended_cpu_fields() {
    let cpu = parse_lscpu(&fixture("cpu/lscpu-intel-x86_64.txt"));

    assert_eq!(cpu.architecture.as_deref(), Some("x86_64"));
    assert_eq!(cpu.vendor.as_deref(), Some("GenuineIntel"));
    assert_eq!(cpu.model_name.as_deref(), Some("Intel(R) Core(TM) i7-1185G7"));
    assert_eq!(cpu.threads, Some(16));
    assert_eq!(cpu.cores_per_socket, Some(4));
    assert_eq!(cpu.sockets, Some(1));
    assert_eq!(cpu.cpu_mhz, Some(1800));
    assert_eq!(cpu.cpu_max_mhz, Some(4800));
    assert_eq!(cpu.cpu_min_mhz, Some(400));
    assert_eq!(cpu.cpu_family.as_deref(), Some("6"));
    assert_eq!(cpu.cpu_model.as_deref(), Some("140"));
    assert_eq!(cpu.stepping.as_deref(), Some("1"));
    assert!(cpu.flags.contains(&"fpu".to_string()));
    assert_eq!(cpu.virtualization.as_deref(), Some("VT-x"));
}

#[test]
fn parse_lshw_falls_back_from_null_product_to_version() {
    let lshw = parse_lshw_processor(&fixture("cpu/lshw-product-null.txt"));
    let merged = merge_cpu_records(None, Some(lshw), &[]);

    assert_eq!(merged.name.as_deref(), Some("Phytium D2000/8"));
    assert_eq!(merged.vendor.as_deref(), Some("Phytium"));
}

#[test]
fn parse_dmidecode_reads_multiple_processor_sockets() {
    let records = parse_dmidecode_processor(&fixture("cpu/dmidecode-4-dual-socket.txt"));

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].socket_designation.as_deref(), Some("CPU 0"));
    assert_eq!(records[0].manufacturer.as_deref(), Some("HiSilicon"));
    assert_eq!(records[0].version.as_deref(), Some("Kunpeng 920"));
    assert_eq!(records[0].max_speed_mhz, Some(2600));
    assert_eq!(records[0].current_speed_mhz, Some(2400));
    assert_eq!(records[0].core_count, Some(48));
    assert_eq!(records[0].thread_count, Some(48));
}

#[test]
fn merge_cpu_records_protects_loongson_name_and_uses_dmi_counts() {
    let lscpu = parse_lscpu(&fixture("cpu/lscpu-loongson-loongarch64.txt"));
    let dmi = parse_dmidecode_processor(&fixture("cpu/dmidecode-4-dual-socket.txt"));

    let merged = merge_cpu_records(Some(lscpu), None, &dmi);

    assert_eq!(merged.name.as_deref(), Some("Loongson-3A5000"));
    assert_eq!(merged.sockets, Some(2));
    assert_eq!(merged.cores, Some(96));
    assert_eq!(merged.threads, Some(96));
    assert_eq!(merged.current_freq_mhz, Some(2400));
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
cargo test -p hw-parser --test cpu_sources
```

Expected: FAIL if P0 is not fully implemented. In the current partial P0 workspace, any failure should point to missing field parsing, lshw fallback, DMI multi-socket parsing, or merge rules.

- [ ] **Step 4: Commit fixture/test red state**

```bash
git add crates/hw-testdata/fixtures/cpu crates/hw-parser/tests/cpu_sources.rs
git commit -m "test: cover cpu source parsing and merge rules"
```

If committing red tests is not acceptable in this branch policy, keep this step uncommitted and continue directly to Task 2.

### Task 2: CPU Parser and Merge Implementation

**Files:**
- Modify: `crates/hw-parser/src/cpu.rs`

- [ ] **Step 1: Implement or reconcile CPU parser types**

Ensure `crates/hw-parser/src/cpu.rs` defines these public structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CpuRecord {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub model_name: Option<String>,
    pub vendor: Option<String>,
    pub cores_per_socket: Option<u32>,
    pub sockets: Option<u32>,
    pub cpu_mhz: Option<u32>,
    pub cpu_max_mhz: Option<u32>,
    pub cpu_min_mhz: Option<u32>,
    pub cpu_family: Option<String>,
    pub cpu_model: Option<String>,
    pub stepping: Option<String>,
    pub bogomips: Option<String>,
    pub flags: Vec<String>,
    pub virtualization: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwCpuRecord {
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmidecodeCpuRecord {
    pub socket_designation: Option<String>,
    pub manufacturer: Option<String>,
    pub version: Option<String>,
    pub family: Option<String>,
    pub max_speed_mhz: Option<u32>,
    pub current_speed_mhz: Option<u32>,
    pub core_count: Option<u32>,
    pub thread_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MergedCpu {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub cores: Option<u32>,
    pub sockets: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    pub min_freq_mhz: Option<u32>,
    pub current_freq_mhz: Option<u32>,
    pub flags: Vec<String>,
}
```

- [ ] **Step 2: Implement parsers and merge rules**

The implementation must satisfy these signatures exactly:

```rust
pub fn parse_lscpu(input: &str) -> CpuRecord;
pub fn parse_lshw_processor(input: &str) -> LshwCpuRecord;
pub fn parse_dmidecode_processor(input: &str) -> Vec<DmidecodeCpuRecord>;
pub fn merge_cpu_records(
    lscpu: Option<CpuRecord>,
    lshw: Option<LshwCpuRecord>,
    dmi: &[DmidecodeCpuRecord],
) -> MergedCpu;
```

Required helper behavior:

```rust
fn parse_mhz(value: &str) -> Option<u32> {
    value
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<f32>().ok())
        .map(|value| value.round() as u32)
}

fn clean_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "Not Specified").then(|| value.to_string())
}
```

Required merge behavior:

```rust
// name: lscpu -> lshw -> dmi, but do not overwrite an existing Loongson name.
// lshw product containing "null" or "ARMv" must fall back to lshw version.
// vendor: lscpu -> lshw -> first dmi manufacturer.
// sockets: unique DMI socket designations -> lscpu sockets -> dmi.len().
// cores/threads: start from lscpu, then correct upward with sane DMI totals.
// frequencies: max = lscpu max -> dmi max; min = lscpu min; current = dmi current.
```

- [ ] **Step 3: Run parser tests**

Run:

```bash
cargo test -p hw-parser --test cpu_sources
```

Expected: PASS.

- [ ] **Step 4: Run existing parser tests**

Run:

```bash
cargo test -p hw-parser
```

Expected: PASS.

- [ ] **Step 5: Commit parser implementation**

```bash
git add crates/hw-parser/src/cpu.rs crates/hw-parser/tests/cpu_sources.rs crates/hw-testdata/fixtures/cpu
git commit -m "feat: merge cpu data from lscpu lshw and dmi"
```

### Task 3: CPU Probe Orchestration

**Files:**
- Modify: `crates/hw-model/src/properties.rs`
- Modify: `crates/hw-probe/src/existing.rs`
- Modify: `crates/hw-probe/Cargo.toml`
- Modify: `crates/hw-probe/tests/existing_category_probes.rs`

- [ ] **Step 1: Add model field**

Ensure `CpuInfo` includes:

```rust
pub current_freq_mhz: Option<u32>,
```

Place it next to `max_freq_mhz` and `min_freq_mhz`.

- [ ] **Step 2: Add probe dev-dependency if fixture files are used**

If `crates/hw-probe/tests/existing_category_probes.rs` reads fixtures via `hw_testdata::fixture`, add to `crates/hw-probe/Cargo.toml`:

```toml
[dev-dependencies]
hw-testdata = { path = "../hw-testdata" }
```

- [ ] **Step 3: Write failing CPU probe tests**

Add these tests to `crates/hw-probe/tests/existing_category_probes.rs`:

```rust
use hw_model::{DeviceKind, DeviceProperties};
use hw_probe::{CpuProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn cpu_probe_uses_dmi_when_lscpu_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "4"],
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
         \tSocket Designation: CPU 0\n\
         \tManufacturer: HiSilicon\n\
         \tVersion: Kunpeng 920\n\
         \tCurrent Speed: 2400 MHz\n\
         \tCore Count: 48\n\
         \tThread Count: 48\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Cpu);
    assert_eq!(result.devices[0].name, "Kunpeng 920");
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.source.as_deref() == Some("lscpu")));

    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.vendor.as_deref(), Some("HiSilicon"));
            assert_eq!(cpu.current_freq_mhz, Some(2400));
            assert_eq!(cpu.cores, Some(48));
            assert_eq!(cpu.threads, Some(48));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_reports_warnings_when_optional_sources_are_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "lscpu",
        std::iter::empty::<&str>(),
        "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.source.as_deref() == Some("lshw -class processor")));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.source.as_deref() == Some("dmidecode -t 4")));
}
```

- [ ] **Step 4: Run probe tests to verify failure**

Run:

```bash
cargo test -p hw-probe --test existing_category_probes
```

Expected: FAIL until `CpuProbe` runs all three sources and maps optional failures to warnings.

- [ ] **Step 5: Implement CPU probe orchestration**

In `crates/hw-probe/src/existing.rs`, import:

```rust
use hw_parser::{
    merge_cpu_records, parse_dmidecode_processor, parse_lscpu, parse_lshw_processor,
};
```

`CpuProbe::probe` must:

```rust
// 1. run lscpu with no args
// 2. run lshw -class processor
// 3. run dmidecode -t 4
// 4. parse successful non-empty outputs
// 5. convert failed optional sources into ProbeResult::source_failure(...).warnings
// 6. return no devices plus warnings only if all three parsed sources are empty
// 7. build one cpu:0 Device from merge_cpu_records(...)
// 8. add SourceEvidence for each successful source
```

When constructing `CpuInfo`, set:

```rust
CpuInfo {
    name: merged.name,
    vendor: merged.vendor,
    architecture: merged.architecture,
    cores: merged.cores,
    threads: merged.threads,
    sockets: merged.sockets,
    max_freq_mhz: merged.max_freq_mhz,
    min_freq_mhz: merged.min_freq_mhz,
    current_freq_mhz: merged.current_freq_mhz,
    flags: merged.flags,
}
```

- [ ] **Step 6: Run probe tests**

Run:

```bash
cargo test -p hw-probe --test existing_category_probes
```

Expected: PASS.

- [ ] **Step 7: Commit probe implementation**

```bash
git add crates/hw-model/src/properties.rs crates/hw-probe/src/existing.rs crates/hw-probe/Cargo.toml crates/hw-probe/tests/existing_category_probes.rs
git commit -m "feat: collect cpu data from optional fallback sources"
```

### Task 4: P0 Verification and Release Build

**Files:**
- No source changes expected unless verification reveals failures.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

Expected: no output and exit code 0.

- [ ] **Step 2: Run full tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 3: Run release build**

Run:

```bash
cargo build --release
```

Expected: PASS.

- [ ] **Step 4: Commit formatting or fixes**

If `cargo fmt` or verification changed files:

```bash
git add crates/hw-model/src/properties.rs crates/hw-parser/src/cpu.rs crates/hw-probe/src/existing.rs crates/hw-probe/tests/existing_category_probes.rs crates/hw-testdata/fixtures/cpu
git commit -m "test: verify cpu fallback compatibility"
```

If no files changed, do not create an empty commit.

---

## Phase P1a - Normalization

### Task 5: Normalization Fixtures and Tests

**Files:**
- Create: `crates/hw-testdata/fixtures/normalize/arch.cases.txt`
- Create: `crates/hw-testdata/fixtures/normalize/cpu-vendor-id.cases.txt`
- Create: `crates/hw-testdata/fixtures/normalize/cpu-name-inference.cases.txt`
- Create: `crates/hw-testdata/fixtures/normalize/gpu-vendor.cases.txt`
- Create: `crates/hw-parser/tests/normalize.rs`

- [ ] **Step 1: Add normalization case fixtures**

Create `crates/hw-testdata/fixtures/normalize/arch.cases.txt`:

```text
x86_64	x86_64
amd64	x86_64
i386	i386
i686	i386
aarch64	aarch64
arm64	aarch64
loongarch64	loongarch64
loongarch	loongarch64
sw_64	sw_64
mips64	mips64
mips64el	mips64
riscv64	riscv64
unknown-arch	
```

Create `crates/hw-testdata/fixtures/normalize/cpu-vendor-id.cases.txt`:

```text
GenuineIntel	Intel
AuthenticAMD	AMD
HygonGenuine	Hygon
CentaurHauls	Zhaoxin
Shanghai	Zhaoxin
UnknownVendor Corp	
```

Create `crates/hw-testdata/fixtures/normalize/cpu-name-inference.cases.txt`:

```text
Loongson-3A5000	Loongson
Phytium D2000/8	Phytium
Kunpeng 920	HiSilicon
HiSilicon Kirin 9006C	HiSilicon
Huawei Taishan	HiSilicon
Zhaoxin KaiXian KX-6640MA	Zhaoxin
Hygon C86 7185	Hygon
Sunway SW1621	Sunway
Intel Core i7	Intel
AMD Ryzen 7	AMD
ARM Cortex-A72	ARM
Mystery CPU	
```

Create `crates/hw-testdata/fixtures/normalize/gpu-vendor.cases.txt`:

```text
NVIDIA Corporation	NVIDIA
Advanced Micro Devices, Inc. [AMD/ATI]	AMD
Intel Corporation	Intel
Matrox Electronics Systems Ltd.	Matrox
ASPEED Technology, Inc.	ASPEED
VMware SVGA II Adapter	VMware
Red Hat, Inc. Virtio GPU	VirtIO
Loongson Technology	Loongson
Jingjia Micro Electronics	Jingjia Micro
JJM GPU	Jingjia Micro
Zhaoxin	Zhaoxin
Moore Threads Technology	Moore Threads
MThreads S80	Moore Threads
Innosilicon Fantasy	Innosilicon
Wuhan Digital Engineering Institute	WDE
Unknown GPU Vendor	
```

- [ ] **Step 2: Write failing normalization tests**

Create `crates/hw-parser/tests/normalize.rs`:

```rust
use hw_parser::normalize::{
    infer_cpu_vendor_from_name, normalize_arch, normalize_cpu_vendor_id, normalize_gpu_vendor,
};
use hw_testdata::fixture;

fn cases(path: &str) -> Vec<(String, Option<String>)> {
    fixture(path)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (input, expected) = line.split_once('\t').unwrap_or((line, ""));
            let expected = (!expected.is_empty()).then(|| expected.to_string());
            (input.to_string(), expected)
        })
        .collect()
}

#[test]
fn normalizes_arch_aliases() {
    for (input, expected) in cases("normalize/arch.cases.txt") {
        assert_eq!(normalize_arch(&input).map(str::to_string), expected, "{input}");
    }
}

#[test]
fn normalizes_cpu_vendor_ids() {
    for (input, expected) in cases("normalize/cpu-vendor-id.cases.txt") {
        assert_eq!(
            normalize_cpu_vendor_id(&input).map(str::to_string),
            expected,
            "{input}"
        );
    }
}

#[test]
fn infers_cpu_vendor_from_model_name() {
    for (input, expected) in cases("normalize/cpu-name-inference.cases.txt") {
        assert_eq!(
            infer_cpu_vendor_from_name(&input).map(str::to_string),
            expected,
            "{input}"
        );
    }
}

#[test]
fn normalizes_gpu_vendors() {
    for (input, expected) in cases("normalize/gpu-vendor.cases.txt") {
        assert_eq!(normalize_gpu_vendor(&input).map(str::to_string), expected, "{input}");
    }
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
cargo test -p hw-parser --test normalize
```

Expected: FAIL because `hw_parser::normalize` does not exist yet.

### Task 6: Normalization Module Implementation

**Files:**
- Create: `crates/hw-parser/src/normalize/mod.rs`
- Create: `crates/hw-parser/src/normalize/arch.rs`
- Create: `crates/hw-parser/src/normalize/cpu_vendor.rs`
- Create: `crates/hw-parser/src/normalize/gpu_vendor.rs`
- Modify: `crates/hw-parser/src/lib.rs`

- [ ] **Step 1: Add module exports**

Create `crates/hw-parser/src/normalize/mod.rs`:

```rust
pub mod arch;
pub mod cpu_vendor;
pub mod gpu_vendor;

pub use arch::normalize_arch;
pub use cpu_vendor::{infer_cpu_vendor_from_name, normalize_cpu_vendor_id};
pub use gpu_vendor::normalize_gpu_vendor;
```

Modify `crates/hw-parser/src/lib.rs`:

```rust
pub mod normalize;
pub use normalize::*;
```

- [ ] **Step 2: Implement architecture normalization**

Create `crates/hw-parser/src/normalize/arch.rs`:

```rust
pub fn normalize_arch(uname_m: &str) -> Option<&'static str> {
    match uname_m.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" => Some("x86_64"),
        "i386" | "i686" => Some("i386"),
        "aarch64" | "arm64" => Some("aarch64"),
        "loongarch64" | "loongarch" => Some("loongarch64"),
        "sw_64" => Some("sw_64"),
        "mips64" | "mips64el" => Some("mips64"),
        "riscv64" => Some("riscv64"),
        _ => None,
    }
}
```

- [ ] **Step 3: Implement CPU vendor normalization**

Create `crates/hw-parser/src/normalize/cpu_vendor.rs`:

```rust
pub fn normalize_cpu_vendor_id(vendor_id: &str) -> Option<&'static str> {
    match vendor_id.trim() {
        "GenuineIntel" => Some("Intel"),
        "AuthenticAMD" => Some("AMD"),
        "HygonGenuine" => Some("Hygon"),
        "CentaurHauls" | "Shanghai" => Some("Zhaoxin"),
        _ => None,
    }
}

pub fn infer_cpu_vendor_from_name(model_name: &str) -> Option<&'static str> {
    let name = model_name.trim().to_ascii_lowercase();
    if name.contains("loongson") {
        Some("Loongson")
    } else if name.contains("phytium") {
        Some("Phytium")
    } else if name.contains("kunpeng")
        || name.contains("hisilicon")
        || name.contains("kirin")
        || name.contains("huawei")
    {
        Some("HiSilicon")
    } else if name.contains("zhaoxin") {
        Some("Zhaoxin")
    } else if name.contains("hygon") {
        Some("Hygon")
    } else if name.contains("sunway") {
        Some("Sunway")
    } else if name.contains("intel") {
        Some("Intel")
    } else if name.contains("amd") {
        Some("AMD")
    } else if name.contains("arm") {
        Some("ARM")
    } else {
        None
    }
}
```

- [ ] **Step 4: Implement GPU vendor normalization**

Create `crates/hw-parser/src/normalize/gpu_vendor.rs`:

```rust
pub fn normalize_gpu_vendor(vendor: &str) -> Option<&'static str> {
    let vendor = vendor.trim().to_ascii_lowercase();
    if vendor.contains("nvidia") {
        Some("NVIDIA")
    } else if vendor.contains("advanced micro devices") || vendor.contains("amd") {
        Some("AMD")
    } else if vendor.contains("intel") {
        Some("Intel")
    } else if vendor.contains("matrox") {
        Some("Matrox")
    } else if vendor.contains("aspeed") {
        Some("ASPEED")
    } else if vendor.contains("vmware") {
        Some("VMware")
    } else if vendor.contains("red hat") || vendor.contains("virtio") {
        Some("VirtIO")
    } else if vendor.contains("loongson") {
        Some("Loongson")
    } else if vendor.contains("jingjia") || vendor.contains("jjm") {
        Some("Jingjia Micro")
    } else if vendor.contains("zhaoxin") {
        Some("Zhaoxin")
    } else if vendor.contains("moore threads") || vendor.contains("mthreads") {
        Some("Moore Threads")
    } else if vendor.contains("innosilicon") {
        Some("Innosilicon")
    } else if vendor.contains("wuhan digital engineering") {
        Some("WDE")
    } else {
        None
    }
}
```

- [ ] **Step 5: Run normalization tests**

Run:

```bash
cargo test -p hw-parser --test normalize
```

Expected: PASS.

- [ ] **Step 6: Commit normalization module**

```bash
git add crates/hw-parser/src/lib.rs crates/hw-parser/src/normalize crates/hw-parser/tests/normalize.rs crates/hw-testdata/fixtures/normalize
git commit -m "feat: add hardware normalization helpers"
```

### Task 7: Wire Normalization into CPU and GPU Probes

**Files:**
- Modify: `crates/hw-model/src/properties.rs`
- Modify: `crates/hw-probe/src/existing.rs`
- Modify: `crates/hw-probe/tests/existing_category_probes.rs`
- Modify: `crates/hw-probe/tests/remaining_category_probes.rs`

- [ ] **Step 1: Add GPU vendor model field**

Modify `GpuInfo` in `crates/hw-model/src/properties.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    pub vendor: Option<String>,
    pub memory_bytes: Option<u64>,
    pub current_resolution: Option<String>,
    pub max_resolution: Option<String>,
}
```

- [ ] **Step 2: Write failing probe normalization tests**

In `crates/hw-probe/tests/existing_category_probes.rs`, update the CPU test assertion:

```rust
match &result.devices[0].properties {
    DeviceProperties::Cpu(cpu) => {
        assert_eq!(cpu.vendor.as_deref(), Some("AMD"));
        assert_eq!(cpu.architecture.as_deref(), Some("x86_64"));
    }
    other => panic!("expected cpu properties, got {other:?}"),
}
```

In `crates/hw-probe/tests/remaining_category_probes.rs`, add a GPU vendor assertion to the existing GPU test:

```rust
match &gpu_device.properties {
    DeviceProperties::Gpu(gpu) => {
        assert_eq!(gpu.vendor.as_deref(), Some("Intel"));
    }
    other => panic!("expected gpu properties, got {other:?}"),
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
cargo test -p hw-probe --test existing_category_probes --test remaining_category_probes
```

Expected: FAIL until probes call normalization helpers and `GpuInfo.vendor` is populated.

- [ ] **Step 4: Wire CPU normalization**

In `crates/hw-probe/src/existing.rs`, import:

```rust
use hw_parser::{
    infer_cpu_vendor_from_name, normalize_arch, normalize_cpu_vendor_id,
};
```

After `merge_cpu_records(...)`, normalize:

```rust
let architecture = merged
    .architecture
    .as_deref()
    .and_then(normalize_arch)
    .map(str::to_string)
    .or(merged.architecture);

let vendor = merged
    .vendor
    .as_deref()
    .and_then(normalize_cpu_vendor_id)
    .or_else(|| lscpu_vendor.as_deref().and_then(normalize_cpu_vendor_id))
    .or_else(|| lshw_vendor.as_deref().and_then(normalize_cpu_vendor_id))
    .or_else(|| merged.name.as_deref().and_then(infer_cpu_vendor_from_name))
    .map(str::to_string)
    .or(merged.vendor);
```

`lscpu_vendor` and `lshw_vendor` are the raw vendor strings captured before calling `merge_cpu_records`. They preserve source-level vendor IDs that may be more normalizable than the final merged DMI manufacturer. Use `architecture` and `vendor` in `CpuInfo`.

- [ ] **Step 5: Wire GPU normalization**

In `GpuProbe`, import `normalize_gpu_vendor` and construct `GpuInfo`. The current PCI parser keeps the full lspci description in `gpu.device` and often leaves `gpu.vendor` empty, so use `gpu.device` as a fallback normalization input when `gpu.vendor` is absent. If neither value normalizes, preserve the original `gpu.vendor` when present, otherwise preserve the original `gpu.device` description.

```rust
let normalized_vendor = gpu
    .vendor
    .as_deref()
    .and_then(normalize_gpu_vendor)
    .or_else(|| gpu.device.as_deref().and_then(normalize_gpu_vendor))
    .map(str::to_string)
    .or_else(|| gpu.vendor.clone())
    .or_else(|| gpu.device.clone());

DeviceProperties::Gpu(GpuInfo {
    vendor: normalized_vendor,
    ..Default::default()
})
```

- [ ] **Step 6: Run probe tests**

Run:

```bash
cargo test -p hw-probe --test existing_category_probes --test remaining_category_probes
```

Expected: PASS.

- [ ] **Step 7: Run full P1a verification**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 8: Commit P1a wiring**

```bash
git add crates/hw-model/src/properties.rs crates/hw-probe/src/existing.rs crates/hw-probe/tests/existing_category_probes.rs crates/hw-probe/tests/remaining_category_probes.rs
git commit -m "feat: normalize cpu and gpu hardware vendors"
```

---

## Phase P1b - Monitor EDID

### Task 8: Binary Source and Fixture Support

**Files:**
- Modify: `crates/hw-source/src/result.rs`
- Modify: `crates/hw-source/src/runner.rs`
- Modify: `crates/hw-source/tests/fake_runner.rs`
- Modify: `crates/hw-testdata/src/lib.rs`

- [ ] **Step 1: Write failing binary runner test**

Add to `crates/hw-source/tests/fake_runner.rs`:

```rust
use std::path::Path;
use hw_source::{FakeSourceRunner, SourceRunner};

#[tokio::test]
async fn fake_runner_returns_registered_binary_file() {
    let runner = FakeSourceRunner::new().with_file_bytes("/sys/class/drm/card0-HDMI-A-1/edid", vec![0, 255, 1]);

    let result = runner
        .read_file_bytes(Path::new("/sys/class/drm/card0-HDMI-A-1/edid"))
        .await;

    assert!(result.is_success());
    assert_eq!(result.bytes, vec![0, 255, 1]);
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p hw-source fake_runner_returns_registered_binary_file
```

Expected: FAIL because `read_file_bytes` and `with_file_bytes` do not exist.

- [ ] **Step 3: Add byte result model**

Add to `crates/hw-source/src/result.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBytesResult {
    pub source: String,
    pub bytes: Vec<u8>,
    pub stderr: String,
    pub error_kind: Option<SourceErrorKind>,
}

impl SourceBytesResult {
    pub fn success(source: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            source: source.into(),
            bytes: bytes.into(),
            stderr: String::new(),
            error_kind: None,
        }
    }

    pub fn error(
        source: impl Into<String>,
        kind: SourceErrorKind,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            bytes: Vec::new(),
            stderr: stderr.into(),
            error_kind: Some(kind),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error_kind.is_none()
    }
}
```

Export it from `crates/hw-source/src/lib.rs` if needed:

```rust
pub use result::*;
```

- [ ] **Step 4: Add binary runner methods**

Modify `SourceRunner`:

```rust
async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult;
```

Implement in `RealSourceRunner` with `tokio::fs::read(path).await`. Implement in `FakeSourceRunner` by adding:

```rust
file_bytes: HashMap<PathBuf, SourceBytesResult>,
```

and:

```rust
pub fn with_file_bytes(mut self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> Self {
    let path = path.into();
    self.file_bytes.insert(
        path.clone(),
        SourceBytesResult::success(path.display().to_string(), bytes),
    );
    self
}
```

- [ ] **Step 5: Add byte fixture helper**

Modify `crates/hw-testdata/src/lib.rs`:

```rust
pub fn fixture_bytes(relative: impl AsRef<Path>) -> Vec<u8> {
    std::fs::read(fixture_path(relative)).expect("fixture exists")
}
```

- [ ] **Step 6: Run source tests**

Run:

```bash
cargo test -p hw-source
```

Expected: PASS.

- [ ] **Step 7: Commit binary source support**

```bash
git add crates/hw-source/src/result.rs crates/hw-source/src/runner.rs crates/hw-source/tests/fake_runner.rs crates/hw-testdata/src/lib.rs
git commit -m "feat: support binary hardware source files"
```

### Task 9: EDID Parser

**Files:**
- Create: `crates/hw-parser/src/edid.rs`
- Modify: `crates/hw-parser/src/lib.rs`
- Create: `crates/hw-parser/tests/edid.rs`

- [ ] **Step 1: Write failing EDID parser tests**

Create `crates/hw-parser/tests/edid.rs`:

```rust
use hw_parser::{parse_edid, EdidError};

fn sample_edid() -> Vec<u8> {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
    edid[8] = 0x05;
    edid[9] = 0xe3; // AOC
    edid[10] = 0x34;
    edid[11] = 0x12;
    edid[16] = 12;
    edid[17] = 32; // 2022
    edid[21] = 52;
    edid[22] = 32;
    edid[54] = 0x1d;
    edid[56] = 0x20;
    edid[58] = 0x30; // h active low
    edid[59] = 0x20; // h blank low
    edid[61] = 0x40; // v active low
    edid[62] = 0x10; // v blank low
    edid[64] = 0x21; // high bits
    edid[72] = 0x00;
    edid[73] = 0x00;
    edid[74] = 0x00;
    edid[75] = 0xfc;
    edid[76] = 0x00;
    edid[77..90].copy_from_slice(b"AOC TEST    \n");
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;
    edid
}

#[test]
fn parse_edid_extracts_identity_and_timing() {
    let edid = parse_edid(&sample_edid()).unwrap();

    assert_eq!(edid.manufacturer.as_deref(), Some("AOC"));
    assert_eq!(edid.product_code, Some(0x1234));
    assert_eq!(edid.week, Some(12));
    assert_eq!(edid.year, Some(2022));
    assert_eq!(edid.size_cm, Some((52, 32)));
    assert_eq!(edid.name.as_deref(), Some("AOC TEST"));
    let mode = edid.preferred_mode.unwrap();
    assert_eq!(mode.width, 1920);
    assert_eq!(mode.height, 1080);
}

#[test]
fn parse_edid_rejects_bad_checksum() {
    let mut bytes = sample_edid();
    bytes[127] = bytes[127].wrapping_add(1);

    assert_eq!(parse_edid(&bytes).unwrap_err(), EdidError::BadChecksum);
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p hw-parser --test edid
```

Expected: FAIL because `parse_edid` does not exist.

- [ ] **Step 3: Implement EDID parser**

Create `crates/hw-parser/src/edid.rs` with these public types:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdidRecord {
    pub manufacturer: Option<String>,
    pub product_code: Option<u16>,
    pub serial: Option<String>,
    pub name: Option<String>,
    pub week: Option<u8>,
    pub year: Option<u16>,
    pub size_cm: Option<(u8, u8)>,
    pub preferred_mode: Option<PreferredMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferredMode {
    pub width: u16,
    pub height: u16,
    pub refresh_hz: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdidError {
    TooShort,
    BadHeader,
    BadChecksum,
}
```

Required parser behavior:

```rust
pub fn parse_edid(bytes: &[u8]) -> Result<EdidRecord, EdidError> {
    // require at least 128 bytes
    // require header 00 ff ff ff ff ff ff 00
    // require first 128 bytes checksum sum % 256 == 0
    // manufacturer: bytes 8..10, 5-bit PNP ID
    // product_code: u16 little-endian bytes 10..12
    // week/year: bytes 16 and 17 + 1990
    // size_cm: bytes 21 and 22 if both non-zero
    // preferred_mode: detailed timing descriptor at byte 54 when pixel clock non-zero
    // descriptors 54, 72, 90, 108: tag 0xfc is monitor name, tag 0xff is serial
}
```

Add to `crates/hw-parser/src/lib.rs`:

```rust
pub mod edid;
pub use edid::*;
```

- [ ] **Step 4: Run EDID tests**

Run:

```bash
cargo test -p hw-parser --test edid
```

Expected: PASS.

- [ ] **Step 5: Commit EDID parser**

```bash
git add crates/hw-parser/src/edid.rs crates/hw-parser/src/lib.rs crates/hw-parser/tests/edid.rs
git commit -m "feat: parse monitor edid data"
```

### Task 10: xrandr Verbose Parser and PNP Lookup

**Files:**
- Modify: `crates/hw-parser/src/monitor.rs`
- Create: `crates/hw-parser/src/normalize/pnp.rs`
- Modify: `crates/hw-parser/src/normalize/mod.rs`
- Create: `crates/hw-parser/tests/monitor_verbose.rs`

- [ ] **Step 1: Write failing xrandr verbose test**

Create `crates/hw-parser/tests/monitor_verbose.rs`:

```rust
use hw_parser::{lookup_pnp_manufacturer, parse_xrandr_verbose};

#[test]
fn parse_xrandr_verbose_extracts_edid_bytes_by_connector() {
    let records = parse_xrandr_verbose(
        "HDMI-1 connected primary 1920x1080+0+0\n\
        \tEDID:\n\
        \t\t00ffffffffffff0005e3341200000000\n\
        \t\t0c200103803420780000000000000000\n\
        eDP-1 disconnected\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].connector, "HDMI-1");
    assert_eq!(&records[0].edid[0..8], &[0, 255, 255, 255, 255, 255, 255, 0]);
}

#[test]
fn pnp_lookup_returns_known_manufacturer_names() {
    assert_eq!(lookup_pnp_manufacturer("AOC"), Some("AOC International"));
    assert_eq!(lookup_pnp_manufacturer("ZZZ"), None);
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p hw-parser --test monitor_verbose
```

Expected: FAIL because `parse_xrandr_verbose` and PNP lookup do not exist.

- [ ] **Step 3: Implement xrandr verbose record parser**

Add to `crates/hw-parser/src/monitor.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrandrVerboseMonitorRecord {
    pub connector: String,
    pub edid: Vec<u8>,
}

pub fn parse_xrandr_verbose(input: &str) -> Vec<XrandrVerboseMonitorRecord> {
    // Track the latest connector from lines like "HDMI-1 connected ..."
    // When a line trimmed equals "EDID:", collect following indented hex lines.
    // Convert hex pairs into bytes and emit one record per connector with non-empty EDID.
}
```

Use this helper:

```rust
fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() != 2 {
                return None;
            }
            std::str::from_utf8(pair)
                .ok()
                .and_then(|value| u8::from_str_radix(value, 16).ok())
        })
        .collect()
}
```

- [ ] **Step 4: Implement PNP lookup**

Create `crates/hw-parser/src/normalize/pnp.rs`:

```rust
pub fn lookup_pnp_manufacturer(id: &str) -> Option<&'static str> {
    match id.trim().to_ascii_uppercase().as_str() {
        "AOC" => Some("AOC International"),
        "AUS" => Some("ASUSTek Computer"),
        "ACR" => Some("Acer"),
        "ACI" => Some("Asus"),
        "BNQ" => Some("BenQ"),
        "CMN" => Some("Chi Mei"),
        "CMO" => Some("Chi Mei Optoelectronics"),
        "DEL" => Some("Dell"),
        "EIZ" => Some("EIZO"),
        "FUS" => Some("Fujitsu"),
        "GSM" => Some("LG Electronics"),
        "HPN" | "HWP" => Some("HP"),
        "LEN" => Some("Lenovo"),
        "LGD" | "LPL" | "LGP" => Some("LG Display"),
        "MSI" => Some("Micro-Star International"),
        "NEC" => Some("NEC"),
        "PHL" => Some("Philips"),
        "SAM" | "SEC" => Some("Samsung"),
        "SHP" => Some("Sharp"),
        "SNY" => Some("Sony"),
        "VSC" => Some("ViewSonic"),
        "HSD" => Some("HannStar Display"),
        "BOE" => Some("BOE"),
        "CPT" => Some("Chunghwa Picture Tubes"),
        "AMR" => Some("AmTRAN"),
        "CHI" => Some("Chimei Innolux"),
        _ => None,
    }
}
```

Export from `normalize/mod.rs`:

```rust
pub mod pnp;
pub use pnp::lookup_pnp_manufacturer;
```

- [ ] **Step 5: Run monitor parser tests**

Run:

```bash
cargo test -p hw-parser --test monitor_verbose
```

Expected: PASS.

- [ ] **Step 6: Commit monitor parser helpers**

```bash
git add crates/hw-parser/src/monitor.rs crates/hw-parser/src/normalize/mod.rs crates/hw-parser/src/normalize/pnp.rs crates/hw-parser/tests/monitor_verbose.rs
git commit -m "feat: parse xrandr verbose edid blocks"
```

### Task 11: Monitor Model and Probe EDID Merge

**Files:**
- Modify: `crates/hw-model/src/properties.rs`
- Modify: `crates/hw-probe/src/existing.rs`
- Modify: `crates/hw-probe/tests/remaining_category_probes.rs`

- [ ] **Step 1: Add monitor EDID fields**

Modify `MonitorInfo`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MonitorInfo {
    pub connector: Option<String>,
    pub resolution: Option<String>,
    pub size_mm: Option<(u32, u32)>,
    pub production_date: Option<String>,
    pub manufacturer: Option<String>,
    pub manufacturer_name: Option<String>,
    pub product: Option<String>,
    pub product_code: Option<u16>,
    pub serial: Option<String>,
    pub manufactured_year: Option<u16>,
    pub manufactured_week: Option<u8>,
    pub size_cm: Option<(u8, u8)>,
    pub preferred_width: Option<u16>,
    pub preferred_height: Option<u16>,
    pub preferred_refresh_hz: Option<u16>,
}
```

- [ ] **Step 2: Write failing monitor probe test**

Add to `crates/hw-probe/tests/remaining_category_probes.rs`:

```rust
use hw_model::DeviceProperties;
use hw_probe::{MonitorProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::{path::PathBuf, time::Duration};

#[tokio::test]
async fn monitor_probe_uses_sysfs_edid_when_xrandr_verbose_is_missing() {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8] = 0x05;
    edid[9] = 0xe3;
    edid[16] = 12;
    edid[17] = 32;
    edid[21] = 52;
    edid[22] = 32;
    edid[72] = 0;
    edid[73] = 0;
    edid[74] = 0;
    edid[75] = 0xfc;
    edid[76] = 0;
    edid[77..90].copy_from_slice(b"AOC TEST    \n");
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;

    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, edid);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(monitor.manufacturer_name.as_deref(), Some("AOC International"));
            assert_eq!(monitor.product.as_deref(), Some("AOC TEST"));
            assert_eq!(monitor.manufactured_year, Some(2022));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run test to verify failure**

Run:

```bash
cargo test -p hw-probe monitor_probe_uses_sysfs_edid_when_xrandr_verbose_is_missing
```

Expected: FAIL until `MonitorProbe` reads sysfs EDID and merges parsed fields.

- [ ] **Step 4: Implement monitor EDID merge**

In `MonitorProbe`:

```rust
// Run xrandr --query. If it succeeds, use it for connected monitors and resolution.
// Run xrandr --verbose. If it succeeds, parse EDID bytes by connector.
// Glob /sys/class/drm/*/edid and read bytes with read_file_bytes.
// Normalize sysfs names: card0-HDMI-A-1 -> HDMI-1, card0-eDP-1 -> eDP-1.
// Prefer xrandr verbose EDID over sysfs EDID for the same connector.
// Parse EDID; on parse error push one warning and keep the monitor device.
```

EDID fields map to `MonitorInfo`:

```rust
manufacturer: edid.manufacturer.clone(),
manufacturer_name: edid
    .manufacturer
    .as_deref()
    .and_then(lookup_pnp_manufacturer)
    .map(str::to_string),
product: edid.name,
product_code: edid.product_code,
serial: edid.serial,
manufactured_year: edid.year,
manufactured_week: edid.week,
size_cm: edid.size_cm,
preferred_width: edid.preferred_mode.as_ref().map(|mode| mode.width),
preferred_height: edid.preferred_mode.as_ref().map(|mode| mode.height),
preferred_refresh_hz: edid.preferred_mode.as_ref().map(|mode| mode.refresh_hz),
```

- [ ] **Step 5: Run monitor probe tests**

Run:

```bash
cargo test -p hw-probe --test remaining_category_probes
```

Expected: PASS.

- [ ] **Step 6: Commit monitor EDID merge**

```bash
git add crates/hw-model/src/properties.rs crates/hw-probe/src/existing.rs crates/hw-probe/tests/remaining_category_probes.rs
git commit -m "feat: enrich monitor data from edid"
```

### Task 12: Final Verification

**Files:**
- No source changes expected unless verification reveals failures.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

Expected: exit code 0.

- [ ] **Step 2: Run full test suite**

Run:

```bash
cargo test --workspace
```

Expected: every crate and doc test passes.

- [ ] **Step 3: Run release build**

Run:

```bash
cargo build --release
```

Expected: release build passes.

- [ ] **Step 4: Run CLI smoke test**

Run:

```bash
target/release/qurbrix-hw list-kinds
```

Expected: command exits 0 and includes `cpu`, `gpu`, and `monitor`.

- [ ] **Step 5: Review git status**

Run:

```bash
git status --short
```

Expected: only intended P0/P1 files are modified or staged. Do not revert unrelated pre-existing files such as `docs/hardware-compatibility-gap-report.md`.

- [ ] **Step 6: Commit final verification fixes if needed**

If verification required fixes:

```bash
git add crates docs/superpowers/plans/2026-07-05-hardware-compat-p0-p1.md
git commit -m "test: verify hardware compatibility p0 p1"
```

If verification did not change files, do not create an empty commit.

---

## Self-Review

### Spec Coverage

- P0 CPU three-source merge: covered by Tasks 1-4.
- P1a arch alias, CPU vendor alias, GPU vendor alias: covered by Tasks 5-7.
- P1b EDID parser, PNP lookup, xrandr verbose, sysfs EDID, monitor model fields: covered by Tasks 8-11.
- Cross-phase verification: covered by Task 12.

### Explicit Deferrals

- P2 network, USB, storage, DMI sysfs fallback are not in this plan.
- P3 optional `glxinfo`, `lshw`, `hwinfo` heavy sources are not in this plan.
- Virtual machine device classification is not in this plan.

### Placeholder Scan

The plan intentionally avoids placeholder markers and vague implementation steps. Steps that contain parser internals include exact signatures and required behavior; implementers must keep parser IO-free and probe orchestration IO-only.

### Type Consistency

- `current_freq_mhz` is added to `CpuInfo` in P0 and used by `CpuProbe`.
- `GpuInfo.vendor` is added in P1a and used by `GpuProbe`.
- `SourceBytesResult` is added in P1b and used only for binary EDID source files.
- `lookup_pnp_manufacturer` lives under `hw_parser::normalize` and is used by `MonitorProbe`.

## Execution Options

Plan complete and saved to `docs/superpowers/plans/2026-07-05-hardware-compat-p0-p1.md`. Two execution options:

1. **Subagent-Driven (recommended)** - dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - execute tasks in this session using `executing-plans`, batch execution with checkpoints.
