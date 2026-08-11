#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "Usage: $0 <app-helpers-directory>" >&2
  exit 64
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
helpers_directory="$1"
resources_directory="$(dirname "$helpers_directory")/Resources"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

profile_directory="release"

cd "$repository_root"
cargo build --locked --release -p lumi-engine
install -d "$helpers_directory"
install -m 755 "target/$profile_directory/lumi-engine" "$helpers_directory/lumi-engine"

# Bundle the Direct Pro DJ Link bridge and a matching Apple Silicon runtime so
# Lumi never depends on a separately installed JDK or Beat Link Trigger.
bridge_jar="$repository_root/bridges/prolink/target/lumi-prolink-bridge.jar"
bridge_sources="$repository_root/bridges/prolink/src"
toolchain_home="${LUMI_PACKAGING_JAVA_HOME:-$repository_root/build/package-toolchains/temurin-21-macos-aarch64/Contents/Home}"
runtime_cache="$repository_root/build/prolink-bridge-runtime"
runtime_modules="java.base,java.desktop,java.prefs,java.sql"
runtime_marker="$runtime_cache/.lumi-modules"

bridge_needs_build=0
if [[ ! -f "$bridge_jar" ]]; then
  bridge_needs_build=1
elif [[ -n "$(find "$bridge_sources" "$repository_root/bridges/prolink/pom.xml" -newer "$bridge_jar" -print -quit)" ]]; then
  bridge_needs_build=1
fi

if [[ "$bridge_needs_build" == "1" ]]; then
  if [[ ! -x "$toolchain_home/bin/java" ]]; then
    echo "ERROR: Java 21 toolchain is required to build the Pro DJ Link bridge." >&2
    exit 1
  fi
  if ! command -v mvn >/dev/null 2>&1; then
    echo "ERROR: Maven is required to build the Pro DJ Link bridge." >&2
    exit 1
  fi
  JAVA_HOME="$toolchain_home" mvn \
    --batch-mode \
    --no-transfer-progress \
    -Dmaven.repo.local="$repository_root/build/maven-repository" \
    --file "$repository_root/bridges/prolink/pom.xml" \
    package \
    -DskipTests
fi

if [[ ! -x "$runtime_cache/bin/java" || ! -f "$runtime_marker" || "$(cat "$runtime_marker" 2>/dev/null || true)" != "$runtime_modules" ]]; then
  if [[ ! -x "$toolchain_home/bin/jlink" ]]; then
    echo "ERROR: Java 21 jlink is required to package the Pro DJ Link bridge." >&2
    exit 1
  fi
  runtime_staging="$(mktemp -d "$repository_root/build/.prolink-runtime.XXXXXX")"
  trap 'rm -rf "$runtime_staging"' EXIT
  "$toolchain_home/bin/jlink" \
    --add-modules "$runtime_modules" \
    --strip-debug \
    --no-header-files \
    --no-man-pages \
    --output "$runtime_staging/runtime"
  if [[ -e "$runtime_cache" ]]; then
    mv "$runtime_cache" "$runtime_cache.invalid.$(date +%s)"
  fi
  printf '%s\n' "$runtime_modules" > "$runtime_staging/runtime/.lumi-modules"
  mv "$runtime_staging/runtime" "$runtime_cache"
  rm -rf "$runtime_staging"
  trap - EXIT
fi

install -d "$resources_directory"
rm -rf "$helpers_directory/prolink"
install -d "$resources_directory/prolink"
install -m 644 "$bridge_jar" "$resources_directory/prolink/lumi-prolink-bridge.jar"
rm -rf "$helpers_directory/prolink-runtime"
rm -rf "$resources_directory/prolink-runtime"
ditto "$runtime_cache" "$resources_directory/prolink-runtime"

# Bundle the separately licensed Ableton Link helper. Acquisition is explicit
# and checksum-pinned so normal local builds never depend on network access.
carabiner_cache="$repository_root/build/carabiner-runtime"
if [[ ! -x "$carabiner_cache/Carabiner" ]]; then
  echo "ERROR: managed Ableton Link runtime is missing." >&2
  echo "Run ./scripts/prepare-carabiner-runtime.sh once." >&2
  exit 1
fi
rm -rf "$resources_directory/link"
install -d "$resources_directory/link"
install -m 755 "$carabiner_cache/Carabiner" "$resources_directory/link/Carabiner"
install -m 644 "$carabiner_cache/PROVENANCE.txt" "$resources_directory/link/PROVENANCE.txt"
