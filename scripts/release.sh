#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="aarch64-apple-darwin"
bundle_dir="$root_dir/src-tauri/target/$target/release/bundle"
app_path="$bundle_dir/macos/Cadence.app"
version="$(
  node -e '
    const fs = require("node:fs");
    const config = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(config.version);
  ' "$root_dir/src-tauri/tauri.conf.json"
)"
dmg_path="$bundle_dir/dmg/Cadence_${version}_aarch64.dmg"

credential_names=(
  APPLE_CERTIFICATE
  APPLE_CERTIFICATE_PASSWORD
  APPLE_SIGNING_IDENTITY
  APPLE_ID
  APPLE_PASSWORD
  APPLE_TEAM_ID
)

any_apple_credentials=false
for name in "${credential_names[@]}"; do
  if [[ -n "${!name:-}" ]]; then
    any_apple_credentials=true
    break
  fi
done

has_signing_source=false
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  has_signing_source=true
elif [[ -n "${APPLE_CERTIFICATE:-}" && -n "${APPLE_CERTIFICATE_PASSWORD:-}" ]]; then
  has_signing_source=true
fi

has_notary_credentials=false
if [[
  -n "${APPLE_ID:-}" &&
  -n "${APPLE_PASSWORD:-}" &&
  -n "${APPLE_TEAM_ID:-}"
]]; then
  has_notary_credentials=true
fi

signed_release=false
if [[ "$any_apple_credentials" == true ]]; then
  if [[ "$has_signing_source" != true || "$has_notary_credentials" != true ]]; then
    cat >&2 <<'EOF'
ERROR: Apple release credentials are incomplete.

Provide either:
  APPLE_CERTIFICATE and APPLE_CERTIFICATE_PASSWORD
or:
  APPLE_SIGNING_IDENTITY

Also provide all notarization credentials:
  APPLE_ID, APPLE_PASSWORD, and APPLE_TEAM_ID

Refusing to fall back to an unsigned build while partial credentials are set.
EOF
    exit 1
  fi
  signed_release=true
fi

cd "$root_dir"
"$root_dir/scripts/check-versions.sh"

if [[ "$signed_release" == true ]]; then
  printf 'Building signed and notarized arm64 release with Tauri...\n'
  npm run tauri -- build --target "$target" --ci
else
  cat >&2 <<'EOF'

======================================================================
WARNING: APPLE SIGNING AND NOTARIZATION CREDENTIALS ARE ABSENT.
THIS BUILD IS UNSIGNED AND RELEASE ARTIFACTS MUST NOT SHIP.
======================================================================

EOF
  npm run tauri -- build --target "$target" --ci --no-sign
fi

[[ -d "$app_path" ]] || {
  printf 'Expected application bundle was not produced: %s\n' "$app_path" >&2
  exit 1
}
[[ -f "$dmg_path" ]] || {
  printf 'Expected DMG was not produced: %s\n' "$dmg_path" >&2
  exit 1
}

if [[ "$signed_release" == true ]]; then
  printf 'Submitting DMG for explicit notarization...\n'
  xcrun notarytool submit "$dmg_path" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait
  xcrun stapler staple "$dmg_path"
  "$root_dir/scripts/verify-release.sh" "$app_path" "$dmg_path"
else
  cat >&2 <<EOF

======================================================================
UNSIGNED BUILD COMPLETE. DO NOT UPLOAD OR DISTRIBUTE THESE ARTIFACTS.
Signed-artifact verification was skipped because it must fail unsigned.

Application: $app_path
DMG:         $dmg_path
======================================================================

EOF
fi
