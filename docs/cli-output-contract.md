# qurbrix-hw CLI Output Contract

`qurbrix-hw` is script/agent-first. Structured results are written to stdout. Logs and diagnostics are written to stderr.

## Default command

```bash
qurbrix-hw scan --format json
```

## Status

- `complete`: requested scan completed without material warnings.
- `partial`: usable report was produced, but at least one source was missing, failed, timed out, or produced partial data.
- `failed`: no valid report was produced.

`partial` returns exit code `0`.

## Device kind strings

Kind strings use kebab-case. Examples:

- `cpu`
- `storage`
- `audio`
- `bluetooth`
- `other-pci`
- `other-device`

## Exit codes

| Exit code | Meaning |
| --- | --- |
| 0 | Scan succeeded, including partial reports |
| 1 | CLI argument error or serialization error |
| 2 | Scan failed and no valid report was generated |
| 3 | Requested kind/source is unsupported |
| 4 | Permission failure prevents core scan |
| 124 | Timeout |
