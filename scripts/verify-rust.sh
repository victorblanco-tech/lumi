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

python3 -m json.tool apps/macos/Lumi/Resources/Localizable.xcstrings >/dev/null
python3 -m json.tool \
  apps/macos/Packages/LumiLiveWorkspace/Localization/Localizable.xcstrings \
  >/dev/null
python3 -m json.tool contracts/protocol/v1/manifest.json >/dev/null
python3 -m json.tool contracts/protocol/v1/envelope.schema.json >/dev/null
for protocol_fixture in contracts/protocol/v1/fixtures/*.json; do
  python3 -m json.tool "$protocol_fixture" >/dev/null
done

cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo test --locked --release -p lumi-planner \
  two_hundred_phrase_plan_completes_within_epic_one_budget
cargo test --locked --release -p lumi-library-sqlite --test repository \
  ten_thousand_track_fixture_meets_epic_two_a_budgets -- --exact --ignored --nocapture
cargo build --locked --workspace --all-features

echo "Portable Rust verification passed."
