#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"
if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

if [[ -z "${JAVA_HOME:-}" ]]; then
  for java_candidate in \
    /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home \
    /usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home; do
    if [[ -x "$java_candidate/bin/java" ]]; then
      export JAVA_HOME="$java_candidate"
      break
    fi
  done
fi

if [[ -z "${JAVA_HOME:-}" || ! -x "$JAVA_HOME/bin/java" ]]; then
  echo "ERROR: OpenJDK 21 is required to run this development simulator." >&2
  exit 1
fi
if ! command -v mvn >/dev/null 2>&1; then
  echo "ERROR: Maven is required to build this development simulator." >&2
  exit 1
fi

export PATH="$JAVA_HOME/bin:$PATH"
mvn \
  --batch-mode \
  --no-transfer-progress \
  -Dmaven.repo.local="$repository_root/build/maven-repository" \
  --file "$repository_root/tools/prolink-simulator/pom.xml" \
  package

cargo build --release --manifest-path "$repository_root/Cargo.toml" --bin lumi-simulator-media
export LUMI_SIM_MEDIA_READER="$repository_root/target/release/lumi-simulator-media"
exec "$JAVA_HOME/bin/java" \
  -jar "$repository_root/tools/prolink-simulator/target/lumi-prolink-simulator.jar" \
  "$@"
