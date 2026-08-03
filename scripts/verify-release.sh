#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s --unsigned\n' "$0" >&2
  exit 2
}

[[ $# -eq 1 && "$1" == "--unsigned" ]] || usage

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="aarch64-apple-darwin"
bundle_dir="$root_dir/target/$target/release/bundle"
app_path="$bundle_dir/macos/Cadence.app"
main_path="$app_path/Contents/MacOS/cadence"
sidecar_path="$app_path/Contents/MacOS/cadence-mcp"
smoke_path="$root_dir/target/release/cadence-mcp-smoke"
hash_file="$bundle_dir/release-cdhashes.env"
version="$(
  node -e '
    const fs = require("node:fs");
    const config = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(config.version);
  ' "$root_dir/src-tauri/tauri.conf.json"
)"
dmg_path="$bundle_dir/dmg/Cadence_${version}_aarch64.dmg"

for command in codesign lipo hdiutil xattr uuidgen; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'Required command not found: %s\n' "$command" >&2
    exit 1
  }
done

[[ -d "$app_path" ]] || {
  printf 'Application bundle not found at root target path: %s\n' "$app_path" >&2
  exit 1
}
[[ -x "$main_path" ]] || {
  printf 'Application executable not found: %s\n' "$main_path" >&2
  exit 1
}
[[ -x "$sidecar_path" ]] || {
  printf 'MCP sidecar not found: %s\n' "$sidecar_path" >&2
  exit 1
}
[[ -x "$smoke_path" ]] || {
  printf 'Smoke client not found at host target path: %s\n' "$smoke_path" >&2
  exit 1
}
[[ -f "$dmg_path" ]] || {
  printf 'DMG not found at root target path: %s\n' "$dmg_path" >&2
  exit 1
}
[[ -f "$hash_file" ]] || {
  printf 'Release CDHash capture not found: %s\n' "$hash_file" >&2
  exit 1
}

EXPECTED_APP_CDHASH="$(sed -n 's/^EXPECTED_APP_CDHASH=//p' "$hash_file")"
EXPECTED_SIDECAR_CDHASH="$(
  sed -n 's/^EXPECTED_SIDECAR_CDHASH=//p' "$hash_file"
)"
[[ "$EXPECTED_APP_CDHASH" =~ ^[[:xdigit:]]+$ ]] || {
  printf 'Invalid expected app CDHash in %s\n' "$hash_file" >&2
  exit 1
}
[[ "$EXPECTED_SIDECAR_CDHASH" =~ ^[[:xdigit:]]+$ ]] || {
  printf 'Invalid expected sidecar CDHash in %s\n' "$hash_file" >&2
  exit 1
}

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

assert_hash() {
  local label="$1"
  local expected="$2"
  local candidate="$3"
  local actual
  actual="$(cdhash "$candidate")"
  [[ "$actual" == "$expected" ]] || {
    printf '%s CDHash mismatch: expected %s, got %s\n' \
      "$label" "$expected" "$actual" >&2
    return 1
  }
}

assert_arm64() {
  local label="$1"
  local candidate="$2"
  local architectures
  architectures="$(lipo -archs "$candidate")"
  [[ "$architectures" == "arm64" ]] || {
    printf '%s architectures are %s; expected exactly arm64\n' \
      "$label" "$architectures" >&2
    return 1
  }
}

assert_bundle_executables() {
  local candidate_app="$1"
  local executable_dir="$candidate_app/Contents/MacOS"
  local actual
  actual="$(
    find "$executable_dir" -mindepth 1 -maxdepth 1 -exec basename {} \; |
      LC_ALL=C sort
  )"
  local expected
  expected=$'cadence\ncadence-mcp'
  [[ "$actual" == "$expected" ]] || {
    printf 'Unexpected Contents/MacOS entries in %s:\n%s\n' \
      "$candidate_app" "$actual" >&2
    return 1
  }
}

printf 'Verifying loose application...\n'
assert_arm64 "Application" "$main_path"
assert_arm64 "MCP sidecar" "$sidecar_path"
codesign --verify --strict "$main_path"
codesign --verify --strict "$sidecar_path"
codesign --verify --deep --strict "$app_path"
assert_bundle_executables "$app_path"
assert_hash "Loose application" "$EXPECTED_APP_CDHASH" "$app_path"
assert_hash "Loose sidecar" "$EXPECTED_SIDECAR_CDHASH" "$sidecar_path"

verify_tmp="$(mktemp -d)"
mount_point="$verify_tmp/mount"
install_dir="$verify_tmp/install"
mounted=false

cleanup() {
  if [[ "$mounted" == true ]]; then
    hdiutil detach "$mount_point" >/dev/null 2>&1 || true
  fi
  rm -rf "$verify_tmp"
}
trap cleanup EXIT

mkdir -p "$mount_point" "$install_dir"
printf 'Mounting DMG...\n'
hdiutil attach -nobrowse -mountpoint "$mount_point" "$dmg_path" >/dev/null
mounted=true

mounted_app="$mount_point/Cadence.app"
mounted_sidecar="$mounted_app/Contents/MacOS/cadence-mcp"
[[ -d "$mounted_app" ]] || {
  printf 'Mounted DMG does not contain Cadence.app\n' >&2
  exit 1
}
assert_bundle_executables "$mounted_app"
codesign --verify --strict "$mounted_app/Contents/MacOS/cadence"
codesign --verify --strict "$mounted_sidecar"
assert_hash "DMG application" "$EXPECTED_APP_CDHASH" "$mounted_app"
assert_hash "DMG sidecar" "$EXPECTED_SIDECAR_CDHASH" "$mounted_sidecar"

printf 'Copying mounted application and exercising quarantine removal...\n'
cp -R "$mounted_app" "$install_dir/Cadence.app"
installed_app="$install_dir/Cadence.app"
codesign --verify --deep --strict "$installed_app"

quarantine_value="0083;$(printf '%x' "$(date +%s)");verify;$(uuidgen)"
xattr -w com.apple.quarantine "$quarantine_value" "$installed_app"
xattr -p com.apple.quarantine "$installed_app" >/dev/null
xattr -dr com.apple.quarantine "$installed_app"
if xattr -p com.apple.quarantine "$installed_app" >/dev/null 2>&1; then
  printf 'Quarantine attribute remained after recursive removal\n' >&2
  exit 1
fi

fixture_db="$verify_tmp/fixture.db"
[[ ! -e "$fixture_db" ]] || {
  printf 'Smoke fixture unexpectedly exists: %s\n' "$fixture_db" >&2
  exit 1
}
printf 'Running installed sidecar smoke test...\n'
env -u CADENCE_MCP_ALLOW_WRITES -u CADENCE_MCP_FAULT \
  "$smoke_path" \
  "$installed_app/Contents/MacOS/cadence-mcp" \
  "$fixture_db"

hdiutil detach "$mount_point" >/dev/null
mounted=false

cat <<EOF

Unsigned release verification passed.

- app and sidecar are arm64
- loose and DMG CDHashes match the post-signing captures
- both bundles contain exactly cadence and cadence-mcp
- copied app passes strict deep signature verification
- quarantine removal and MCP smoke handshake succeeded
EOF
