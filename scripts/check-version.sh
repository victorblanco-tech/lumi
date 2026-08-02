#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
canonical_version="$(tr -d '[:space:]' < "$repository_root/VERSION")"

workspace_version="$(awk '
  /^\[workspace.package\]$/ { in_workspace_package = 1; next }
  /^\[/ { in_workspace_package = 0 }
  in_workspace_package && /^version[[:space:]]*=/ {
    value = $0
    sub(/^[^=]*=[[:space:]]*"/, "", value)
    sub(/"[[:space:]]*$/, "", value)
    print value
    exit
  }
' "$repository_root/Cargo.toml")"

xcconfig="$repository_root/apps/macos/Config/Base.xcconfig"
apple_product_version="$(awk -F '=' '/^LUMI_PRODUCT_VERSION[[:space:]]*=/ { gsub(/[[:space:]]/, "", $2); print $2 }' "$xcconfig")"
apple_marketing_version="$(awk -F '=' '/^MARKETING_VERSION[[:space:]]*=/ { gsub(/[[:space:]]/, "", $2); print $2 }' "$xcconfig")"
release_version="${canonical_version%%-*}"

[[ -n "$canonical_version" ]] || { echo "ERROR: VERSION is empty." >&2; exit 1; }
[[ "$workspace_version" == "$canonical_version" ]] || {
  echo "ERROR: Cargo workspace version '$workspace_version' differs from VERSION '$canonical_version'." >&2
  exit 1
}
[[ "$apple_product_version" == "$canonical_version" ]] || {
  echo "ERROR: Apple product version '$apple_product_version' differs from VERSION '$canonical_version'." >&2
  exit 1
}
[[ "$apple_marketing_version" == "$release_version" ]] || {
  echo "ERROR: Apple marketing version '$apple_marketing_version' differs from '$release_version'." >&2
  exit 1
}

echo "Version check passed: $canonical_version"
