#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

for required_tool in jq python3; do
  if ! command -v "$required_tool" >/dev/null 2>&1; then
    echo "ERROR: '$required_tool' is required for the license inventory gate." >&2
    exit 1
  fi
done

temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/lumi-license.XXXXXX")"
cleanup() {
  rm -rf "$temporary_directory"
}
trap cleanup EXIT

sbom_path="$temporary_directory/lumi.spdx.json"
"$script_dir/generate-sbom.sh" "$sbom_path" >/dev/null

unknown_licenses="$(
  jq -r \
    '.packages[] | select(.licenseDeclared == null or .licenseDeclared == "" or .licenseDeclared == "NOASSERTION") | "\(.name) \(.versionInfo // "unknown")"' \
    "$sbom_path"
)"
if [[ -n "$unknown_licenses" ]]; then
  echo "ERROR: the release SBOM contains dependencies without a declared license:" >&2
  printf '%s\n' "$unknown_licenses" >&2
  exit 1
fi

for required_package in \
  'pkg:maven/org.deepsymmetry/beat-link@8.0.0' \
  'pkg:github/Deep-Symmetry/carabiner@v1.2.0' \
  'pkg:github/Ableton/link@41d9aa111f702e78b6fbaee9d3e06dda1db6420d' \
  'pkg:generic/sqlcipher@libsqlite3-sys-0.38.1-bundle'; do
  if ! jq -e --arg purl "$required_package" \
    '[.packages[].externalRefs[]?.referenceLocator] | index($purl) != null' \
    "$sbom_path" >/dev/null; then
    echo "ERROR: required packaged component '$required_package' is absent from the SBOM." >&2
    exit 1
  fi
done

for notice in beat-link Carabiner 'Ableton Link' SQLCipher OpenJDK; do
  if ! grep -Fq "$notice" "$repository_root/THIRD_PARTY_NOTICES.md"; then
    echo "ERROR: THIRD_PARTY_NOTICES.md does not cover '$notice'." >&2
    exit 1
  fi
done

echo "Release license inventory gate passed."
