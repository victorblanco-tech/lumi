#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
version="1.2.0"
asset="Carabiner_Mac.dmg"
expected_sha256="13a53b16fb044fbfde46122804948e6d95137cc0a7812c3e7b34767817b5bdae"
expected_binary_sha256="27d771f51d0f7cc6c9c51a65a44abbdfc6d15e6536840cc8b4793f1dd6602fca"
expected_source_revision="b7310b6e01443d90b24200318aee38e4313cd2a0"
expected_gflags_revision="e171aa2d15ed9eb17054558e0b3a6a413bb01067"
expected_gflags_docs_revision="8411df715cf522606e3b1aca386ddfc0b63d34b4"
expected_link_revision="41d9aa111f702e78b6fbaee9d3e06dda1db6420d"
expected_asio_revision="c465349fa5cd91a64bb369f5131ceacab2c0c1c3"
runtime_directory="$repository_root/build/carabiner-runtime"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/lumi-carabiner.XXXXXX")"
mount_directory="$temporary_root/mount"
source_directory="$temporary_root/carabiner-source"
source_archive_name="Carabiner-$version-complete-source.tar.gz"
mounted=0

cleanup() {
  if [[ "$mounted" == "1" ]]; then
    hdiutil detach "$mount_directory" -quiet || true
  fi
  rm -rf "$temporary_root"
}
trap cleanup EXIT

command -v curl >/dev/null 2>&1 || {
  echo "ERROR: curl is required to acquire the pinned Carabiner runtime." >&2
  exit 1
}
command -v git >/dev/null 2>&1 || {
  echo "ERROR: git is required to acquire complete corresponding source." >&2
  exit 1
}

mkdir -p "$mount_directory"
curl --fail --location --silent --show-error \
  "https://github.com/Deep-Symmetry/carabiner/releases/download/v$version/$asset" \
  --output "$temporary_root/$asset"

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

actual_binary_sha256="$(shasum -a 256 "$mount_directory/Carabiner" | awk '{print $1}')"
if [[ "$actual_binary_sha256" != "$expected_binary_sha256" ]]; then
  echo "ERROR: Carabiner executable checksum mismatch." >&2
  exit 1
fi

git clone \
  --recurse-submodules \
  --shallow-submodules \
  --branch "v$version" \
  --depth 1 \
  https://github.com/Deep-Symmetry/carabiner.git \
  "$source_directory"

verify_revision() {
  local directory="$1"
  local expected="$2"
  local label="$3"
  local actual
  actual="$(git -C "$directory" rev-parse HEAD)"
  if [[ "$actual" != "$expected" ]]; then
    echo "ERROR: $label source revision '$actual' differs from '$expected'." >&2
    exit 1
  fi
}

verify_revision "$source_directory" "$expected_source_revision" "Carabiner"
verify_revision "$source_directory/gflags" "$expected_gflags_revision" "gflags"
verify_revision "$source_directory/gflags/doc" "$expected_gflags_docs_revision" "gflags docs"
verify_revision "$source_directory/link" "$expected_link_revision" "Ableton Link"
verify_revision "$source_directory/link/modules/asio-standalone" "$expected_asio_revision" "ASIO"

if git -C "$source_directory" submodule status --recursive | grep -Eq '^[+-]'; then
  echo "ERROR: one or more Carabiner source submodules are missing or modified." >&2
  exit 1
fi

COPYFILE_DISABLE=1 tar \
  --exclude='.git' \
  --exclude='.DS_Store' \
  -czf "$temporary_root/$source_archive_name" \
  -C "$temporary_root" \
  "$(basename "$source_directory")"

source_archive_sha256="$(shasum -a 256 "$temporary_root/$source_archive_name" | awk '{print $1}')"

install -d "$runtime_directory"
install -m 755 "$mount_directory/Carabiner" "$runtime_directory/Carabiner"
install -m 644 "$source_directory/LICENSE.md" "$runtime_directory/LICENSE.md"
install -m 644 "$temporary_root/$source_archive_name" \
  "$runtime_directory/$source_archive_name"
{
  echo "Carabiner $version"
  echo "Source: https://github.com/Deep-Symmetry/carabiner/tree/v$version"
  echo "Asset SHA-256: $expected_sha256"
  echo "Executable SHA-256 before app signing: $expected_binary_sha256"
  echo "Source revision: $expected_source_revision"
  echo "Ableton Link revision: $expected_link_revision"
  echo "Complete source archive SHA-256: $source_archive_sha256"
  echo "License: GPL-2.0-or-later"
} > "$runtime_directory/PROVENANCE.txt"

echo "Prepared pinned Carabiner runtime:"
echo "  $runtime_directory/Carabiner"
echo "  $runtime_directory/$source_archive_name"
