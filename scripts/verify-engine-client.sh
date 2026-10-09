#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

# Use the same ownership gate for focused reruns as for full Apple verification.
# Never stop apps automatically, and never join the user's DJ network in these tests.
bash "$script_dir/check-apple-test-exclusivity.sh"
env -u LUMI_PROLINK_JAVA -u LUMI_PROLINK_BRIDGE_JAR \
  LUMI_ENGINE_TEST_EXECUTABLE="$repository_root/target/release/lumi-engine" \
  LUMI_CARABINER_EXECUTABLE="$repository_root/build/carabiner-runtime/Carabiner" \
  swift test --no-parallel -Xswiftc -warnings-as-errors \
    --package-path "$repository_root/apps/macos/Packages/LumiEngineClient" "$@"
