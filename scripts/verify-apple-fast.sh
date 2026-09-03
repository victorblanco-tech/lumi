#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
module_cache="$repository_root/build/swift-module-cache"

mkdir -p "$module_cache"
export CLANG_MODULE_CACHE_PATH="$module_cache"
export SWIFTPM_MODULECACHE_OVERRIDE="$module_cache"

"$script_dir/check-environment.sh"
"$script_dir/check-version.sh"
"$script_dir/check-structure.sh"
"$script_dir/check-architecture.sh"
"$script_dir/test-product-versioning.sh"
"$script_dir/check-ios-app-icon.sh"

cd "$repository_root"

swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiProtocol
# LumiEngineClient includes real-process integration tests and therefore belongs
# to verify-apple.sh after the release engine/helper runtimes are prepared.
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
remote_app="$(dirname "$remote_info_plist")"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconName' "$remote_info_plist")" == "AppIcon" ]] || {
  echo "ERROR: built Lumi Remote app does not declare AppIcon as its primary icon." >&2
  exit 1
}
[[ -f "$remote_app/Assets.car" && -f "$remote_app/AppIcon60x60@2x.png" ]] || {
  echo "ERROR: built Lumi Remote app is missing compiled AppIcon assets." >&2
  exit 1
}

echo "Fast Apple development verification passed."
