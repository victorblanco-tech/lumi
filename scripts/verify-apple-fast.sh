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

cd "$repository_root"

swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiProtocol
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiEngineClient
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiDesignSystem
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiLiveWorkspace
swift test -Xswiftc -warnings-as-errors --package-path apps/macos/Packages/LumiLibraryWorkspace

echo "Fast Apple development verification passed."
