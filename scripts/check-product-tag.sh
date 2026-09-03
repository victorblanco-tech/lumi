#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
product="${1:-}"
tag="${2:-}"

case "$product" in
  macos)
    version_file="$repository_root/VERSION"
    prefix="v"
    ;;
  remote)
    version_file="$repository_root/apps/ios/VERSION"
    prefix="lumi-remote-v"
    ;;
  simulator)
    version_file="$repository_root/tools/prolink-simulator/VERSION"
    prefix="prolink-simulator-v"
    ;;
  *)
    echo "Usage: check-product-tag.sh {macos|remote|simulator} <tag>" >&2
    exit 2
    ;;
esac

version="$(tr -d '[:space:]' < "$version_file")"
expected="${prefix}${version}"
if [[ "$tag" != "$expected" ]]; then
  echo "ERROR: tag '$tag' does not match $product product version '$expected'." >&2
  exit 1
fi

echo "$product tag check passed: $tag"
