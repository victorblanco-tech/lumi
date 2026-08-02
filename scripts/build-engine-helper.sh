#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "Usage: $0 <app-helpers-directory>" >&2
  exit 64
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
helpers_directory="$1"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

profile_directory="debug"
if [[ "${CONFIGURATION:-Debug}" == "Release" ]]; then
  profile_directory="release"
fi

cd "$repository_root"
if [[ "$profile_directory" == "release" ]]; then
  cargo build --locked --release -p lumi-engine
else
  cargo build --locked -p lumi-engine
fi
install -d "$helpers_directory"
install -m 755 "target/$profile_directory/lumi-engine" "$helpers_directory/lumi-engine"
