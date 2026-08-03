#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s /path/to/Cadence.app /path/to/Cadence_*.dmg\n' "$0" >&2
  exit 2
}

# TODO(B9): invoke this for both the loose app and the mounted-DMG copy when
# verify-release.sh gains its v1.2 --unsigned flow and cadence-mcp sidecar.
assert_v12_bundle_executables() {
  local candidate_app="$1"
  local executable_dir="$candidate_app/Contents/MacOS"
  local actual
  actual="$(
    find "$executable_dir" -mindepth 1 -maxdepth 1 -type f -perm -111 \
      -exec basename {} \; | LC_ALL=C sort
  )"
  local expected
  expected=$'cadence\ncadence-mcp'
  [[ "$actual" == "$expected" ]] || {
    printf 'Unexpected bundled executables in %s:\n%s\n' "$executable_dir" "$actual" >&2
    return 1
  }
}

[[ $# -eq 2 ]] || usage

app_path="$1"
dmg_path="$2"
binary_path="$app_path/Contents/MacOS/cadence"

[[ -d "$app_path" ]] || {
  printf 'Application bundle not found: %s\n' "$app_path" >&2
  exit 1
}
[[ -f "$dmg_path" ]] || {
  printf 'DMG not found: %s\n' "$dmg_path" >&2
  exit 1
}
[[ -f "$binary_path" ]] || {
  printf 'Application binary not found: %s\n' "$binary_path" >&2
  exit 1
}

for command in codesign spctl xcrun lipo; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'Required command not found: %s\n' "$command" >&2
    exit 1
  }
done

printf 'Verifying application signature...\n'
codesign --verify --deep --strict "$app_path"

assess() {
  local label="$1"
  shift
  local output
  if ! output="$("$@" 2>&1)"; then
    printf '%s assessment failed:\n%s\n' "$label" "$output" >&2
    return 1
  fi
  printf '%s\n' "$output"
  grep -q 'accepted' <<<"$output" || {
    printf '%s assessment did not report accepted\n' "$label" >&2
    return 1
  }
  if [[ "$label" == "Application" ]]; then
    grep -q 'source=Notarized Developer ID' <<<"$output" || {
      printf 'Application assessment did not report Notarized Developer ID\n' >&2
      return 1
    }
  fi
}

printf 'Assessing notarized application with Gatekeeper...\n'
assess "Application" spctl -a -t exec -vvv "$app_path"

printf 'Assessing DMG with Gatekeeper...\n'
assess "DMG" spctl -a -t open --context context:primary-signature -vvv "$dmg_path"

printf 'Validating stapled tickets...\n'
xcrun stapler validate "$app_path"
xcrun stapler validate "$dmg_path"

architectures="$(lipo -archs "$binary_path")"
if [[ "$architectures" != "arm64" ]]; then
  printf 'Unexpected application architectures: %s (expected exactly arm64)\n' \
    "$architectures" >&2
  exit 1
fi
printf 'Architecture verified: arm64\n'

cat <<EOF

Automated release verification passed.

Complete the quarantined-download launch check on the downloaded DMG:

  xattr -w com.apple.quarantine "0083;\$(printf %x \$(date +%s));Safari;\$(uuidgen)" "$dmg_path"
  hdiutil attach "$dmg_path"

Then copy Cadence.app from the mounted image to /Applications and launch it.
Gatekeeper must open it without Privacy & Security intervention. Finally eject
the mounted image with:

  hdiutil detach "/Volumes/Cadence"
EOF
