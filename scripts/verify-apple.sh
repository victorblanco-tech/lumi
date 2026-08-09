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
"$script_dir/check-architecture.sh"

cd "$repository_root"

# The native process integration and app bundle both require the real local
# engine binary. Portable Rust lint, tests, migrations, and release benchmarks
# intentionally remain in verify-rust.sh so they can run on a Linux runner.
cargo build --locked -p lumi-engine

swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiProtocol
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiDesignSystem
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiLiveWorkspace
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiLibraryWorkspace
LUMI_ENGINE_TEST_EXECUTABLE="$repository_root/target/debug/lumi-engine" \
  swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiEngineClient

xcodebuild \
  -project apps/macos/Lumi.xcodeproj \
  -scheme Lumi \
  -configuration Debug \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath build/DerivedData \
  CODE_SIGNING_ALLOWED=NO \
  GCC_TREAT_WARNINGS_AS_ERRORS=YES \
  -quiet \
  build

built_app="build/DerivedData/Build/Products/Debug/Lumi Dev.app"
built_info_plist="$built_app/Contents/Info.plist"
built_engine_helper="$built_app/Contents/Helpers/lumi-engine"
built_app_icon="$built_app/Contents/Resources/AppIcon.icns"
built_asset_catalog="$built_app/Contents/Resources/Assets.car"
built_product_version="$(/usr/libexec/PlistBuddy -c 'Print :LumiProductVersion' "$built_info_plist")"
built_bundle_identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$built_info_plist")"
built_channel="$(/usr/libexec/PlistBuddy -c 'Print :LumiReleaseChannel' "$built_info_plist")"
built_data_directory="$(/usr/libexec/PlistBuddy -c 'Print :LumiDataDirectoryName' "$built_info_plist")"
canonical_version="$(tr -d '[:space:]' < VERSION)"

if [[ "$built_product_version" != "$canonical_version" ]]; then
  echo "ERROR: built app version '$built_product_version' differs from VERSION '$canonical_version'." >&2
  exit 1
fi

if [[ "$built_bundle_identifier" != "co.victorblan.tech.lumi.dev" ]] \
  || [[ "$built_channel" != "dev" ]] \
  || [[ "$built_data_directory" != "Lumi Dev" ]]; then
  echo "ERROR: Debug build does not have the isolated Dev identity." >&2
  exit 1
fi

if [[ ! -x "$built_engine_helper" ]]; then
  echo "ERROR: the built app does not contain an executable Lumi engine helper." >&2
  exit 1
fi

if [[ ! -s "$built_app_icon" ]] || [[ ! -s "$built_asset_catalog" ]]; then
  echo "ERROR: the built Lumi app is missing its compiled app icon or in-app brand assets." >&2
  exit 1
fi

if ! file "$built_engine_helper" | grep -q 'arm64'; then
  echo "ERROR: the built Lumi engine helper is not Apple Silicon arm64." >&2
  exit 1
fi

"$script_dir/render-visual-evidence.sh" "$repository_root/build/VisualEvidence"

visual_evidence_count="$(find "$repository_root/build/VisualEvidence" -type f -name '*.png' | wc -l | tr -d '[:space:]')"
if [[ "$visual_evidence_count" != "22" ]]; then
  echo "ERROR: expected 22 visual evidence PNGs, found $visual_evidence_count." >&2
  exit 1
fi

"$script_dir/check-epic-2a-evidence.sh" "$repository_root/build/VisualEvidence"

echo "Apple application verification passed."
