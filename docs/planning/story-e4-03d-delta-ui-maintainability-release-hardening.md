# Story E4-03D: Delta UI, maintainability and release hardening

- Status: **Implementation complete — Instruments and public signing pending**
- Priority: **P1 High**
- Effort: **8**
- Components: Engine, Protocol, macOS, Security, Delivery
- GitHub tracking: [#103](https://github.com/victorblanco-tech/lumi/issues/103)

## User outcome

As a DJ, Live remains responsive and compact while the show-critical engine has
stable ownership boundaries, useful diagnostics and a reviewable release
security posture.

## Scope

### D1 — Revisioned read models

- split immutable library/editor data from high-frequency transport and status;
- publish revisioned deltas or topic snapshots instead of rebuilding the full
  application snapshot every 250 ms;
- cache waveform, beat grid, phrases, hot cues and plan projections by stable
  revision;
- update Swift observable state only when the relevant revision changes;
- cap payload size and construction time with regression tests.

### D2 — UI performance

- profile Live Decks with Instruments using two active decks and visible plan;
- remove avoidable main-thread decoding, layout and animation work;
- let display interpolation drop frames without affecting engine truth;
- retain accepted Editor behavior, where free navigation has different needs
  from fixed-playhead Live behavior;
- keep status notifications in one overlay so they do not move decks.

### D3 — Maintainability seams

- split the largest engine and Swift files only along proven ownership seams;
- extract snapshot projection, library command workers, realtime orchestration
  and service lifecycle without changing domain language;
- add architecture checks for forbidden realtime dependencies;
- keep framework and naming conventions uniform; no generic `Utils`, `Common`,
  `Shared`, `Helpers` or `Misc` buckets.

### D4 — Security and release supply chain

- prevent session credentials from being inherited by Java/Link child helpers;
- redact or classify stderr and public OSLog content;
- keep simulator remote control explicitly development-only, token-authenticated
  and bounded;
- document that a future iPhone companion uses explicit pairing and scoped
  credentials, never the desktop session token;
- produce checksums, license notices and a bounded SBOM for RC artifacts;
- add hardened runtime/notarization before a public stable distribution; ad-hoc
  signing remains acceptable only for the current private/open-source test path.

## Acceptance criteria

- unchanged library/editor revisions are not serialized or decoded on each
  transport update;
- Live payload size and snapshot construction time have explicit budgets and
  do not regress by more than the agreed threshold from the E4-03A baseline;
- two-deck Live visual playback is smooth enough for use while engine timing
  metrics remain unchanged under the same workload;
- realtime crates/modules cannot import SQLite, UI or waveform rendering;
- refactored modules have focused contract tests and no accepted behavior loss;
- child helper environments do not contain the Lumi IPC session token;
- logs contain no token, full user path, mounted-media payload or raw protocol
  body by default;
- the RC artifact includes checksum, SBOM, third-party notices and reproducible
  local verification evidence.

## Non-goal

The UI does not become the realtime clock. Delta updates improve display and
maintainability only; all lighting correctness remains engine-owned.

## Dev-35 implementation result

- snapshots expose a monotonic library revision and the macOS monitor requests
  the full library projection only after that revision changes;
- realtime MIDI health is projected as a compact diagnostic and participates
  in Live readiness without putting scheduling logic in SwiftUI;
- helper environments remain allowlisted and exclude the Lumi IPC credential;
- local packaging emits checksum, licence, notices and an SPDX 2.3 SBOM;
- full Apple verification covers the application bundle, engine client,
  library workspace, Live workspace and visual evidence.

The measured snapshot budgets remain within the accepted baseline. A retained
two-deck Instruments profile and hardened-runtime/notarized public packaging are
release work; private Dev distribution remains ad-hoc signed by design.
