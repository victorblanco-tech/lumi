#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
classifier="$script_dir/classify-ci-changes.sh"

assert_classification() {
  local expected="$1"
  shift
  local actual
  actual="$($classifier "$@")"
  if [[ "$actual" != "$expected" ]]; then
    echo "ERROR: classification differed for: $*" >&2
    echo "Expected:" >&2
    printf '%s\n' "$expected" >&2
    echo "Actual:" >&2
    printf '%s\n' "$actual" >&2
    exit 1
  fi
}

docs_only=$'docs=1\nrust=0\napple=0\nfull=0'
rust_and_apple=$'docs=0\nrust=1\napple=1\nfull=0'
apple_only=$'docs=0\nrust=0\napple=1\nfull=0'
safe_fallback=$'docs=0\nrust=1\napple=1\nfull=1'
mixed_docs_apple=$'docs=1\nrust=0\napple=1\nfull=0'
all_checks=$'docs=1\nrust=1\napple=1\nfull=1'
no_file_changes=$'docs=0\nrust=0\napple=0\nfull=0'

assert_classification "$docs_only" docs/assets/brand/lumi-github-header.svg
assert_classification "$docs_only" README.md
assert_classification "$rust_and_apple" engine/crates/lumi-engine/src/main.rs
assert_classification "$apple_only" apps/macos/Lumi/App/LumiApp.swift
assert_classification "$mixed_docs_apple" README.md apps/macos/Lumi/App/LumiApp.swift
assert_classification "$safe_fallback" .github/workflows/foundation.yml
assert_classification "$safe_fallback" future-platform/new-source.txt
assert_classification "$all_checks" --all
assert_classification "$no_file_changes" --from-git HEAD HEAD

echo "CI change classification tests passed."
