#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
output_directory="${1:-$repository_root/build/VisualEvidence}"

cd "$repository_root"
swift run \
  --package-path apps/macos/Packages/LumiLiveWorkspace \
  LumiVisualEvidence \
  --output "$output_directory"

swift run \
  --package-path apps/macos/Packages/LumiLibraryWorkspace \
  LumiLibraryVisualEvidence \
  --output "$output_directory"

echo "Visual evidence rendered to $output_directory"
