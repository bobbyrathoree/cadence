#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$root_dir/crates/core/tests/baseline-manifest.txt"

usage() {
  printf 'Usage: %s --exact|--subset|--self-test\n' "${0##*/}" >&2
  exit 2
}

check_duplicates() {
  local label="$1"
  local input="$2"
  local duplicates

  duplicates="$(LC_ALL=C sort "$input" | uniq -d)"
  if [[ -n "$duplicates" ]]; then
    printf '%s contains duplicate test names:\n%s\n' "$label" "$duplicates" >&2
    return 1
  fi
}

compare_names() {
  local mode="$1"
  local expected="$2"
  local actual="$3"
  local temp_dir="$4"
  local expected_sorted="$temp_dir/expected.sorted"
  local actual_sorted="$temp_dir/actual.sorted"
  local missing="$temp_dir/missing"
  local added="$temp_dir/added"

  check_duplicates "baseline manifest" "$expected" || return 1
  check_duplicates "current test list" "$actual" || return 1

  LC_ALL=C sort "$expected" > "$expected_sorted"
  LC_ALL=C sort "$actual" > "$actual_sorted"
  comm -23 "$expected_sorted" "$actual_sorted" > "$missing"
  comm -13 "$expected_sorted" "$actual_sorted" > "$added"

  if [[ -s "$missing" ]]; then
    printf 'Baseline tests missing or renamed:\n' >&2
    sed 's/^/  /' "$missing" >&2
    return 1
  fi

  if [[ "$mode" == "--exact" && -s "$added" ]]; then
    printf 'Unexpected tests in exact mode:\n' >&2
    sed 's/^/  /' "$added" >&2
    return 1
  fi
}

run_self_test() {
  local fixtures="$root_dir/scripts/fixtures/baseline"
  local temp_dir
  temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/cadence-baseline-self-test.XXXXXX")"
  trap 'rm -rf "$temp_dir"' RETURN

  compare_names --exact "$fixtures/manifest.txt" "$fixtures/exact.txt" "$temp_dir"
  ! compare_names --exact "$fixtures/manifest.txt" "$fixtures/missing.txt" "$temp_dir"
  ! compare_names --subset "$fixtures/manifest.txt" "$fixtures/renamed.txt" "$temp_dir"
  ! compare_names --exact "$fixtures/manifest.txt" "$fixtures/duplicate.txt" "$temp_dir"
  ! compare_names --exact "$fixtures/manifest.txt" "$fixtures/added.txt" "$temp_dir"
  compare_names --subset "$fixtures/manifest.txt" "$fixtures/added.txt" "$temp_dir"

  printf 'Baseline checker self-test passed.\n'
}

[[ $# -eq 1 ]] || usage
mode="$1"

if [[ "$mode" == "--self-test" ]]; then
  run_self_test
  exit 0
fi

[[ "$mode" == "--exact" || "$mode" == "--subset" ]] || usage

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/cadence-baseline.XXXXXX")"
trap 'rm -rf "$temp_dir"' EXIT
actual="$temp_dir/actual.txt"

cargo test --workspace -- --list 2>&1 |
  awk '/: test$/ { sub(/: test$/, ""); print }' > "$actual"

compare_names "$mode" "$manifest" "$actual" "$temp_dir"
count="$(wc -l < "$actual" | tr -d ' ')"
printf 'Baseline check passed (%s, %s current tests).\n' "${mode#--}" "$count"
