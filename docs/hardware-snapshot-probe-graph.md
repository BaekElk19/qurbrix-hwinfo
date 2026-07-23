# Full Scan Probe and Source Graph

All probes are read-only and may run concurrently. The fixed list index remains
the merge order, so completion order cannot affect devices, warnings, sources or
consumed PCI references.

| Probe group | Probes | Shared source examples | Merge dependency |
|---|---|---|---|
| Platform | system, BIOS, CPU, memory | dmidecode, `/sys`, `/proc` | none |
| Bus inventory | PCI, USB | lspci, lsusb, `/sys/bus` | consumed IDs applied after all probes |
| Display | GPU, monitor | lspci, lshw, xrandr | consumed IDs applied after all probes |
| Storage/network | storage, network | lspci, lshw, sysfs | consumed IDs applied after all probes |
| Peripherals | audio, Bluetooth, input, camera, battery, printer, CD-ROM | lspci, lshw, hwinfo, proc/sysfs | consumed IDs applied after all probes |

Within one scan, command keys contain the executable, exact argument vector and
timeout. File, byte-file, canonical-path and glob keys use normalized `PathBuf`
or pattern values. Concurrent equivalent requests share a `OnceCell`, including
failures, so every consumer observes the same result.

Only the explicit read-only command allowlist in `hw_source::is_cacheable_command`
is cached. Unknown commands are treated as potentially side-effecting and are
never cached. All external commands, cached or not, acquire the configurable
semaphore; the default ceiling is four. No probe needs a serial whitelist because
probes do not mutate shared state. Publication remains a separate short SQLite
transaction after scanning.

The global deadline is propagated into source timeouts and also cancels unfinished
probe futures. Real child processes use Tokio `kill_on_drop`; source and test
activity use drop guards so cancellation cannot leak permits or active counters.
