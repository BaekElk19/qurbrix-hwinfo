#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

for command in rg; do
    command -v "$command" >/dev/null || {
        echo "missing architecture-check command: $command" >&2
        exit 1
    }
done

forbidden_cli='collect_scan_report|Command::new|std::process::Command|tokio::process::Command'
if rg -n "$forbidden_cli" crates/hw-cli/src; then
    echo "hw-cli bypasses the inventory observation service" >&2
    exit 1
fi

forbidden_bindid='hw_probe|hw_source|RealSourceRunner|SourceRunner|collect_bindid_report'
if rg -n "$forbidden_bindid" crates/hw-bindid/src crates/hw-bindid/Cargo.toml; then
    echo "hw-bindid owns a collection path" >&2
    exit 1
fi

test "$(rg -l 'ScanReport::empty\(\)' crates/*/src --glob '*.rs' | wc -l)" -eq 1
rg -q '^pub async fn collect_scan_report\(' crates/hw-collect/src/collector.rs
rg -q 'hw_collect::collect_scan_report' crates/hw-inventory/src/service.rs
rg -q 'observe_inventory' crates/hw-cli/src/main.rs
test -f docs/architecture/hardware-collection-call-graph.md

echo "single hardware collection path: PASS"
