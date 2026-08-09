#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
visual_evidence_directory="${1:-$repository_root/build/VisualEvidence}"

required_golden_files=(
  "fixtures/epic-2a-v1/library-editor-e2e.json"
  "fixtures/demo-library-v1/simulator-e2e.json"
  "fixtures/source-reconciliation/horizon-lines-preview.json"
)

for golden_file in "${required_golden_files[@]}"; do
  python3 -m json.tool "$repository_root/$golden_file" >/dev/null
done

required_visuals=(
  "library-ready-dark-camelot.png"
  "library-ready-light-classic.png"
  "track-editor-dark-camelot.png"
  "track-editor-light-host-classic.png"
  "phrase-role-settings-dark.png"
  "phrase-source-mapping-light.png"
  "autoloop-matrix-dark.png"
  "library-conflict-dark.png"
  "local-playback-library-next-dark-camelot.png"
)

for visual in "${required_visuals[@]}"; do
  if [[ ! -f "$visual_evidence_directory/$visual" ]]; then
    echo "ERROR: Epic 2A visual evidence '$visual' is missing." >&2
    exit 1
  fi
done

evidence_document="$repository_root/docs/release/0.2.0-epic-2a-evidence.md"
limitations_document="$repository_root/docs/release/0.2.0-demo-and-limitations.md"

for required_term in "10,000" "Rekordbox 7" "SoundSwitch" "PRO DJ LINK"; do
  if ! grep -Fq "$required_term" "$evidence_document" "$limitations_document"; then
    echo "ERROR: Epic 2A evidence does not name required boundary '$required_term'." >&2
    exit 1
  fi
done

echo "Epic 2A demo evidence check passed."
