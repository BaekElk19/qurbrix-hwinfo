# Snapshot V1 Field Mapping

The typed model is the source of truth. Empty strings and known placeholders
are treated as absent by the canonicalizer. Values marked `hot` are retained
in the legacy JSON artifact when present but are excluded from identity and
relational snapshot properties.

| Contract field | Source | Null/absence | Stable | Bind ID | Config FP | Projection |
|---|---|---|---|---|---|---|
| `machine_bind_id` | V2 canonical identity groups | no value when required group missing | yes | derived | input | snapshot header |
| `configuration_fingerprint` | V2 identity + kernel/firmware/driver | no value on probe failure | yes for one observation | no | derived | snapshot header/state |
| platform UUID/serial/product | system, motherboard properties | fallback in priority order | yes | yes | yes | identifiers/properties |
| CPU vendor/model/socket/topology | `CpuInfo` and device fields | fallback to architecture/topology | yes | yes | yes | identifiers/properties |
| memory serial/part/locator/capacity | `MemoryInfo` | fallback to part+locator or topology | yes | yes | yes | identifiers/properties |
| storage WWN/serial/model/path | `StorageInfo` and device fields | stable controller/path fallback | yes | yes | yes | identifiers/properties |
| physical permanent MAC/bus | `NetworkInfo`, PCI bus | exclude loopback/virtual/random | yes | yes | yes | identifiers/properties |
| GPU PCI vendor/device/subsystem | `BusInfo::Pci`, `GpuInfo` | trusted absence allowed | yes | optional | yes | identifiers/properties |
| kernel release | system metadata | omitted if unreadable | no | no | yes | source/config |
| BIOS vendor/version/date | `BiosInfo` | omitted if unreadable | no | no | yes | source/config |
| driver name/module/version | `DriverInfo` | omitted if unreadable | no | no | yes | device/source |
| temperature/frequency/usage/power | existing `DeviceProperties` | omitted from projection | hot | no | no | artifact only |
| IP/link/throughput/device node | probe runtime fields | omitted | volatile | no | no | nowhere |

Every externally serialized DTO includes an explicit schema/fingerprint
version. SQLite uses foreign keys and immutable snapshot rows; properties use a
typed value column and never a JSON blob.
