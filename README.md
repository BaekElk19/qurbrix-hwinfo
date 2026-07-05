# Qurbrix HW Info

Qurbrix HW Info is a set of Rust crates for collecting, parsing, normalizing, and reporting Linux hardware information. It turns command output, `/proc`, `/sys`, PCI, USB, DMI, display, power, and peripheral data into a typed `ScanReport` plus flat JSON, JSONL, summary, and table views.

Chinese documentation is available in [README.zh-CN.md](README.zh-CN.md).

## Features

- Collect CPU, memory, BIOS/baseboard, monitor, storage, GPU, network, PCI, USB, audio, Bluetooth, input, camera, battery, printer, and CD-ROM information.
- Preserve source evidence for debugging, replay, and result comparison.
- Emit a typed `ScanReport` model for Rust callers.
- Emit flat JSON, JSONL, summary, and table views for scripts and agents.
- Provide fake source runners and fixture-driven parser/probe tests.

## Layout

```text
.
├── src/                    # Top-level qurbrix-hw facade for collection and schema helpers
├── crates/
│   ├── hw-model/           # ScanReport, Device, DeviceKind, and property models
│   ├── hw-source/          # Command and file sources with timeout handling
│   ├── hw-parser/          # Parsers for lscpu, dmidecode, lsblk, xrandr, ip, lspci, lsusb, and more
│   ├── hw-probe/           # Category probes that turn parsed data into Device values
│   ├── hw-collect/         # Collection orchestration that builds ScanReport
│   ├── hw-output/          # Flat JSON, JSONL, summary, table, and schema helpers
│   ├── hw-cli/             # qurbrix-hw CLI argument parsing and commands
│   └── hw-testdata/        # Parser fixture helpers
└── Cargo.toml              # Top-level library manifest
```

## Runtime Requirements

The target platform is Linux. Collection quality depends on available commands and permissions:

- Basic system data: `lscpu`, `/proc/bus/input/devices`, `/proc/asound/cards`
- BIOS, baseboard, memory slots: `dmidecode`, usually requiring root
- Storage: `lsblk`
- Monitor/GPU: `xrandr`, `lspci`, `/sys/class/drm`
- Network: `ip`

When some commands are unavailable, the collector tries to fall back to other sources where possible. Returned fields may be less complete.

## Build

```bash
cargo check --workspace
cargo test --workspace
```

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

## Data Flow

1. `hw-source` runs commands or reads system files.
2. `hw-parser` parses raw text into compact source records.
3. `hw-probe` turns source records into typed `Device` values.
4. `hw-collect` orchestrates probes and returns a `ScanReport`.
5. `hw-output` converts reports into flat JSON, JSONL, summary, and table views.

## Notes

- `dmidecode`, some `/sys` paths, and device details may require elevated permissions.
- Monitor collection uses EDID and optional `xrandr`; without a graphical session it still attempts sysfs reads.
- `partial` reports are still intended to be machine-consumable.
- Logs and diagnostics should go to stderr; structured command output goes to stdout.
