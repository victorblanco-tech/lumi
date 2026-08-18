#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

"$script_dir/verify-rust.sh"
"$script_dir/verify-prolink-bridge.sh"
"$script_dir/verify-prolink-simulator.sh"
"$script_dir/verify-apple.sh"

echo "Repository verification passed."
