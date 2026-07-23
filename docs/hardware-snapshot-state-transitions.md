# Snapshot State Transitions

| From | Observation | Action | Result/current pointer |
|---|---|---|---|
| Start/no current | quick succeeds, core complete | full scan and publish | new `Published(id)` |
| Start/current | bind + config equal, age < TTL, no force | record quick success | `Reused(current)` |
| Start/current | force, TTL expired, or either fingerprint differs | full scan and publish | `Published(new)`; old readable |
| Start/current | quick fails | attempt full scan | publish new or `Failed(old)` |
| Any | full scan failed | record error | `Failed(old)`; pointer unchanged |
| Any | partial + core complete + default policy | publish with warnings | `Published(new, partial)` |
| Any | partial + incomplete core or Reject policy | reject | `Failed(old)`; pointer unchanged |
| Any | artifact serialization/fsync/hash fails | remove temp/artifact | `Failed(old)`; no visible row |
| Any | SQLite transaction fails | rollback and clean artifact | `Failed(old)`; no partial rows |
| Any | artifact missing/hash/schema mismatch on read | reject read and probe again | old row remains but not reusable |
| Any | concurrent caller waits for lease | re-read pointer/fingerprint | one publish; other reuses equivalent id |
| Any | stale running lease | mark failed/recover orphan | pointer unchanged; next call proceeds |

Invariants: published rows and their device projections are immutable; only the
single `inventory_state` row changes; a current pointer references a complete
database projection with a verified artifact; a failed attempt never overwrites
the previous current machine or configuration identity.
