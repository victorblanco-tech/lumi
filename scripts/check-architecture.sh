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
  "engine/crates/lumi-midi-output/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|midi-coremidi)' \
  "the MIDI output port may not depend on adapters or application orchestration."
reject_dependency \
  "engine/crates/lumi-midi-coremidi/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|domain|library)' \
  "the CoreMIDI adapter may depend only on the provider-neutral MIDI output port."
reject_dependency \
  "engine/crates/lumi-blt-midi/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|library|lighting-output|midi-output)' \
  "the Beat Link Trigger adapter may depend only on deck-source/domain ports and raw CoreMIDI messages."
reject_dependency \
  "engine/crates/lumi-prolink-input/Cargo.toml" \
  'lumi-(engine|simulator|planner|protocol|library|lighting-output|midi-output|midi-coremidi|blt-midi)' \
  "the direct Pro DJ Link input boundary may not depend on engine, planning, library, output, MIDI, or BLT adapters."
reject_product_dependency \
  "engine/crates/lumi-engine/Cargo.toml" \
  'lumi-blt-midi' \
  "the production engine must use direct Pro DJ Link and may not link the retired BLT MIDI adapter."
reject_product_dependency \
  "engine/crates/lumi-engine/Cargo.toml" \
  'lumi-rekordbox-(xml|resolver)' \
  "the production engine must ingest Rekordbox data through mounted OneLibrary USB media only."
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

if grep -Eiq 'soundswitch|midi|bank(_id|number)?|slot(_id|number)?' \
  "$repository_root/engine/crates/lumi-library/src/autoloop_catalog.rs"; then
  echo "ERROR: the logical Autoloop catalog may not contain provider or physical-address concepts." >&2
  exit 1
fi

reject_dependency \
  "apps/macos/Packages/LumiDesignSystem/Package.swift" \
  'Lumi(LiveWorkspace|EngineClient|Protocol)' \
  "LumiDesignSystem must remain independent from feature and transport packages."
reject_dependency \
  "apps/macos/Packages/LumiLiveWorkspace/Package.swift" \
  'LumiEngineClient' \
  "LumiLiveWorkspace views may not import process or transport ownership."
reject_dependency \
  "apps/macos/Packages/LumiLibraryWorkspace/Package.swift" \
  'Lumi(EngineClient|LiveWorkspace)' \
  "LumiLibraryWorkspace must remain independent from process ownership and other features."

if find "$repository_root/apps/macos" -type f -name '*.swift' -print0 \
  | xargs -0 grep -Eq 'BeatLinkTriggerIntegrationView|BLT MIDI Deck Frame'; then
  echo "ERROR: the macOS product must not expose the retired BLT MIDI runtime path." >&2
  exit 1
fi

if sed -n '1,/^#\[cfg(test)\]/p' \
  "$repository_root/engine/crates/lumi-engine/src/commands.rs" \
  | grep -Eq 'previewRekordboxXmlSync|applyRekordboxXmlSync|importRekordboxAnalysis|inspectRekordboxDevice|syncRekordboxDevice|resolveRekordboxDeviceConflict'; then
  echo "ERROR: library ingestion must stay outside the realtime engine protocol." >&2
  exit 1
fi

if grep -Eq 'previewRekordboxXMLSync|applyRekordboxXMLSync|importRekordboxAnalysis|inspectRekordboxDevice|syncRekordboxDevice|resolveRekordboxDeviceConflict' \
  "$repository_root/apps/macos/Packages/LumiEngineClient/Sources/LumiEngineClient/EngineCommand.swift"; then
  echo "ERROR: removable-media work must use the isolated bounded worker." >&2
  exit 1
fi

echo "Architecture dependency check passed."
