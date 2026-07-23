#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
grep -q 'schema_version.*qurbrix.hw.snapshot.contract.v1' "$root/docs/hardware-snapshot-golden-vectors.json"
test "$(grep -c '^|' "$root/docs/hardware-snapshot-field-mapping.md")" -ge 10
test "$(grep -c '^|' "$root/docs/hardware-snapshot-state-transitions.md")" -ge 10
awk -F, 'NR > 1 { if ($1 !~ /^[0-9]+$/ || $2 !~ /^[0-9]+$/ || $4 != 45 || $5 != 10) exit 1 } END { if (NR != 11) exit 1 }' "$root/docs/hardware-snapshot-performance-baseline.csv"
! rg -n 'TBD|TODO|待确认' "$root/docs/adr/0001-hardware-snapshot-v1.md" "$root/docs/hardware-snapshot-field-mapping.md" "$root/docs/hardware-snapshot-state-transitions.md"
echo 'hardware snapshot contract: PASS'
