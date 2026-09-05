#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

"$script_dir/check-version.sh"
"$script_dir/check-structure.sh"
"$script_dir/check-architecture.sh"

cd "$repository_root"

cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

# These packages own the high-risk ingress, scheduling, recovery and output
# boundaries. Ignored LAN/physical tests are deliberately reserved for lab.
cargo test --locked --all-features \
  -p lumi-prolink-input \
  -p lumi-timing-output \
  -p lumi-lighting-output \
  -p lumi-midi-output \
  -p lumi-midi-coremidi
cargo test --locked -p lumi-domain --test reducer_runtime

# Release-mode budgets catch regressions hidden by debug-build overhead.
cargo test --locked --release -p lumi-planner \
  two_hundred_phrase_plan_completes_within_epic_one_budget
cargo test --locked --release -p lumi-library-sqlite --test repository \
  ten_thousand_track_fixture_meets_epic_two_a_budgets -- --exact --ignored --nocapture
cargo test --locked --release -p lumi-prolink-input \
  supervisor::tests::fifty_thousand_status_updates_remain_constant_space_and_within_budget \
  -- --exact --nocapture
cargo test --locked --release -p lumi-engine \
  session::tests::full_snapshot_projection_has_a_measured_bounded_baseline \
  -- --exact --nocapture

"$script_dir/verify-prolink-bridge.sh"
"$script_dir/verify-prolink-simulator.sh"

echo "Local technical robustness and performance gate passed."
