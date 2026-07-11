# qurbrix-hw CLI Output Contract

`qurbrix-hw` is script/agent-first. Structured results are written to stdout. Logs and diagnostics are written to stderr.

This CLI is the stable cross-language API. Rust callers may use the top-level
`qurbrix-hw` library facade directly; non-Rust callers should execute the CLI
and parse stdout.

Compatibility rules:

- `schema_version` identifies the output contract.
- Compatible changes may add fields, device kinds, capabilities, warnings, or
  enum values.
- Breaking changes to field names, field meanings, required fields, or status
  semantics require a new `schema_version`.
- JSON object field order and whitespace are not stable.
- Human commands such as `summary` and `table` are not machine contracts.

## Default command

```bash
qurbrix-hw scan --format json
```

`scan --format json` emits one flat JSON object:

- `schema_version`: currently `qurbrix.hw.scan.v1`.
- `status`: `complete`, `partial`, or `failed`.
- `metadata`: scanner/system metadata object.
- `summary`: `device_count`, `counts_by_kind`, and `warning_count`.
- `devices`: array of flat device objects.
- `warnings`: array of scan-level warnings unless `--no-warnings` is used.

Each flat device object contains:

- `id`, `kind`, and `name`.
- Optional `vendor`, `model`, `serial`, `bus`, and `driver`.
- `capabilities`, `identifiers`, `properties`, `sources`, and `warnings`.

`properties` is a tagged object with `kind` and `data`. Device and source kind strings use kebab-case. Status strings use snake_case.

`--format typed-json` emits the typed `ScanReport` model. `--format jsonl` emits one flat device JSON object per line. `--format summary-json` emits only the `summary` object.

`--no-sources` removes source evidence from device objects. `--no-warnings` removes warning arrays from output; `status` can still be `partial` because status is computed before output filtering.

## Status

- `complete`: requested scan completed without material warnings.
- `partial`: usable report was produced, but at least one source was missing, failed, timed out, or produced partial data.
- `failed`: no valid report was produced.

`partial` returns exit code `0`.

## Bind ID command

```bash
sudo qurbrix-hw bindid --pretty
sudo qurbrix-hw bindid --timeout 30s
```

`bindid` emits one JSON object. It is a lightweight business binding ID for
routine reads and low-frequency hardware binding checks. It is intentionally
not named `fingerprint` and is not a complete machine fingerprint.

The component set is intentionally narrow and is inspired by the original
binding behavior:

- Required kinds: `system`, `motherboard`, `memory`, `storage`, `network`.
- Optional kinds: `gpu`.
- CPU and display/monitor are intentionally excluded.
- Network contributes MAC only; network type, interface, IP, speed, and
  link-state are excluded.

Output fields:

- `schema_version`: always `qurbrix.hw.bindid.v1`.
- `algorithm`: always `qurbrix-hw-bindid-sha1-hex16-v1`.
- `status`: `complete` or `failed`.
- `value`: 16-character lowercase SHA1 hex prefix when complete, otherwise
  `null`.
- `required_kinds`: `system`, `motherboard`, `memory`, `storage`, `network`.
- `optional_kinds`: `gpu`.
- `covered_kinds`: kinds included in the computed input.
- `missing_required_kinds`: required kinds missing after collection.
- `missing_optional_kinds`: optional kinds missing after collection.
- `component_keys`: normalized component key strings used as algorithm input.
- `warnings`: bindid warnings.

Algorithm summary:

- Values are normalized by trimming/collapsing whitespace and dropping empty
  values or placeholders.
- Component fields are sorted by field name.
- Component keys are sorted.
- Component keys are joined with `||`.
- The joined string is hashed with SHA1; `value` is the first 16 characters of
  lowercase hex.
- Values in component keys escape `%`, `|`, and `=`.

`bindid` requires root like other hardware access commands. A non-root
invocation exits `4` before probing and writes no JSON to stdout. Metadata
commands (`schema`, `list-kinds`, and `sources`) do not require root.

## Device kind strings

Supported kind strings are:

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

`qurbrix-hw list-kinds --format json` emits the same list as a JSON array.

## Schema command

```bash
qurbrix-hw schema
```

emits:

```json
{"schema_version":"qurbrix.hw.scan.v1"}
```

`qurbrix-hw schema --version` emits only `qurbrix.hw.scan.v1`.

## Sources command

```bash
qurbrix-hw sources --format json
```

emits:

```json
{"sources":[]}
```

Source introspection is reserved for future work. The command accepts only JSON
format so unsupported machine formats are rejected instead of silently ignored.

## Exit codes

| Exit code | Meaning |
| --- | --- |
| 0 | Scan succeeded, including partial reports; bindid completed |
| 1 | CLI argument error or serialization error |
| 2 | Scan failed and no valid report was generated; bindid failed because required kinds were missing after collection, with JSON still printed |
| 3 | Requested kind/source is unsupported |
| 4 | Permission failure prevents core scan or bindid probing; bindid stdout is empty |
