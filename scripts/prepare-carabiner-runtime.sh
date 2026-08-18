#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
version="1.2.0"
asset="Carabiner_Mac.dmg"
expected_sha256="13a53b16fb044fbfde46122804948e6d95137cc0a7812c3e7b34767817b5bdae"
runtime_directory="$repository_root/build/carabiner-runtime"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/lumi-carabiner.XXXXXX")"
mount_directory="$temporary_root/mount"
mounted=0

cleanup() {
  if [[ "$mounted" == "1" ]]; then
    hdiutil detach "$mount_directory" -quiet || true
  fi
  rm -rf "$temporary_root"
}
trap cleanup EXIT

command -v gh >/dev/null 2>&1 || {
  echo "ERROR: GitHub CLI is required to acquire the pinned Carabiner runtime." >&2
  exit 1
}

mkdir -p "$mount_directory"
gh release download "v$version" \
  --repo Deep-Symmetry/carabiner \
  --pattern "$asset" \
  --dir "$temporary_root"

actual_sha256="$(shasum -a 256 "$temporary_root/$asset" | awk '{print $1}')"
if [[ "$actual_sha256" != "$expected_sha256" ]]; then
  echo "ERROR: Carabiner checksum mismatch." >&2
  exit 1
fi

hdiutil attach "$temporary_root/$asset" \
  -nobrowse \
  -readonly \
  -mountpoint "$mount_directory" \
  -quiet
mounted=1

if [[ ! -x "$mount_directory/Carabiner" ]]; then
  echo "ERROR: official Carabiner release does not contain its executable." >&2
  exit 1
fi
if [[ "$(lipo -archs "$mount_directory/Carabiner")" != *arm64* ]]; then
  echo "ERROR: official Carabiner release does not contain arm64." >&2
  exit 1
fi

install -d "$runtime_directory"
install -m 755 "$mount_directory/Carabiner" "$runtime_directory/Carabiner"
{
  echo "Carabiner $version"
  echo "Source: https://github.com/Deep-Symmetry/carabiner/tree/v$version"
  echo "Asset SHA-256: $expected_sha256"
  echo "License: GPL-2.0-or-later"
} > "$runtime_directory/PROVENANCE.txt"

echo "Prepared pinned Carabiner runtime:"
echo "  $runtime_directory/Carabiner"
