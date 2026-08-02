#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

"$script_dir/check-environment.sh"
"$script_dir/check-version.sh"
"$script_dir/check-structure.sh"

cd "$repository_root"

python3 -m json.tool apps/macos/Lumi/Resources/Localizable.xcstrings >/dev/null
python3 -m json.tool contracts/protocol/v1/manifest.json >/dev/null
python3 -m json.tool contracts/protocol/v1/envelope.schema.json >/dev/null
for protocol_fixture in contracts/protocol/v1/fixtures/*.json; do
  python3 -m json.tool "$protocol_fixture" >/dev/null
done
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
swift test --package-path apps/macos/Packages/LumiProtocol
LUMI_ENGINE_TEST_EXECUTABLE="$repository_root/target/debug/lumi-engine" \
  swift test --package-path apps/macos/Packages/LumiEngineClient

xcodebuild \
  -project apps/macos/Lumi.xcodeproj \
  -scheme Lumi \
  -configuration Debug \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath build/DerivedData \
  CODE_SIGNING_ALLOWED=NO \
  -quiet \
  build

built_info_plist="build/DerivedData/Build/Products/Debug/Lumi.app/Contents/Info.plist"
built_engine_helper="build/DerivedData/Build/Products/Debug/Lumi.app/Contents/Helpers/lumi-engine"
built_product_version="$(/usr/libexec/PlistBuddy -c 'Print :LumiProductVersion' "$built_info_plist")"
canonical_version="$(tr -d '[:space:]' < VERSION)"

if [[ "$built_product_version" != "$canonical_version" ]]; then
  echo "ERROR: built app version '$built_product_version' differs from VERSION '$canonical_version'." >&2
  exit 1
fi

if [[ ! -x "$built_engine_helper" ]]; then
  echo "ERROR: the built app does not contain an executable Lumi engine helper." >&2
  exit 1
fi

if ! file "$built_engine_helper" | grep -q 'arm64'; then
  echo "ERROR: the built Lumi engine helper is not Apple Silicon arm64." >&2
  exit 1
fi

echo "Repository verification passed."
