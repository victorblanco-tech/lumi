#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
distribution_root="$repository_root/build/prolink-simulator-distribution"
version="$(tr -d '[:space:]' < "$repository_root/tools/prolink-simulator/VERSION")"
archive="$distribution_root/lumi-prolink-simulator-${version}-macos-$(uname -m).tar.gz"

if [[ -z "${JAVA_HOME:-}" ]]; then
  for java_candidate in \
    /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home \
    /usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home; do
    if [[ -x "$java_candidate/bin/jlink" ]]; then
      export JAVA_HOME="$java_candidate"
      break
    fi
  done
fi
if [[ -z "${JAVA_HOME:-}" || ! -x "$JAVA_HOME/bin/jlink" ]]; then
  echo "ERROR: OpenJDK 21 with jlink is required to package the simulator." >&2
  exit 1
fi

"$script_dir/verify-prolink-simulator.sh"

mkdir -p "$distribution_root"
staging_root="$(mktemp -d "$distribution_root/.staging.XXXXXX")"
trap 'rm -rf "$staging_root"' EXIT
bundle_root="$staging_root/lumi-prolink-simulator"
mkdir -p "$bundle_root/bin" "$bundle_root/lib"

"$JAVA_HOME/bin/jlink" \
  --add-modules java.base,java.desktop,java.prefs,java.sql,jdk.httpserver \
  --strip-debug \
  --no-header-files \
  --no-man-pages \
  --compress=zip-6 \
  --output "$bundle_root/runtime"

cp "$repository_root/tools/prolink-simulator/target/lumi-prolink-simulator.jar" \
  "$bundle_root/lib/lumi-prolink-simulator.jar"
cp "$repository_root/tools/prolink-simulator/bin/lumi-prolink-simulator" \
  "$bundle_root/bin/lumi-prolink-simulator"
cp "$repository_root/tools/prolink-simulator/README.md" "$bundle_root/README.md"
chmod +x "$bundle_root/bin/lumi-prolink-simulator"

temporary_archive="$archive.tmp"
tar -C "$staging_root" -czf "$temporary_archive" lumi-prolink-simulator
mv "$temporary_archive" "$archive"

echo "$archive"
