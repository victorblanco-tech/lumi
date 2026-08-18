#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

for required_variable in \
  LUMI_SIM_URL \
  LUMI_SIM_TOKEN \
  LUMI_PROLINK_NETWORK_DATABASE; do
  if [[ -z "${!required_variable:-}" ]]; then
    echo "ERROR: $required_variable is required for the show/lab gate." >&2
    echo "See docs/development/local-quality-gates.md for setup." >&2
    exit 1
  fi
done
if [[ ! -f "$LUMI_PROLINK_NETWORK_DATABASE" ]]; then
  echo "ERROR: LUMI_PROLINK_NETWORK_DATABASE must be a disposable synced database file." >&2
  exit 1
fi

cd "$repository_root"

LUMI_RUN_PROLINK_NETWORK_TEST=1 \
  cargo test --locked -p lumi-engine --test prolink_network_acceptance \
    direct_timing_continues_while_the_client_is_idle \
    -- --ignored --exact --nocapture

LUMI_RUN_PROLINK_OUTPUT_TEST=1 \
  cargo test --locked -p lumi-engine --test prolink_network_acceptance \
    stopped_live_deck_start_and_operation_resume_restore_the_current_autoloop \
    -- --ignored --exact --nocapture

if [[ -n "${LUMI_CARABINER_TEST_EXECUTABLE:-}" ]]; then
  if [[ ! -x "$LUMI_CARABINER_TEST_EXECUTABLE" ]]; then
    echo "ERROR: LUMI_CARABINER_TEST_EXECUTABLE is not executable." >&2
    exit 1
  fi
  cargo test --locked -p lumi-timing-output --test carabiner_runtime \
    launches_real_helper_and_publishes_a_link_timeline \
    -- --ignored --exact --nocapture
else
  echo "NOTE: managed Link helper check skipped; set LUMI_CARABINER_TEST_EXECUTABLE to include it."
fi

echo "Bounded show/lab acceptance gate passed. One-hour and physical DMX evidence remain separate RC gates."
