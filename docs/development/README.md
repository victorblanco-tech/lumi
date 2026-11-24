# Lumi development environment

Lumi's first build target is Apple Silicon with a macOS 15.0 deployment
minimum. Product code uses Rust 2024 and Swift 6 strict concurrency.

## macOS 27 compatibility validation

Development CI retains macOS 26 and adds the standard Apple Silicon `xcode-27`
runner. It checks the actual OS, Xcode and SDK before testing: a runner label
alone is not compatibility evidence. Compiler caches are separated by runner,
OS version, Xcode build and SDK. Both matrix jobs must pass the Development gate;
macOS 27 always runs the complete Apple verification, including native process
tests, both app builds, bundled helpers and 22 deterministic visual renders.

Only standard runners are used. They are free for this public repository;
the Apple job refuses to run if the repository becomes private. There are no
paid larger runners, new artifact uploads, releases, or changes to a local OS.
See [GitHub's standard runner policy](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-public-repositories).

Status: compatibility validation in progress on `dev`; no production support
claim yet. The minimum deployment target remains macOS 15.0. Existing production
and installed Dev apps are not replaced as part of CI validation.

The review focuses on the following Apple SDK changes: SwiftUI `@State`
initialization, changed Menu/Picker/Slider implementations, menu icon visibility,
and file-selection behavior. Do not add speculative workarounds; fix reproduced
compiler, runtime or interaction regressions while retaining older-OS behavior.
See [Apple's macOS 27 release notes](https://developer.apple.com/documentation/macos-release-notes/macos-27-release-notes).

Before declaring full show support, test real USB authorization/reconnection,
SMAppService installation/restart/quit, iPhone discovery/pairing, CDJ timing over
Ethernet and Wi-Fi, and SoundSwitch/CoreMIDI/Link with physical lighting on macOS
27. CI renders are not interactive UI acceptance, and a shared Link clock is not
proof of physical light/audio beat alignment. Do not assume earlier firewall
findings apply unchanged to the new OS. SoundSwitch requires version 2.11 or
newer for macOS 27 according to its
[compatibility statement](https://support.soundswitch.com/en/support/solutions/articles/69000883511-soundswitch-macos-27-golden-gate-compatibility).

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
open -n "$PWD/build/DerivedData/Build/Products/Debug/Lumi Dev.app"
```

Quit any older Lumi runtime first. Debug builds use the isolated Lumi Dev bundle
identifier and database, while Preview and Stable remain installed separately.
Only one channel should own the live MIDI/lighting endpoints at a time. The
explicit `open -n` command above is the canonical hands-on test path for every
story.

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

Library-owned tracks enter the production Local Playback path through
[`library-local-playback.md`](library-local-playback.md), including exact
timeline identity, audio transport, planning, and mapped Autoloop evidence.

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
