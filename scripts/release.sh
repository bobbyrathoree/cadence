#!/usr/bin/env bash
set -euo pipefail

unset APPLE_SIGNING_IDENTITY
unset APPLE_CERTIFICATE
unset APPLE_CERTIFICATE_PASSWORD
unset APPLE_ID
unset APPLE_PASSWORD
unset APPLE_TEAM_ID

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="aarch64-apple-darwin"
target_dir="$root_dir/target/$target/release"
bundle_dir="$target_dir/bundle"
app_path="$bundle_dir/macos/Cadence.app"
sidecar_source="$target_dir/cadence-mcp"
staged_sidecar="$root_dir/src-tauri/binaries/cadence-mcp-$target"
smoke_path="$root_dir/target/release/cadence-mcp-smoke"
hash_file="$bundle_dir/release-cdhashes.env"
obsolete_harness="$target_dir/api_fatal_harness"
version="$(
  node -e '
    const fs = require("node:fs");
    const config = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(config.version);
  ' "$root_dir/src-tauri/tauri.conf.json"
)"
dmg_path="$bundle_dir/dmg/Cadence_${version}_aarch64.dmg"
release_tmp="$(mktemp -d)"

cleanup() {
  rm -rf "$release_tmp"
  rm -f "$staged_sidecar"
  rmdir "$root_dir/src-tauri/binaries" 2>/dev/null || true
}
trap cleanup EXIT

cdhash() {
  local candidate="$1"
  local hash
  hash="$(
    codesign -dv --verbose=4 "$candidate" 2>&1 |
      sed -n 's/^CDHash=//p' |
      head -n 1
  )"
  [[ -n "$hash" ]] || {
    printf 'Could not read CDHash from %s\n' "$candidate" >&2
    return 1
  }
  printf '%s' "$hash"
}

cd "$root_dir"
"$root_dir/scripts/check-versions.sh"

printf 'Building arm64 cadence-mcp sidecar...\n'
cargo build --release -p cadence-mcp --target "$target" --no-default-features
[[ -f "$sidecar_source" ]] || {
  printf 'Expected sidecar was not produced: %s\n' "$sidecar_source" >&2
  exit 1
}

printf 'Ad-hoc signing and staging cadence-mcp...\n'
codesign -s - --force "$sidecar_source"
mkdir -p "$(dirname "$staged_sidecar")"
cp "$sidecar_source" "$staged_sidecar"

printf 'Building application bundle with the release-only sidecar overlay...\n'
# A pre-fix release may have left this auto-discovered test binary behind.
rm -f "$obsolete_harness" "$obsolete_harness.d"
rm -rf "$app_path"
npm run tauri -- build \
  --target "$target" \
  --ci \
  --no-sign \
  --config src-tauri/tauri.release.conf.json \
  --bundles app
[[ ! -e "$obsolete_harness" ]] || {
  printf 'Release build unexpectedly produced test harness: %s\n' \
    "$obsolete_harness" >&2
  exit 1
}
[[ -d "$app_path" ]] || {
  printf 'Expected application bundle was not produced: %s\n' "$app_path" >&2
  exit 1
}

printf 'Ad-hoc signing application bundle...\n'
codesign -s - --force --deep "$app_path"

EXPECTED_APP_CDHASH="$(cdhash "$app_path")"
EXPECTED_SIDECAR_CDHASH="$(cdhash "$app_path/Contents/MacOS/cadence-mcp")"
export EXPECTED_APP_CDHASH EXPECTED_SIDECAR_CDHASH
mkdir -p "$bundle_dir"
printf 'EXPECTED_APP_CDHASH=%s\nEXPECTED_SIDECAR_CDHASH=%s\n' \
  "$EXPECTED_APP_CDHASH" \
  "$EXPECTED_SIDECAR_CDHASH" >"$hash_file"

printf 'Building host cadence-mcp-smoke client...\n'
cargo build --release -p cadence-mcp --bin cadence-mcp-smoke
[[ -x "$smoke_path" ]] || {
  printf 'Expected smoke client was not produced: %s\n' "$smoke_path" >&2
  exit 1
}

printf 'Creating DMG directly from the signed application...\n'
dmg_staging="$release_tmp/dmg-staging"
mkdir -p "$dmg_staging" "$(dirname "$dmg_path")"
cp -R "$app_path" "$dmg_staging/Cadence.app"
ln -s /Applications "$dmg_staging/Applications"
hdiutil create \
  -volname Cadence \
  -srcfolder "$dmg_staging" \
  -ov \
  -format UDZO \
  "$dmg_path"

cat <<EOF

Unsigned arm64 release artifacts created.

Application: $app_path
DMG:         $dmg_path
CDHashes:    $hash_file

Run scripts/verify-release.sh --unsigned before distribution.
EOF
