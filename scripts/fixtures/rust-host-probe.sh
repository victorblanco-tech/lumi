#!/usr/bin/env bash
set -uo pipefail

# Diagnostics only: never starts Lumi, talks to devices or changes OS settings.
probe() {
  local label="$1"
  shift
  "$@" cargo build --locked --release -p thiserror \
    --target-dir "build/rust-host-probe-$label" \
    >"build/rust-host-probe-$label.log" 2>&1
  local status=$?
  echo "PROBE $label: exit $status"
  if [[ "$status" != 0 ]]; then
    grep -E 'WARN|ERROR|error|dylib|dlopen|failed' "build/rust-host-probe-$label.log" | tail -35
  fi
}
mkdir -p build
probe inherited env RUSTC_LOG=rustc_metadata=info
probe no-sdk env -u SDKROOT -u MACOSX_DEPLOYMENT_TARGET RUSTC_LOG=rustc_metadata=info
probe clean env -i "HOME=$HOME" "PATH=$PATH" RUSTC_LOG=rustc_metadata=info
probe serial env CARGO_BUILD_JOBS=1 RUSTC_LOG=rustc_metadata=info
