# Qurbrix HW Info

[![CI](https://github.com/BaekElk19/qurbrix-hwinfo/actions/workflows/ci.yml/badge.svg)](https://github.com/BaekElk19/qurbrix-hwinfo/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/BaekElk19/qurbrix-hwinfo)](https://github.com/BaekElk19/qurbrix-hwinfo/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Qurbrix HW Info is a set of Rust crates for collecting, parsing, normalizing, and reporting Linux hardware information. It turns command output, `/proc`, `/sys`, PCI, USB, DMI, display, power, and peripheral data into a typed `ScanReport` plus flat JSON, JSONL, summary, and table views.

Chinese documentation is available in [README.zh-CN.md](README.zh-CN.md).

## Features

- Collect CPU, memory, BIOS/baseboard, monitor, storage, GPU, network, PCI, USB, audio, Bluetooth, input, camera, battery, printer, and CD-ROM information.
- Preserve source evidence for debugging, replay, and result comparison.
- Emit a typed `ScanReport` model for Rust callers.
- Emit flat JSON, JSONL, summary, and table views for scripts and agents.
- Provide fake source runners and fixture-driven parser/probe tests.
- Keep immutable, queryable hardware snapshots with UUIDv7 IDs, bindid v2,
  checked JSON artifacts, and a local SQLite history.

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
│   ├── hw-inventory/       # Snapshot state machine, SQLite projection, and artifacts
│   ├── hw-output/          # Flat JSON, JSONL, summary, table, and schema helpers
│   ├── hw-cli/             # qurbrix-hw CLI argument parsing and commands
│   └── hw-testdata/        # Parser fixture helpers
└── Cargo.toml              # Top-level library manifest
```

## Runtime Requirements

The target platform is Linux.

**Root is required for `scan`, `summary`, `table`, `bindid`, and `snapshot ensure`** — they refuse to run
otherwise with `root access is required for this command; rerun with sudo`. Use `sudo`
or run as root. The `list-kinds`, `schema`, and `sources` commands do not need root.

Collection quality depends on available commands and permissions:

- Basic system data: `lscpu`, `/proc/bus/input/devices`, `/proc/asound/cards`
- BIOS, baseboard, memory slots: `dmidecode` (requires root)
- Storage: `lsblk`
- Monitor/GPU: `xrandr`, `lspci`, `/sys/class/drm`
- Network: `ip`

When some commands are unavailable, the collector falls back to other sources where
possible. Returned fields may be less complete.

## Installation

### Prebuilt binaries

Download the latest release from
[GitHub Releases](https://github.com/BaekElk19/qurbrix-hwinfo/releases). Pick
the archive matching your machine:

| Archive | Architecture | glibc floor |
|---|---|---|
| `qurbrix-hw-<version>-x86_64-unknown-linux-gnu-glibc2.28.tar.gz` | 64-bit Intel/AMD | 2.28 |
| `qurbrix-hw-<version>-aarch64-unknown-linux-gnu-glibc2.28.tar.gz` | 64-bit ARM | 2.28 |
| `qurbrix-hw-<version>-loongarch64-unknown-linux-gnu-glibc2.36.tar.gz` | LoongArch64 | 2.36 |

Verify and install:

```bash
ARCHIVE="qurbrix-hw-0.1.4-x86_64-unknown-linux-gnu-glibc2.28" # choose from the table above
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf "${ARCHIVE}.tar.gz"
sudo install -m 0755 "${ARCHIVE}/qurbrix-hw" /usr/local/bin/
```

Prebuilt binaries are dynamically linked against glibc. The x86_64 and aarch64
artifacts are built with pinned `cargo-zigbuild` and require glibc **2.28** or
newer. The LoongArch64 artifact uses the pinned `cross` toolchain and requires
glibc **2.36** or newer. Every release job verifies that the
highest `GLIBC_*` symbol does not exceed the floor shown in the table. Older
distributions need to build from source.

### From source

```bash
cargo install --path .
```

## Build

```bash
cargo check --workspace
cargo test --workspace
```

## Commands

At a glance:

| Command      | Root? | What it does                                    | Output              |
|--------------|-------|-------------------------------------------------|---------------------|
| `scan`       | yes   | Collect all hardware and emit structured data   | JSON / JSONL / typed-JSON / summary-JSON |
| `summary`    | yes   | Human-readable device count per kind            | Plain text          |
| `table`      | yes   | Two-column table of devices (optionally by kind)| Plain text          |
| `bindid`     | yes   | Stable machine binding ID derived from hardware | JSON                |
| `list-kinds` | no    | List every device kind the scanner knows        | Text or JSON        |
| `schema`     | no    | Print scan output schema version                | Text                |
| `sources`    | no    | List raw sources used during collection         | JSON                |
| `snapshot`   | ensure only | Ensure, query, diff, or export snapshots  | Stable JSON         |

Global: `qurbrix-hw --help`, `qurbrix-hw <command> --help`, `qurbrix-hw --version`.

### `scan` — structured hardware report

```bash
sudo qurbrix-hw scan --format json --pretty
```

Flags:

- `--format json|jsonl|typed-json|summary-json` (default `json`)
  - `json` — flat schema, best for other tools
  - `jsonl` — one device per line, best for streaming
  - `typed-json` — internal Rust model shape (may change; not the stable contract)
  - `summary-json` — same counts as `summary` command in JSON
- `--pretty` — pretty-print JSON
- `--kind <k>` / `--exclude-kind <k>` — repeatable; e.g. `--kind cpu --kind memory`
- `--timeout 30s` — per-source timeout
- `--no-optional-sources` — skip optional/slow probes
- `--no-sources` — omit the raw `sources` block from the report
- `--no-warnings` — suppress non-fatal warnings

Example (truncated):

```json
{
  "schema_version": "qurbrix.hw.scan.v2",
  "status": "complete",
  "summary": { "device_count": 1, "counts_by_kind": {"cpu": 1}, "warning_count": 0 },
  "devices": [
    {
      "id": "cpu:0",
      "kind": "cpu",
      "name": "AMD Ryzen 7 5800H with Radeon Graphics",
      "properties": { "data": { "cores": 8, "threads": 16, ... } }
    }
  ]
}
```

### `summary` — quick device counts

```bash
sudo qurbrix-hw summary
```

```text
Status: Partial
Devices: 65
Warnings: 5
audio: 1
battery: 1
bios: 1
cpu: 1
gpu: 1
memory: 2
storage: 1
...
```

### `table` — devices as a table

```bash
sudo qurbrix-hw table                # all devices
sudo qurbrix-hw table --kind storage # one kind only
```

```text
KIND       ID                           NAME
storage    storage:dev:/dev/sda         VMware, VMware Virtual S
```

### `bindid` — stable machine ID

Derives a stable ID from motherboard/memory/storage/network identifiers. Useful for
license binding, telemetry deduplication, or fleet inventory. Missing components are
reported so callers can decide whether the ID is trustworthy for their use case.

```bash
sudo qurbrix-hw bindid --pretty
```

```json
{
  "schema_version": "qurbrix.hw.bindid.v1",
  "algorithm": "qurbrix-hw-bindid-sha1-hex16-v1",
  "status": "complete",
  "value": "a05173f4b72b4597",
  "required_kinds": ["system","motherboard","memory","storage","network"],
  "optional_kinds": ["gpu"],
  "covered_kinds": ["gpu","memory","motherboard","network","storage","system"],
  "missing_required_kinds": [],
  "missing_optional_kinds": []
}
```

### `list-kinds` — supported device kinds

```bash
qurbrix-hw list-kinds                # text, one per line
qurbrix-hw list-kinds --format json  # JSON array
```

```text
system
motherboard
bios
cpu
memory
storage
gpu
monitor
network
audio
bluetooth
input
camera
battery
printer
cdrom
usb
pci
other-pci
other-device
```

### `schema` / `sources`

```bash
qurbrix-hw schema             # -> qurbrix.hw.scan.v2
qurbrix-hw sources            # -> {"sources":[]}
```

## Hardware snapshots

Snapshots are on-demand observations, not live monitoring. Nothing watches
udev, netlink, USB, PCI, or network events between calls. Run `snapshot ensure`
again when an application needs a current inventory. This project does not
depend on `qurbrix-monitor` and stores no metric or event time series.

The default state directory is `/var/lib/qurbrix-hwinfo`:

```text
/var/lib/qurbrix-hwinfo/qurbrix_hwinfo.db
/var/lib/qurbrix-hwinfo/reports/<snapshot_id>.json
```

Directories use mode `0700`; the database and report artifacts use `0600`.
Use `--state-dir /controlled/path` for tests, containers, or a caller-owned
directory. The complete `ScanReport` is an immutable SHA-256-checked artifact;
SQLite contains normalized query tables and never a report/device JSON blob.

```bash
sudo qurbrix-hw snapshot ensure
sudo qurbrix-hw snapshot ensure --force --max-age 0s
qurbrix-hw snapshot show 01900000-0000-7000-8000-000000000000 --pretty
qurbrix-hw snapshot list --limit 30 --offset 0
qurbrix-hw snapshot diff <from-snapshot-id> <to-snapshot-id>
qurbrix-hw snapshot export <snapshot-id> --output ./scan-report.json
qurbrix-hw snapshot health --pretty
qurbrix-hw snapshot prune --dry-run
qurbrix-hw snapshot pin <snapshot-id>
qurbrix-hw snapshot mark-uploaded <snapshot-id>
```

`ensure` defaults to a 24-hour TTL and publishes a partial scan only when the
core identity contract is complete. Add `--reject-partial` for strict callers.
`export` refuses to replace an existing path unless `--overwrite` is supplied.
All snapshot stdout documents use schema `qurbrix.hw.snapshot.cli.v1`; diagnostics
go only to stderr. Snapshot exit codes are `0` success, `1` CLI/serialization,
`2` scan/policy failure, `4` permission, `5` not found, `6` storage/integrity,
and `124` lease timeout. Existing command exit codes remain unchanged.

Rust callers use the same store and artifact:

```rust,no_run
use qurbrix_hw::{ensure_snapshot, EnsureSnapshotOptions, InventoryStore};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let store = InventoryStore::open("/var/lib/qurbrix-hwinfo").await?;
let id = ensure_snapshot(&store, EnsureSnapshotOptions::default()).await?;
let snapshot = store.load_snapshot(id).await?.expect("published snapshot");
let report = store.load_scan_report(id).await?.expect("verified artifact");
assert_eq!(snapshot.snapshot_id, id);
assert_eq!(snapshot.device_count as usize, report.devices.len());
# Ok(()) }
```

Automatic retention keeps the current snapshot, every pinned or unuploaded
snapshot, and the newest 30 snapshots for each machine bind ID. `snapshot prune`
may delete only uploaded snapshots older than 90 days and outside that recent
window. Preview with `--dry-run`; adjust `--keep-recent` and `--max-age` when a
different bound is required. Artifact deletion failures remain in a retry queue.
`snapshot health` runs SQLite integrity/foreign-key checks, verifies every
artifact, reports orphans and metrics, and performs a passive WAL checkpoint.

To remove all local history, stop callers and remove the caller-selected state
directory (or `/var/lib/qurbrix-hwinfo`) as its owning user. Do not delete the
database separately from `reports/`; integrity checks intentionally reject that
split state.

## Integration Contract

Rust callers should depend on the top-level `qurbrix-hw` library facade. Other
languages should call the CLI and parse stdout JSON; this is the stable
cross-language boundary.

For machine callers:

- Prefer `qurbrix-hw scan --format json` for the flat external schema.
- Use `qurbrix-hw scan --format typed-json` only when you intentionally want the
  Rust model shape serialized as JSON.
- Do not parse human commands such as `summary` or `table`.
- Treat field order and whitespace as unstable.
- Treat `schema_version` as the compatibility marker. Breaking output changes
  require a new schema version; compatible changes may add fields.

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

## Contributing

Contributions are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) for local
setup, test commands, and commit conventions. Bug reports and feature
requests go to
[GitHub Issues](https://github.com/BaekElk19/qurbrix-hwinfo/issues); code
changes come through pull requests.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
