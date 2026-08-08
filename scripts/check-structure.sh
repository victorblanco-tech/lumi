#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

required_paths=(
  "Cargo.toml"
  "engine/crates/lumi-domain"
  "engine/crates/lumi-deck-source"
  "engine/crates/lumi-blt-midi"
  "engine/crates/lumi-engine"
  "engine/crates/lumi-lighting-output"
  "engine/crates/lumi-midi-output"
  "engine/crates/lumi-midi-coremidi"
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
  "apps/macos/Config/Dev.xcconfig"
  "apps/macos/Config/Preview.xcconfig"
  "apps/macos/Config/Stable.xcconfig"
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
  "fixtures/demo-library-v1/simulator-e2e.json"
  "fixtures/epic-2a-v1/library-editor-e2e.json"
  "docs/development/demo-control.md"
  "docs/development/music-library-core.md"
  "docs/development/library-workspace.md"
  "docs/development/track-editor-preview.md"
  "docs/release/0.1.0-demo-and-limitations.md"
  "docs/release/0.1.0-epic-1-evidence.md"
  "docs/release/0.2.0-demo-and-limitations.md"
  "docs/release/0.2.0-epic-2a-evidence.md"
  "docs/release/0.3.0-release-readiness.md"
  "docs/release/0.3.0-dev-demo-and-limitations.md"
  "docs/release/unsigned-macos-installation.txt"
  "scripts/check-architecture.sh"
  "scripts/check-epic-2a-evidence.sh"
  "scripts/verify-rust.sh"
  "scripts/verify-apple.sh"
  "scripts/verify.sh"
  "scripts/package-macos-local.sh"
  "scripts/backup-macos-user-data.sh"
  "scripts/clone-macos-channel-data.sh"
  "docs"
  "scripts"
)

for required_path in "${required_paths[@]}"; do
  if [[ ! -e "$repository_root/$required_path" ]]; then
    echo "ERROR: required repository path '$required_path' is missing." >&2
    exit 1
  fi
done

retired_namespace='nl.''blancoservices'
if grep -R -Fq "$retired_namespace" \
  --exclude-dir=.git \
  --exclude-dir=.build \
  --exclude-dir=build \
  --exclude-dir=target \
  "$repository_root"; then
  echo "ERROR: the retired private namespace is present in the active repository." >&2
  exit 1
fi

if ! grep -Fq 'PRODUCT_BUNDLE_IDENTIFIER = co.victorblan.tech.lumi.dev' \
  "$repository_root/apps/macos/Config/Dev.xcconfig" \
  || ! grep -Fq 'PRODUCT_BUNDLE_IDENTIFIER = co.victorblan.tech.lumi.preview' \
    "$repository_root/apps/macos/Config/Preview.xcconfig" \
  || ! grep -Fq 'PRODUCT_BUNDLE_IDENTIFIER = co.victorblan.tech.lumi' \
    "$repository_root/apps/macos/Config/Stable.xcconfig"; then
  echo "ERROR: macOS release-channel bundle identities are incomplete." >&2
  exit 1
fi

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

if ! grep -Fq 'coremidi-rs' \
  "$repository_root/engine/crates/lumi-midi-coremidi/Cargo.toml"; then
  echo "ERROR: the macOS MIDI adapter must use the audited safe CoreMIDI wrapper." >&2
  exit 1
fi

if grep -REq '\.font\(\.[A-Za-z]|Color\.(red|green|orange|blue|purple|pink)' \
  "$repository_root/apps/macos/Lumi/App" \
  "$repository_root/apps/macos/Packages/LumiLiveWorkspace/Sources/LumiLiveWorkspace/Views"; then
  echo "ERROR: app feature views must use LumiDesignSystem typography and color tokens." >&2
  exit 1
fi

foundation_workflow="$repository_root/.github/workflows/foundation.yml"
if ! grep -Fq 'workflow_dispatch:' "$foundation_workflow" \
  || grep -Fq 'pull_request:' "$foundation_workflow" \
  || grep -Fq '      - dev' "$foundation_workflow"; then
  echo "ERROR: costly Foundation CI must remain manual and main-release-only during local-first development." >&2
  exit 1
fi
if ! grep -Fq 'runs-on: ubuntu-24.04' "$foundation_workflow" \
  || ! grep -Fq 'run: ./scripts/verify-rust.sh' "$foundation_workflow"; then
  echo "ERROR: portable Rust verification must run on the Linux CI job." >&2
  exit 1
fi
if ! grep -Fq 'runs-on: macos-26' "$foundation_workflow" \
  || ! grep -Fq 'run: ./scripts/verify-apple.sh' "$foundation_workflow"; then
  echo "ERROR: Apple application verification must run on the macOS CI job." >&2
  exit 1
fi
if grep -Fq 'run: ./scripts/verify.sh' "$foundation_workflow"; then
  echo "ERROR: CI must not run the complete cross-platform gate on macOS." >&2
  exit 1
fi
if [[ "$(grep -Fc 'uses: actions/cache@v5' "$foundation_workflow")" != "2" ]]; then
  echo "ERROR: both CI platform jobs must restore their manifest-keyed build cache." >&2
  exit 1
fi
if grep -Eq '^[[:space:]]*(swift|xcodebuild)[[:space:]]' \
  "$repository_root/scripts/verify-rust.sh"; then
  echo "ERROR: the portable Rust gate may not acquire Apple-only work." >&2
  exit 1
fi
if grep -Eq '^[[:space:]]*cargo[[:space:]]+(fmt|clippy|test)' \
  "$repository_root/scripts/verify-apple.sh"; then
  echo "ERROR: the Apple gate may not duplicate portable Rust verification." >&2
  exit 1
fi

echo "Repository structure check passed."
