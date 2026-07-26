# Changelog

## 0.2.3 - 2026-07-26

- Kept inventory collection and immutable snapshots complete while applying CLI
  device/source/warning filters only to stdout.
- Made lease waiting progress-aware, protected live probe history during store
  recovery, and fenced direct snapshot publication with the scan lease.
- Applied `--timeout` to the quick probe, every source, and the full-scan global
  deadline.
- Defined filtered report status as the health of the underlying inventory
  observation, independent of which devices or warnings are hidden from stdout.

## 0.2.2 - 2026-07-23

- Added `observe_inventory`, the sole application service for hardware views.
- Changed `scan`, `summary`, `table`, and `bindid` to reuse or publish immutable inventory snapshots.
- Removed `snapshot ensure` and the independent bindid collector.
- Added a collection call graph and a CI architecture check.

## 0.2.0 - 2026-07-23

- Introduced immutable hardware snapshots, quick probes, retention, and snapshot maintenance commands.
