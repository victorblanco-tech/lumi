# Epic E4-03: Show-grade hardening

- Status: **Dev implementation complete — RC/lab evidence pending**
- Phase: **4 – macOS Beta (`0.4.0`)**
- Priority: **P0 Critical**
- Target: **before `0.4.0-rc-1`**
- Depends on: E4-02 and ADR-0030
- Architecture: ADR-0031
- GitHub tracking: [#99](https://github.com/victorblanco-tech/lumi/issues/99)

## Outcome

Lumi can run a complete DJ show while library synchronization, waveform
rendering and diagnostics are active without delaying, duplicating or losing a
planned SoundSwitch AutoLoop command. The engine survives helper, UI and source
failures predictably, protects the library during backup and recovery, and has
repeatable local evidence for functionality, performance, robustness and
security.

This epic hardens the existing product. It is not a rewrite and it does not add
new end-user workflow breadth. Every internal change must preserve the accepted
Local Playback and Live Deck behavior through characterization tests.

## Why now

The current end-to-end chain works, but several responsibilities still share
the same execution and snapshot paths:

- the 20 ms integration pump and synchronous application commands run in one
  engine task;
- a full UI snapshot rebuilds library, deck, waveform and output state even
  when only transport changed;
- Pro DJ Link ingress can accumulate unbounded work before it is drained;
- the macOS app launches a single-client child process while the accepted
  architecture requires a reconnectable service;
- backup and restore can begin before engine process exit is confirmed;
- local Apple tests can conflict with an installed running Lumi instance and
  report misleading CoreMIDI failures.

These are release risks because the sparse Bank/AutoLoop command is more
timing-critical than visual smoothness or continuous Link tempo publication.

## Delivery phases and stories

| Order | Story | Visible/verifiable milestone |
|---|---|---|
| 1 | [E4-03A – Regression baseline and security floor](story-e4-03a-regression-baseline-and-security-floor.md) | One documented local command per functional, technical, security and lab gate; deterministic failures and retained baseline evidence |
| 2 | [E4-03B – Realtime AutoLoop execution lane](story-e4-03b-realtime-autoloop-execution-lane.md) | AutoLoops remain exact during UI polling, USB work, seeks, hotcues, master changes and ingress bursts |
| 3 | [E4-03C – Engine service and data recovery](story-e4-03c-engine-service-and-data-recovery.md) | UI reconnects to an autonomous engine; backup/restore and restart are transactional and crash-tested |
| 4 | [E4-03D – Delta UI, maintainability and release hardening](story-e4-03d-delta-ui-maintainability-release-hardening.md) | Smooth bounded UI reads, smaller ownership seams, hardened helpers/logging and complete RC evidence |

The stories are sequential at their acceptance boundary. Characterization
tests from A are required before runtime work in B. Service work in C may start
behind an adapter once B's realtime contract is fixed. D consumes the stable
contracts from B and C.

## Epic-wide invariants

1. SwiftUI, SQLite, USB scanning, waveform work and diagnostics never schedule
   or authorize a lighting command.
2. Every pending output stage carries source, deck, track-load, plan, phrase
   and transport-generation identity; stale work cannot reach CoreMIDI.
3. Pro DJ Link ingress and all inter-task queues are bounded, observable and
   have an explicit overflow policy.
4. Unknown or ambiguous tracks remain visible but cannot infer an AutoLoop.
5. Existing authored phrases, USB relations and SoundSwitch mappings survive
   upgrades, backup, restore and engine restart.
6. A degraded UI may drop display frames; the show path may not drop or delay a
   valid critical transition silently.
7. Lumi remains fully offline during a show. Network access in a security audit
   is a development/release activity only.

## Epic acceptance

- all local functional and technical gates pass from a clean checkout;
- the one-hour simulator soak records zero missed or duplicate AutoLoop
  transitions, zero stale-generation emissions and no unbounded growth;
- a normal pre-armed phrase transition meets the ADR-0030 p95 CoreMIDI boundary
  target of at most 20 ms; p50, p95, p99 and maximum are retained, never only an
  average;
- forward/backward seek, hotcue, beatjump, pause/resume and atomic master
  handoff cancel obsolete work and select the correct landing phrase;
- library sync and snapshot/UI stress do not materially regress the timing
  distribution against the accepted baseline;
- helper crash, bridge restart, UI quit/relaunch and engine restart recover
  without a false green status or app restart requirement;
- backup and restore are engine-owned, integrity-checked and proven against
  WAL, interrupted staging and incompatible/corrupt input;
- dependency audit has no known vulnerable production dependency accepted
  without a documented exception and expiry;
- physical CDJ-1500X, DJM-V5, SoundSwitch, Control One and DMX evidence is
  captured before `0.4.0-rc-1` is promoted.

## Out of scope

- new DJ hardware families;
- iPhone pairing or remote control;
- custom output-profile builder;
- replacing Beat Link or Carabiner with new protocol implementations;
- large visual redesigns;
- a broad codebase rewrite.

## Tracking and evidence

Each story gets one parent issue/sub-issue relationship in GitHub Projects.
Evidence is stored under a versioned `docs/release/0.4.0-*` record or as a small
machine-readable artifact referenced from the pull request. Raw audio and
copyrighted Rekordbox data are never committed.

## Dev-35 implementation result

Version `0.4.0-dev-35` completes the code hardening slice for this epic:

- a dedicated bounded realtime MIDI execution lane owns Bank and AutoLoop
  deadlines and cancels obsolete generations;
- the engine exposes realtime queue and latency health so a failed lane cannot
  produce a false-green Live status;
- library reads are revisioned and unchanged library/editor state is excluded
  from high-frequency polling;
- SQLite backup and restore are executed by the owning engine with integrity,
  schema, staging and rollback validation;
- the current macOS adapter reconnects to a channel-specific persistent engine
  after UI relaunch without resetting show state;
- checksum, licence, notices and SPDX SBOM are included in the Dev artifact.

The retained evidence is documented in
[`0.4.0-dev-35-show-grade-hardening.md`](../release/0.4.0-dev-35-show-grade-hardening.md).
Promotion of the lifecycle adapter to `SMAppService`, a one-hour soak,
Instruments evidence and the physical CDJ/SoundSwitch/DMX run remain explicit
pre-RC work. They are not silently claimed by the successful Dev gates.

## Dev-36 USB/live resolution correction

Version `0.4.0-dev-36` removes the trusted-media blocker discovered before the
physical run: stable USB sources no longer alias by display name, and GRAY plus
CHRM aliases may jointly resolve a Live track when both agree on its canonical
Lumi identity. Conflicts remain fail-closed. Evidence and migration behavior
are recorded in
[`0.4.0-dev-36-usb-identity-and-live-resolution.md`](../release/0.4.0-dev-36-usb-identity-and-live-resolution.md).

Version `0.4.0-dev-37` separates Live operation-control concurrency from
high-rate deck telemetry. A beat or position update can no longer invalidate a
valid Off → Arm, Arm/Pause → Start, Live → Pause or safety Off transition. The
decision and regression evidence are captured in
[`0.4.0-dev-37-live-operation-controls.md`](../release/0.4.0-dev-37-live-operation-controls.md).

Version `0.4.0-dev-38` completes the current Delta UI slice: Live presentation
uses stable Core Animation clocks, routine polls no longer republish equivalent
state, and an AppKit-owned minimum window size removes the measured macOS
hosting-view layout loop. Realtime MIDI remained isolated and below its budget
in the retained release soak. Evidence is recorded in
[`0.4.0-dev-38-live-rendering-and-output-timing.md`](../release/0.4.0-dev-38-live-rendering-and-output-timing.md).
