#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"
packaging_java_home="${LUMI_PACKAGING_JAVA_HOME:-${JAVA_HOME:-$repository_root/build/package-toolchains/temurin-21-macos-aarch64/Contents/Home}}"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

"$script_dir/check-environment.sh"
"$script_dir/check-version.sh"
"$script_dir/check-structure.sh"
"$script_dir/check-architecture.sh"
"$script_dir/test-product-versioning.sh"

cd "$repository_root"

"$script_dir/check-apple-test-exclusivity.sh"

# The native process integration and app bundle both require the real local
# engine binary. Portable Rust lint, tests, migrations, and release benchmarks
# intentionally remain in verify-rust.sh so they can run on a Linux runner.
cargo build --locked --release -p lumi-engine

swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiProtocol
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiDesignSystem
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiLiveWorkspace
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiLibraryWorkspace
swift test -Xswiftc -warnings-as-errors --package-path apps/ios/Packages/LumiRemoteClient
swift test -Xswiftc -warnings-as-errors --package-path apps/ios/Packages/LumiRemoteFeature

xcodebuild \
  -project apps/ios/LumiRemote.xcodeproj \
  -scheme LumiRemote \
  -configuration Dev \
  -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath build/iOSDerivedData \
  CODE_SIGNING_ALLOWED=NO \
  GCC_TREAT_WARNINGS_AS_ERRORS=YES \
  -quiet \
  build

remote_info_plist="build/iOSDerivedData/Build/Products/Dev-iphonesimulator/Lumi Remote Dev.app/Info.plist"
[[ -f "$remote_info_plist" ]] || { echo "ERROR: Lumi Remote iOS app was not built." >&2; exit 1; }
[[ "$(/usr/libexec/PlistBuddy -c 'Print :LumiProductVersion' "$remote_info_plist")" == "$(tr -d '[:space:]' < apps/ios/VERSION)" ]] || {
  echo "ERROR: built Lumi Remote version differs from apps/ios/VERSION." >&2
  exit 1
}

LUMI_ENGINE_TEST_EXECUTABLE="$repository_root/target/release/lumi-engine" \
LUMI_PROLINK_JAVA="$packaging_java_home/bin/java" \
  LUMI_PROLINK_BRIDGE_JAR="$repository_root/bridges/prolink/target/lumi-prolink-bridge.jar" \
  LUMI_CARABINER_EXECUTABLE="$repository_root/build/carabiner-runtime/Carabiner" \
  swift test --no-parallel -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiEngineClient

xcodebuild \
  -project apps/macos/Lumi.xcodeproj \
  -scheme Lumi \
  -configuration Dev \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath build/DerivedData \
  LUMI_PACKAGING_JAVA_HOME="$packaging_java_home" \
  CODE_SIGNING_ALLOWED=NO \
  GCC_TREAT_WARNINGS_AS_ERRORS=YES \
  -quiet \
  build

built_app="build/DerivedData/Build/Products/Dev/Lumi.app"
built_info_plist="$built_app/Contents/Info.plist"
built_engine_helper="$built_app/Contents/Helpers/lumi-engine"
built_launch_agent="$built_app/Contents/Library/LaunchAgents/co.victorblan.tech.lumi.dev.engine.plist"
built_link_helper="$built_app/Contents/Resources/link/Carabiner"
built_app_icon="$built_app/Contents/Resources/AppIcon.icns"
built_asset_catalog="$built_app/Contents/Resources/Assets.car"
built_product_version="$(/usr/libexec/PlistBuddy -c 'Print :LumiProductVersion' "$built_info_plist")"
built_bundle_identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$built_info_plist")"
built_display_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleDisplayName' "$built_info_plist")"
built_channel="$(/usr/libexec/PlistBuddy -c 'Print :LumiReleaseChannel' "$built_info_plist")"
built_data_directory="$(/usr/libexec/PlistBuddy -c 'Print :LumiDataDirectoryName' "$built_info_plist")"
built_demo_seed="$(/usr/libexec/PlistBuddy -c 'Print :LumiSeedDemoLibrary' "$built_info_plist")"
canonical_version="$(tr -d '[:space:]' < VERSION)"

if [[ "$built_product_version" != "$canonical_version" ]]; then
  echo "ERROR: built app version '$built_product_version' differs from VERSION '$canonical_version'." >&2
  exit 1
fi

if [[ "$built_bundle_identifier" != "co.victorblan.tech.lumi.dev" ]] \
  || [[ "$built_display_name" != "Lumi Dev" ]] \
  || [[ "$built_channel" != "dev" ]] \
  || [[ "$built_data_directory" != "Lumi Dev" ]] \
  || [[ "$built_demo_seed" != "1" ]]; then
  echo "ERROR: Dev build does not have the isolated identity." >&2
  exit 1
fi

if [[ ! -x "$built_engine_helper" ]]; then
  echo "ERROR: the built app does not contain an executable Lumi engine helper." >&2
  exit 1
fi

if [[ -z "$(/usr/libexec/PlistBuddy -c 'Print :NSRemovableVolumesUsageDescription' "$built_info_plist" 2>/dev/null)" ]]; then
  echo "ERROR: the built app does not explain its read-only USB access." >&2
  exit 1
fi
if ! otool -v -s __TEXT __info_plist "$built_engine_helper" \
  | grep -q '<key>NSRemovableVolumesUsageDescription</key>'; then
  echo "ERROR: the built Lumi engine helper does not explain its read-only USB access." >&2
  exit 1
fi
if [[ ! -f "$built_launch_agent" ]]; then
  echo "ERROR: the built app does not contain the SMAppService LaunchAgent." >&2
  exit 1
fi
if ! otool -s __TEXT __info_plist "$built_engine_helper" >/dev/null 2>&1; then
  echo "ERROR: the built Lumi engine helper has no embedded Info.plist." >&2
  exit 1
fi

if [[ ! -x "$built_link_helper" ]]; then
  echo "ERROR: the built app does not contain the managed Ableton Link helper." >&2
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


if ! file "$built_link_helper" | grep -q 'arm64'; then
  echo "ERROR: the managed Ableton Link helper is not Apple Silicon compatible." >&2
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
