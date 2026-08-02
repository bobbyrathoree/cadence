#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

read_json_version() {
  node -e '
    const fs = require("node:fs");
    const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8")).version;
    if (typeof value !== "string" || value.length === 0) process.exit(1);
    process.stdout.write(value);
  ' "$1"
}

tauri_version="$(read_json_version "$root_dir/src-tauri/tauri.conf.json")"
package_version="$(read_json_version "$root_dir/package.json")"
cargo_version="$(
  cargo metadata \
    --format-version 1 \
    --no-deps \
    --manifest-path "$root_dir/src-tauri/Cargo.toml" |
    node -e '
      let input = "";
      process.stdin.setEncoding("utf8");
      process.stdin.on("data", chunk => input += chunk);
      process.stdin.on("end", () => {
        const metadata = JSON.parse(input);
        const cadence = metadata.packages.find(pkg => pkg.name === "cadence");
        if (!cadence) process.exit(1);
        process.stdout.write(cadence.version);
      });
    '
)"

if [[ "$cargo_version" != "$tauri_version" || "$package_version" != "$tauri_version" ]]; then
  printf 'Version mismatch: tauri.conf.json=%s Cargo.toml=%s package.json=%s\n' \
    "$tauri_version" "$cargo_version" "$package_version" >&2
  exit 1
fi

printf 'Versions aligned: %s\n' "$tauri_version"
