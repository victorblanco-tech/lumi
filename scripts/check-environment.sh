#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

fail() {
  echo "ERROR: $1" >&2
  exit 1
}

resolve_rust_path() {
  if command -v rustc >/dev/null 2>&1; then
    return
  fi

  cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"
  if [[ -x "$cargo_bin_directory/rustc" ]]; then
    export PATH="$cargo_bin_directory:$PATH"
    return
  fi

  fail "Rust was not found. Install it with rustup from https://rustup.rs."
}

[[ "$(uname -m)" == "arm64" ]] || fail "Epic 1 requires an Apple Silicon arm64 Mac."
[[ -f "$repository_root/rust-toolchain.toml" ]] || fail "rust-toolchain.toml is missing."

command -v xcodebuild >/dev/null 2>&1 || fail "Full Xcode is not installed."
command -v xcrun >/dev/null 2>&1 || fail "xcrun is not available."
resolve_rust_path

xcodebuild -checkFirstLaunchStatus

echo "Environment"
echo "  Architecture: $(uname -m)"
echo "  $(xcodebuild -version | tr '\n' ' ')"
echo "  $(swift --version | sed -n '1p')"
echo "  macOS SDK: $(xcrun --sdk macosx --show-sdk-version)"
echo "  iOS SDK: $(xcrun --sdk iphoneos --show-sdk-version)"
echo "  $(rustc --version)"
echo "  $(cargo --version)"

rustup component list --installed | grep -q '^rustfmt-' || fail "rustfmt is not installed."
rustup component list --installed | grep -q '^clippy-' || fail "Clippy is not installed."

if xcrun simctl list runtimes 2>/dev/null | grep -q '^iOS '; then
  echo "  iOS Simulator: available"
else
  echo "  iOS Simulator: not available (not required for the macOS foundation build)"
fi

if [[ "${LUMI_CHECK_GITHUB_AUTH:-0}" == "1" ]]; then
  command -v gh >/dev/null 2>&1 || fail "GitHub CLI is not installed."
  gh auth status >/dev/null
  echo "  GitHub CLI: authenticated"
fi

echo "Environment check passed."
