#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected_version="${1:-0.2.0}"
cd "$root"

for command in cargo jq rg git; do
    command -v "$command" >/dev/null || {
        echo "missing release-check command: $command" >&2
        exit 1
    }
done

metadata="$(cargo metadata --format-version 1 --locked --offline)"

jq -e --arg version "$expected_version" '
    [.packages[] | select(.source == null)] as $workspace
    | ($workspace | length > 0)
      and all($workspace[];
        .version == $version and .license == "MIT OR Apache-2.0")
' <<<"$metadata" >/dev/null

jq -e '
    [.packages[] | select(.license == null and .license_file == null)]
    | length == 0
' <<<"$metadata" >/dev/null

# A dependency expression containing a copyleft option remains compatible when
# it also offers a permissive OR branch (for example r-efi). Reject packages
# whose metadata offers only GPL-family/SSPL choices.
jq -e '
    [.packages[]
      | select(
          ((.license // "")
            | test("(^|[^A-Za-z])(AGPL|GPL|LGPL|SSPL)([^A-Za-z]|$)"; "i"))
          and (((.license // "")
            | test("MIT|Apache|BSD|ISC|Zlib|Unicode|Unlicense|CC0"; "i"))
            | not)
        )]
    | length == 0
' <<<"$metadata" >/dev/null

if rg -n 'qurbrix-monitor|udev|netlink' \
    Cargo.toml crates/*/Cargo.toml crates/*/src src tests; then
    echo "monitor, hotplug, or event-listener coupling found" >&2
    exit 1
fi

mapfile -t runtime_files < <(
    find "$root" \
        -path "$root/.git" -prune -o \
        -path "$root/target" -prune -o \
        \( -name '*.db' -o -name '*.db-wal' -o -name '*.db-shm' -o -name '*.tmp' \) \
        -print
)
if ((${#runtime_files[@]})); then
    printf 'unexpected runtime file: %s\n' "${runtime_files[@]}" >&2
    exit 1
fi

git diff --check
bash scripts/verify-hardware-snapshot-contract.sh

echo "hardware snapshot release ${expected_version}: PASS"
