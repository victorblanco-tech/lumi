#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
duration_seconds="${LUMI_AUTOLOOP_SOAK_SECONDS:-}"

if [[ -z "$duration_seconds" ]] || ! [[ "$duration_seconds" =~ ^[0-9]+$ ]]; then
  echo "ERROR: set LUMI_AUTOLOOP_SOAK_SECONDS to an integer duration." >&2
  exit 1
fi
if (( duration_seconds < 3600 )); then
  echo "ERROR: RC AutoLoop evidence requires at least 3600 seconds." >&2
  exit 1
fi

cd "$repository_root"
LUMI_AUTOLOOP_SOAK_SECONDS="$duration_seconds" \
  cargo test --locked --release -p lumi-midi-output --test realtime_soak \
    realtime_lane_configurable_soak_retains_correctness_and_latency \
    -- --ignored --exact --nocapture

echo "One-hour realtime AutoLoop RC soak passed."
