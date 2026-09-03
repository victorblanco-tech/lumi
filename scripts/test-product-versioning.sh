#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

macos="$(tr -d '[:space:]' < "$repository_root/VERSION")"
remote="$(tr -d '[:space:]' < "$repository_root/apps/ios/VERSION")"
simulator="$(tr -d '[:space:]' < "$repository_root/tools/prolink-simulator/VERSION")"

"$script_dir/check-product-tag.sh" macos "v$macos"
"$script_dir/check-product-tag.sh" remote "lumi-remote-v$remote"
"$script_dir/check-product-tag.sh" simulator "prolink-simulator-v$simulator"

if "$script_dir/check-product-tag.sh" remote "v$remote" >/dev/null 2>&1; then
  echo "ERROR: Lumi Remote accepted the macOS tag namespace." >&2
  exit 1
fi
if "$script_dir/check-product-tag.sh" simulator "v$simulator" >/dev/null 2>&1; then
  echo "ERROR: the simulator accepted the macOS tag namespace." >&2
  exit 1
fi

echo "Independent product versioning tests passed."
