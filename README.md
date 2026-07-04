# Qurbrix HW Info

Qurbrix HW Info is a set of Rust crates for collecting, parsing, normalizing, and storing Linux hardware information. It turns command output, `/proc`, `/sys`, EDID, PCI, and network data into a unified `Inventory`, then derives a stable machine `bindid`, database component rows, and JSON payloads for reporting or printing.

Chinese documentation is available in [README.zh-CN.md](README.zh-CN.md).

## Features

- Collect CPU, memory, BIOS/baseboard, monitor, storage, GPU, and network information.
- Preserve raw command output for debugging, replay, and result comparison.
- Generate a stable machine `bindid` from hardware component keys.
- Convert hardware data into `ComponentRow` records compatible with the `component_records` table shape.
- Build formatted JSON with motherboard, CPU, memory, storage, network, and mirrored component-row data.
- Store component records with SQLite upsert support.

## Layout

```text
.
├── src/                    # Top-level qurbrix-hw facade for collect, bindid, row conversion, JSON normalization
├── crates/
│   ├── hw-model/           # Hardware models, Inventory, ComponentInfo trait
│   ├── hw-source/          # Command and file sources with timeout handling
│   ├── hw-parser/          # Parsers for lscpu, dmidecode, lsblk, xrandr, EDID, ip, ethtool, and more
│   ├── hw-collect/         # Collection orchestration that builds Inventory
│   ├── hw-store/           # bindid, ComponentRow mapping, and SQLite repository
│   ├── hw-merge/           # Parse-output merge entry points
│   └── hw-api/             # API/DBus draft module, still needs alignment with the current model
└── Cargo.toml              # Top-level library manifest
```

## Runtime Requirements

The target platform is Linux. Collection quality depends on available commands and permissions:

- Basic system data: `lscpu`, `cat /proc/cpuinfo`, `/proc/meminfo`
- BIOS, baseboard, memory slots: `dmidecode`, usually requiring root
- Storage: `lsblk`, `udevadm`
- Monitor/GPU: `xrandr`, `glxinfo`, `/sys/class/drm`
- Network: `ip`, `lspci`, `ethtool`

When some commands are unavailable, the collector tries to fall back to other sources where possible. Returned fields may be less complete.

## Current Build Prerequisite

The `Cargo.toml` files in this directory use `*.workspace = true` inheritance, but this checkout does not include the Cargo workspace root with `[workspace]` metadata. Running `cargo check` directly from this standalone directory currently fails.

To build the project, use one of these approaches:

- Put this directory back under the original Cargo workspace that provides `workspace.package` and `workspace.dependencies`.
- Or add the missing `[workspace]`, `[workspace.package]`, and `[workspace.dependencies]` configuration locally.

After the workspace is restored, use:

```bash
cargo check
cargo test
```

## Basic Usage

```rust
use qurbrix_hw::{
    build_formatprint_payload, collect, compute_bind_id, to_component_rows, CollectConfig,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CollectConfig::default();
    let inventory = collect(&config).await?;

    let bind_id = compute_bind_id(&inventory);
    let machine_sn = inventory.machine_serial();
    let rows = to_component_rows(&inventory, machine_sn, &bind_id);
    let payload = build_formatprint_payload(&inventory, &bind_id, machine_sn, &rows);

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}
```

## Storage Example

```rust
use hw_store::repo::ComponentRepo;

async fn save(rows: &[qurbrix_hw::ComponentRow]) -> anyhow::Result<()> {
    let repo = ComponentRepo::open_or_create("hardware.db").await?;
    repo.upsert_batch(rows).await?;
    Ok(())
}
```

`ComponentRepo` creates the `component_records` table and upserts rows using `(fd_CODE, fd_NAME, fd_SN, fd_INFO_EX10)` as the conflict key.

## Data Flow

1. `hw-source` runs commands or reads system files.
2. `hw-parser` parses raw text into `CpuInfo`, `MemoryInfo`, `BiosInfo`, and related structures.
3. `hw-collect` orchestrates collection and returns an `Inventory`.
4. The top-level `qurbrix-hw` crate computes `bindid`, creates `ComponentRow` values, and builds formatted JSON.
5. `hw-store` can write component rows to SQLite.

## Notes

- `dmidecode`, some `/sys` paths, and device details may require elevated permissions.
- Monitor collection uses EDID and optional `xrandr`; without a graphical session it still attempts sysfs reads.
- `bindid` is computed by sorting component keys, hashing them with SHA1, and taking the first 16 hex characters.
- `hw-api` currently contains an older API draft. Its types and collector calls need to be aligned with the current `Inventory` model before it is included in the main build.
