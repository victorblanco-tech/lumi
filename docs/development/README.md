# Lumi development environment

Lumi's first build target is Apple Silicon with a macOS 15.0 deployment
minimum. Product code uses Rust 2024 and Swift 6 strict concurrency.

## Required tools

- Apple Silicon Mac;
- latest validated stable full Xcode, with macOS and iOS support;
- Rust installed through rustup;
- Git and GitHub CLI for repository collaboration.

Beta Xcode, additional Apple platform runtimes, Node.js, Docker, Java and
Homebrew are not prerequisites for Epic 1.

## Initial setup

After installing Xcode, open it once and complete its first-launch setup. Then
select it as the active developer directory:

```bash
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
sudo xcodebuild -runFirstLaunch
```

Install Rust using the official rustup installer and keep the default stable
profile. The repository's `rust-toolchain.toml` selects the exact validated
toolchain and required components.

If a newly opened non-interactive shell cannot find Rust, either start a new
shell or load Cargo's environment configuration. Lumi's repository scripts also
resolve the standard rustup installation directory explicitly.

## Verify

The environment check is read-only and safe to repeat:

```bash
./scripts/check-environment.sh
```

To include GitHub authentication in the check:

```bash
LUMI_CHECK_GITHUB_AUTH=1 ./scripts/check-environment.sh
```

Run all local foundation checks and builds with:

```bash
./scripts/verify.sh
```

During the solo POC phase this local command is the required feature and PR
gate. GitHub's `Foundation` workflow runs only on explicit manual dispatch and
after a merge to `main`; it does not duplicate local validation for every PR or
`dev` merge. The workflow assigns work to the least costly correct platform.
`./scripts/verify-rust.sh` is portable and owns Rust lint,
tests, migrations, contracts, and release benchmarks on Linux.
`./scripts/verify-apple.sh` owns Swift tests, the real Swift-to-Rust process,
the unsigned arm64 app, and deterministic visual evidence on macOS. Dependency
and build caches are keyed only by locked toolchain and build manifests; a
cache hit never skips a compiler, test, benchmark, or evidence command.

After a successful build, launch the exact unsigned development app with:

```bash
open -n /Users/victor/Engineering/Repo/Lumi/build/DerivedData/Build/Products/Debug/Lumi.app
```

Quit any older Lumi debug instance first. The repository and Xcode debug builds
share one bundle identifier, so running both at once makes it ambiguous which
binary macOS activates. The absolute `open -n` command above is the canonical
hands-on test path for every story.

The status becomes **Local engine ready** only after the app starts the bundled
Rust helper, authenticates over loopback, and receives its initial snapshot.

Generated Xcode build data is written below the ignored root `build/`
directory rather than mixed with source files.

Repository ownership, dependency direction, and naming rules are documented in
[`repository-structure.md`](repository-structure.md).

The app-scoped Rust process, loopback authentication, and shutdown behavior are
documented in [`local-engine-session.md`](local-engine-session.md).

Semantic presentation tokens, reusable components, global appearance, and key
notation are documented in [`design-system.md`](design-system.md).

Pure domain ownership, reducer behavior, ordering, revisions, and bounded
ingress are documented in [`domain-runtime.md`](domain-runtime.md).

The deterministic two-deck source, controllable clock, fixture, and adapter
boundary are documented in
[`simulator-deck-source.md`](simulator-deck-source.md).

Library-owned tracks enter that same provider-neutral path through
[`library-track-simulator.md`](library-track-simulator.md), including exact
timeline identity and logical Autoloop dry-run evidence.

Deterministic phrase planning, the minimal catalog, fallback behavior, golden
plan, and performance budget are documented in
[`deterministic-planner.md`](deterministic-planner.md).

Provider-neutral execution, the operational output gate, stale-context checks,
and the canonical dry-run transcript are documented in
[`dry-run-output.md`](dry-run-output.md).

Versioned demo controls, deterministic client-driven playback, state revision
checks, and the bounded engine event timeline are documented in
[`demo-control.md`](demo-control.md).

The provider-neutral music-library model, source contract, local SQLite schema,
and deterministic 10,000-track fixture are documented in
[`music-library-core.md`](music-library-core.md).

The native Library destination, bounded engine queries, presentation states,
and track-inspector boundary are documented in
[`library-workspace.md`](library-workspace.md).

The fixed-dark CDJ-inspired waveform, shared beat-coordinate system, isolated
read-only audio preview, controls, and cleanup rules are documented in
[`track-editor-preview.md`](track-editor-preview.md).

The authoritative, versioned, beat-quantized Lumi Phrase Point timeline and its editing
and recovery rules are documented in
[`phrase-timeline-editing.md`](phrase-timeline-editing.md).

Stable phrase-role identities, reversible archiving, usage diagnostics, and
future-only provider mappings are documented in
[`phrase-role-management.md`](phrase-role-management.md).

The provider-neutral four-Theme, Phrase Role, and Variant matrix, strict
same-role fallback, preflight coverage, and target-adapter boundary are
documented in [`autoloop-catalog.md`](autoloop-catalog.md).

The per-phrase `AUTO`, Theme-independent fixed-Variant, and optional exact
Theme override strategies, including stale validation and reset behavior, are
documented in
[`phrase-loop-strategies.md`](phrase-loop-strategies.md).

Versioned source comparison, metadata-safe refreshes, beat-aligned rebase,
explicit merge conflicts, and transactional recovery are documented in
[`source-reconciliation.md`](source-reconciliation.md).

The authoritative Live/Next presentation boundary, explicit degraded states,
accessibility identifiers, and locked-session PNG workflow are documented in
[`live-workspace.md`](live-workspace.md).

The cross-cutting Epic 2A golden scenarios, scale budgets, fault matrix, visual
manifest, demo walkthrough, and remaining isolated Rekordbox gate are collected
in [`../release/0.2.0-epic-2a-evidence.md`](../release/0.2.0-epic-2a-evidence.md).

## Troubleshooting

- `xcodebuild` points at Command Line Tools: rerun `xcode-select --switch`.
- Xcode first-launch is incomplete: open Xcode or run
  `sudo xcodebuild -runFirstLaunch`.
- `rustc` is missing: install Rust through rustup and open a new shell.
- GitHub authentication expired: run `gh auth refresh -h github.com`.
- Simulator discovery fails inside a restricted shell: verify it from Terminal
  with `xcrun simctl list runtimes`.
