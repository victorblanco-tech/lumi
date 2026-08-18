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
apple_build_number="$(awk -F '=' '/^CURRENT_PROJECT_VERSION[[:space:]]*=/ { gsub(/[[:space:]]/, "", $2); print $2 }' "$xcconfig")"
bridge_version="$(awk -F '[<>]' '/<version>/{ print $3; exit }' "$repository_root/bridges/prolink/pom.xml")"
simulator_version="$(awk -F '[<>]' '/<version>/{ print $3; exit }' "$repository_root/tools/prolink-simulator/pom.xml")"
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
[[ "$bridge_version" == "$canonical_version" ]] || {
  echo "ERROR: Pro DJ Link bridge version '$bridge_version' differs from VERSION '$canonical_version'." >&2
  exit 1
}
[[ "$simulator_version" == "$canonical_version" ]] || {
  echo "ERROR: Pro DJ Link simulator version '$simulator_version' differs from VERSION '$canonical_version'." >&2
  exit 1
}

if [[ "$canonical_version" =~ -dev-([0-9]+)$ ]]; then
  dev_sequence="${BASH_REMATCH[1]}"
  [[ "$apple_build_number" == "$dev_sequence" ]] || {
    echo "ERROR: Apple build number '$apple_build_number' differs from dev sequence '$dev_sequence'." >&2
    exit 1
  }
fi

if [[ "$canonical_version" =~ -rc-([0-9]+)$ ]]; then
  rc_sequence="${BASH_REMATCH[1]}"
  [[ "$apple_build_number" == "$rc_sequence" ]] || {
    echo "ERROR: Apple build number '$apple_build_number' differs from rc sequence '$rc_sequence'." >&2
    exit 1
  }
fi

echo "Version check passed: $canonical_version"
