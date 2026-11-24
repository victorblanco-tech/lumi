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
It also runs the Rust database/SQLCipher, Rekordbox-device, Remote TLS, stream,
Pro DJ Link, CoreMIDI packet and timing-adapter suites natively on macOS 27.

Only standard runners are used. They are free for this public repository;
the Apple job refuses to run if the repository becomes private. There are no
paid larger runners, new artifact uploads, releases, or changes to a local OS.
The repository cache limit was verified at the included 10 GB on 2026-11-24;
do not raise it without approval, since
[additional cache storage is billable](https://docs.github.com/en/billing/concepts/product-billing/github-actions).
See [GitHub's standard runner policy](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-public-repositories).

Status (2026-11-24): the [complete development gate passed](https://github.com/victorblanco-tech/lumi/actions/runs/45488879534)
on macOS 26 and macOS 27. The macOS 27 runner used 27.0.1, Xcode 27.1 and SDK
27.0. Both app builds, 252 Swift tests, all 22 visual renders and 170 additional
native Rust tests passed. Four native cases remain explicitly opt-in: the
release-only library benchmark (run separately by the Linux Rust gate) and
three real-Carabiner helper/soak tests, which this native test invocation did
not execute. This is build/automated coverage, not full show certification.
The minimum deployment target remains macOS 15.0. Existing production and
installed Dev apps are not replaced as part of CI validation.

The Xcode 27 build exposed a reproducible Rust host-plugin failure (`E0463` for
`thiserror_impl`) when the app's `MACOSX_DEPLOYMENT_TARGET` leaked into compiler
tools. Helpers now use an explicit Rust target and a narrowly scoped compiler
wrapper: runtime crates retain the product deployment floor and selected SDK;
host-only proc macros/build tools omit only that product floor. Existing compiler
wrappers remain chained. LaunchAgent metadata is linked only into the final
service executables, never their dependencies; the USB worker remains separate.
The [isolated regression run](https://github.com/victorblanco-tech/lumi/actions/runs/45484287477)
reproduced the old failure and passed with host/product isolation. The optional
`Apple Rust host probe` workflow retains those diagnostic comparisons; its final
isolated build is mandatory, while earlier failing variants are diagnostic data.

Library visual evidence uses an unshown AppKit window with `NSHostingView` and
an explicit 2× bitmap. The old `ImageRenderer` path failed on macOS 27 when the
scene contained native tables/split views. This is a test-host change, not a
change to the production views. The
[macOS 27 visual probe](https://github.com/victorblanco-tech/lumi/actions/runs/45488792186)
passed all 13 library/editor/settings renders; the full gate also retains the
nine Live renders. These synthetic fixtures are test evidence, not replacement
screenshots for the public README or evidence of a live show.

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

The `Development` workflow runs on pushes to `dev` and manual dispatch. Change
classification selects the required documentation, Rust and Apple gates;
macOS 27 always receives full Apple verification when Apple validation is
required. The workflow assigns work to the least costly correct platform.
`./scripts/verify-rust.sh` is portable and owns Rust lint,
tests, migrations, contracts, and release benchmarks on Linux.
`./scripts/verify-apple.sh` owns Swift tests, the real Swift-to-Rust process,
the unsigned arm64 app, and deterministic visual evidence on macOS. Dependency
and build caches include the locked dependencies and actual Apple toolchain; a
cache hit never skips a compiler, test, benchmark, or evidence command.

After a successful build, launch the exact unsigned development app with:

```bash
open -n "$PWD/build/DerivedData/Build/Products/Dev/Lumi.app"
```

Quit any older Lumi runtime first, with the user's approval if it is in use.
Dev builds use the isolated Lumi Dev bundle
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
