# Hardware Collection Call Graph

This is the production collection boundary for v0.2.2. Tests may provide an
`InventoryScanner` fake, but do not add another production collection path.

```text
qurbrix-hw scan / summary / table / bindid
                    |
                    v
          hw-inventory::observe_inventory
                    |
       +------------+-------------+
       |                          |
       v                          v
quick_probe                RealInventoryScanner::full_scan
hw-inventory::probe                  |
       |                              v
       |                    hw-collect::collect_scan_report
       |                              |
       |                              v
       |                       hw-probe -> hw-parser -> hw-source
       |
       +--> InventoryStore scan/probe history and immutable artifacts
```

| Entry point | Final collection function | Reads raw sources | Produces `ScanReport` | Writes inventory | Decision |
|---|---|---:|---:|---:|---|
| `scan` | `observe_inventory` | No | Formats stored observation | Yes | Keep |
| `summary` | `observe_inventory` | No | Formats stored observation | Yes | Keep |
| `table` | `observe_inventory` | No | Formats stored observation | Yes | Keep |
| `bindid` | `observe_inventory` | No | Derives SHA-1 business ID from observation | Yes | Keep |
| `snapshot` query/maintenance | Store query APIs | No | Loads verified stored report when needed | As requested | Keep |
| `quick_probe` | `hw-inventory::probe::quick_probe` | Yes, limited identity sources | No | Probe history only | Keep |
| `RealInventoryScanner::full_scan` | `hw-collect::collect_scan_report` | Indirectly | Yes | Via observer | Keep |
| `hw-collect::collect_scan_report` | Probe graph | Indirectly | Yes | No | Keep as the sole collector |
| Former `snapshot ensure` | Removed | N/A | N/A | N/A | Delete |
| Former bindid collector | Removed | N/A | N/A | N/A | Delete |

`observe_inventory` creates a quick `scan_run` for every hardware observation.
When the identity and configuration fingerprint match the current verified
snapshot, it returns that immutable report. A changed fingerprint, a forced
scan, an expired snapshot, or a quick-probe failure leads to the one full scan
call. Every full scan is recorded and every usable core-complete report is
persisted and promoted to `current_snapshot`.
