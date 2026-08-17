#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
duration_seconds="${LUMI_LIVE_SOAK_SECONDS:-}"
prolink_java="${LUMI_PROLINK_JAVA:-$repository_root/build/package-toolchains/temurin-21-macos-aarch64/Contents/Home/bin/java}"
prolink_bridge_jar="${LUMI_PROLINK_BRIDGE_JAR:-$repository_root/bridges/prolink/target/lumi-prolink-bridge.jar}"

for required_variable in \
  LUMI_SIM_URL \
  LUMI_SIM_TOKEN \
  LUMI_PROLINK_NETWORK_DATABASE \
  LUMI_CARABINER_TEST_EXECUTABLE; do
  if [[ -z "${!required_variable:-}" ]]; then
    echo "ERROR: $required_variable is required for the Live integration soak." >&2
    exit 1
  fi
done
if [[ ! -f "$LUMI_PROLINK_NETWORK_DATABASE" ]]; then
  echo "ERROR: LUMI_PROLINK_NETWORK_DATABASE must be a disposable synced database file." >&2
  exit 1
fi
if [[ ! -x "$LUMI_CARABINER_TEST_EXECUTABLE" ]]; then
  echo "ERROR: LUMI_CARABINER_TEST_EXECUTABLE must be executable." >&2
  exit 1
fi
if [[ ! -x "$prolink_java" ]] || [[ ! -f "$prolink_bridge_jar" ]]; then
  echo "ERROR: the packaged Pro DJ Link Java runtime and bridge JAR are required." >&2
  exit 1
fi
if [[ -z "$duration_seconds" ]] || ! [[ "$duration_seconds" =~ ^[0-9]+$ ]]; then
  echo "ERROR: LUMI_LIVE_SOAK_SECONDS must be an integer duration." >&2
  exit 1
fi
minimum_seconds=30
if [[ "${LUMI_REQUIRE_RC_DURATION:-0}" == "1" ]]; then
  minimum_seconds=3600
fi
if (( duration_seconds < minimum_seconds )); then
  echo "ERROR: this soak requires at least $minimum_seconds seconds per lane." >&2
  exit 1
fi

cd "$repository_root"
mkdir -p build/Evidence
evidence_path="${LUMI_LIVE_EVIDENCE_PATH:-$repository_root/build/Evidence/live-integration-${duration_seconds}s.json}"
export LUMI_PROLINK_JAVA="$prolink_java"
export LUMI_PROLINK_BRIDGE_JAR="$prolink_bridge_jar"

LUMI_RUN_PROLINK_ONLY_SOAK=1 \
LUMI_PROLINK_SOAK_SECONDS="$duration_seconds" \
  cargo test --locked --release -p lumi-engine --test prolink_network_acceptance \
    prolink_only_configurable_soak_has_bounded_ingress_without_output_side_effects \
    -- --ignored --exact --nocapture

LUMI_LINK_SOAK_SECONDS="$duration_seconds" \
  cargo test --locked --release -p lumi-timing-output --test carabiner_runtime \
    link_only_configurable_soak_keeps_one_bounded_latest_clock \
    -- --ignored --exact --nocapture

LUMI_AUTOLOOP_SOAK_SECONDS="$duration_seconds" \
  cargo test --locked --release -p lumi-midi-output --test realtime_soak \
    realtime_lane_configurable_soak_retains_correctness_and_latency \
    -- --ignored --exact --nocapture

LUMI_RUN_LIVE_INTEGRATION_SOAK=1 \
LUMI_LIVE_EVIDENCE_PATH="$evidence_path" \
LUMI_CARABINER_EXECUTABLE="$LUMI_CARABINER_TEST_EXECUTABLE" \
  cargo test --locked --release -p lumi-engine --test prolink_network_acceptance \
    combined_lanes_remain_bounded_and_emit_release_evidence \
    -- --ignored --exact --nocapture

echo "Live integration soak passed. Evidence: $evidence_path"
