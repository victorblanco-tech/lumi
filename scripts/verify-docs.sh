#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

cd "$repository_root"
python3 "$script_dir/verify-docs.py"

if ! git diff --check; then
  echo "ERROR: documentation changes contain whitespace errors." >&2
  exit 1
fi
