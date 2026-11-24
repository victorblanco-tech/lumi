#!/usr/bin/env bash
set -euo pipefail

# Cargo's explicit --target keeps runtime crates separate from host build tools.
# Xcode's deployment floor belongs to the product, not proc macros/build scripts.
# macOS 27 rejects proc-macro loading when that product floor leaks into the host
# compilation. Keep the selected SDK and every target-side setting unchanged.
compiler="$1"
shift
command=("$compiler")
if [[ -n "${LUMI_UPSTREAM_RUSTC_WRAPPER:-}" ]]; then
  command=("$LUMI_UPSTREAM_RUSTC_WRAPPER" "$compiler")
fi
for argument in "$@"; do
  case "$argument" in
    --target|--target=*) exec "${command[@]}" "$@" ;;
  esac
done
exec env -u MACOSX_DEPLOYMENT_TARGET "${command[@]}" "$@"
