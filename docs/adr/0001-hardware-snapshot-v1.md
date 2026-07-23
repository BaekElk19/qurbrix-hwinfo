# ADR 0001: Hardware Snapshot V1 Contract

## Status

Accepted for the `0.2.0` implementation line.

## Context

The scanner already produces a typed `ScanReport`. Snapshot management needs a
stable identity and persistence boundary without changing existing scan output.
The database is a query projection; the complete report remains an immutable
JSON artifact.

## Decisions

1. `SnapshotId` is UUIDv7, generated in the application, serialized as a lower
   case hyphenated string and stored as `TEXT`.
2. `machine_bind_id` is the full SHA-256 digest of canonical V2 physical
   identity records. V1's `qurbrix-hw-bindid-sha1-hex16-v1` output remains
   available for compatibility. V2 required groups are platform, CPU, memory,
   storage and physical network; GPU is optional. A trusted absence marker is
   used only after successful enumeration.
3. `configuration_fingerprint` is a separate SHA-256 digest containing the V2
   machine identity plus versioned kernel, firmware, driver and stable firmware
   fields. It never contains IP addresses, link state, temperatures, usage,
   power, random MACs, device nodes or monitor/event data.
4. Quick probe is on-demand and core-only. It never starts a listener, task or
   daemon. Full scan is a thin call to `collect_scan_report`.
5. A published snapshot is immutable. The report is written to
   `reports/<snapshot_id>.json` through a same-filesystem temporary file,
   flush/fsync and atomic rename. SQLite stores the normalized projection and
   artifact metadata, never report/device JSON blobs.
6. `PublishIfCoreComplete` is the default partial policy. A failed full scan
   leaves the previous current pointer untouched. The default TTL is 24 hours.
7. The default state directory is `/var/lib/qurbrix-hwinfo/`; tests and callers
   may override it. Default retention protects current, pinned, unuploaded and
   the newest 30 snapshots per machine; uploaded snapshots older than 90 days
   may be removed.
8. Rust API and CLI are both supported. `qurbrix-monitor` is not a dependency,
   and no hotplug or time-series data is stored.

## Compatibility and privacy

Existing `collect_scan_report`, JSON/JSONL/summary/table output and exit codes
are unchanged. Stable identifiers can contain serials and MACs, so state files
default to private permissions and error logs contain only stable error codes
and redacted diagnostics.

## Alternatives considered

- Reusing GPL-derived Deepin source would require converting every repository
  declaration to GPL-3.0-or-later and adds a large review surface; it was not
  needed because the existing probe interfaces provide the required behavior.
- Storing the entire report in SQLite would make queries and migrations brittle;
  normalized projection plus a checked artifact preserves both use cases.
- A background watcher would violate the on-demand boundary and duplicate the
  monitor project's responsibility.

## Consequences

The service performs quick probe work on every call and full scan only when the
fingerprint, TTL or force option requires it. Filesystem and SQLite publication
are coordinated by a publish protocol; readers reject missing or tampered
artifacts instead of returning an apparently valid snapshot.
