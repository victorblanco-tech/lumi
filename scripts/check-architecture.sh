#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

reject_dependency() {
  local manifest="$1"
  local pattern="$2"
  local explanation="$3"
  if grep -Eq "$pattern" "$repository_root/$manifest"; then
    echo "ERROR: $explanation" >&2
    exit 1
  fi
}

reject_product_dependency() {
  local manifest="$1"
  local pattern="$2"
  local explanation="$3"
  if sed -n '/^\[dependencies\]/,/^\[/p' "$repository_root/$manifest" \
    | grep -Eq "$pattern"; then
    echo "ERROR: $explanation" >&2
    exit 1
  fi
}

if grep -q '^\[dependencies\]' "$repository_root/engine/crates/lumi-domain/Cargo.toml"; then
  echo "ERROR: lumi-domain must remain dependency-free." >&2
  exit 1
fi

reject_dependency \
  "engine/crates/lumi-protocol/Cargo.toml" \
  'lumi-(domain|engine|simulator|planner|lighting-output|output-dry-run)' \
  "lumi-protocol may not depend on domain, application, or provider crates."
reject_dependency \
  "engine/crates/lumi-planner/Cargo.toml" \
  'lumi-(engine|simulator|deck-source|lighting-output|output-dry-run|protocol)' \
  "lumi-planner may depend inward on the domain only."
reject_dependency \
  "engine/crates/lumi-simulator/Cargo.toml" \
  'lumi-(engine|planner|lighting-output|output-dry-run|protocol)' \
  "lumi-simulator may not depend on planning, output, transport, or the engine."
reject_dependency \
  "engine/crates/lumi-lighting-output/Cargo.toml" \
  'lumi-(engine|simulator|planner|output-dry-run|protocol)' \
  "the output port may not depend on adapters or application orchestration."
reject_dependency \
  "engine/crates/lumi-library/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|deck-source|lighting-output|output-dry-run|library-source|library-demo|library-sqlite)' \
  "the canonical library model and repository port may depend inward on the domain only."
reject_dependency \
  "engine/crates/lumi-library-source/Cargo.toml" \
  'lumi-(domain|engine|simulator|planner|protocol|deck-source|lighting-output|output-dry-run|library-demo|library-sqlite)' \
  "the library source port may depend only on the canonical library model."
reject_dependency \
  "engine/crates/lumi-library-demo/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|deck-source|lighting-output|output-dry-run|library-sqlite)' \
  "the demo library source must remain independent from runtime, transport, and persistence adapters."
reject_product_dependency \
  "engine/crates/lumi-library-sqlite/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|deck-source|lighting-output|output-dry-run|library-source|library-demo)' \
  "the SQLite library adapter may depend inward on the canonical model only."

reject_dependency \
  "apps/macos/Packages/LumiDesignSystem/Package.swift" \
  'Lumi(LiveWorkspace|EngineClient|Protocol)' \
  "LumiDesignSystem must remain independent from feature and transport packages."
reject_dependency \
  "apps/macos/Packages/LumiLiveWorkspace/Package.swift" \
  'LumiEngineClient' \
  "LumiLiveWorkspace views may not import process or transport ownership."

echo "Architecture dependency check passed."
