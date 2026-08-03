#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

required_paths=(
  "Cargo.toml"
  "engine/crates/lumi-domain"
  "engine/crates/lumi-deck-source"
  "engine/crates/lumi-engine"
  "engine/crates/lumi-lighting-output"
  "engine/crates/lumi-library"
  "engine/crates/lumi-library-source"
  "engine/crates/lumi-library-demo"
  "engine/crates/lumi-library-sqlite"
  "engine/crates/lumi-output-dry-run"
  "engine/crates/lumi-planner"
  "engine/crates/lumi-protocol"
  "engine/crates/lumi-simulator"
  "apps/macos/Lumi.xcodeproj"
  "apps/macos/Lumi"
  "apps/macos/Packages/LumiProtocol"
  "apps/macos/Packages/LumiEngineClient"
  "apps/macos/Packages/LumiDesignSystem"
  "apps/macos/Packages/LumiLiveWorkspace"
  "apps/macos/Packages/LumiLibraryWorkspace"
  "contracts"
  "fixtures"
  "fixtures/demo-session-v1/session.json"
  "fixtures/demo-session-v1/initial-transcript.ndjson"
  "fixtures/demo-session-v1/next-plan.json"
  "fixtures/demo-session-v1/output-effects.json"
  "fixtures/demo-session-v1/canonical-e2e.json"
  "fixtures/demo-library-v1/library.json"
  "docs/development/demo-control.md"
  "docs/development/music-library-core.md"
  "docs/development/library-workspace.md"
  "docs/development/track-editor-preview.md"
  "docs/release/0.1.0-demo-and-limitations.md"
  "docs/release/0.1.0-epic-1-evidence.md"
  "scripts/check-architecture.sh"
  "docs"
  "scripts"
)

for required_path in "${required_paths[@]}"; do
  if [[ ! -e "$repository_root/$required_path" ]]; then
    echo "ERROR: required repository path '$required_path' is missing." >&2
    exit 1
  fi
done

for forbidden_name in Utils Common Shared Helpers Misc; do
  if find "$repository_root" \
    -type d \
    \( -path "$repository_root/.git" -o -path "$repository_root/build" -o -path "$repository_root/target" \) -prune \
    -o -type d -name "$forbidden_name" -print \
    | grep -q .; then
    echo "ERROR: ambiguous '$forbidden_name' directory found." >&2
    exit 1
  fi
done

if grep -Eq 'tokio|serde|tracing' "$repository_root/engine/crates/lumi-domain/Cargo.toml"; then
  echo "ERROR: lumi-domain may not depend on runtime, wire, or observability crates." >&2
  exit 1
fi

if grep -REq '\.font\(\.[A-Za-z]|Color\.(red|green|orange|blue|purple|pink)' \
  "$repository_root/apps/macos/Lumi/App" \
  "$repository_root/apps/macos/Packages/LumiLiveWorkspace/Sources/LumiLiveWorkspace/Views"; then
  echo "ERROR: app feature views must use LumiDesignSystem typography and color tokens." >&2
  exit 1
fi

echo "Repository structure check passed."
