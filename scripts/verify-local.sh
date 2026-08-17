#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
gate="${1:-}"

case "$gate" in
  functional)
    "$script_dir/verify-functional.sh"
    ;;
  technical)
    "$script_dir/verify-technical.sh"
    ;;
  full)
    "$script_dir/check-apple-test-exclusivity.sh"
    "$script_dir/verify.sh"
    ;;
  security)
    "$script_dir/verify-security.sh"
    ;;
  lab)
    "$script_dir/verify-show-lab.sh"
    ;;
  soak)
    "$script_dir/verify-live-integration-soak.sh"
    ;;
  *)
    echo "Usage: ./scripts/verify-local.sh {functional|technical|full|security|lab|soak}" >&2
    echo "See docs/development/local-quality-gates.md for prerequisites and evidence." >&2
    exit 2
    ;;
esac
