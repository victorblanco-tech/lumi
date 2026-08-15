# ADR-0031: Show-critical runtime isolation and local quality gates

- Status: **Accepted**
- Date: **2026-08-15**
- Refines: ADR-0003, ADR-0029 and ADR-0030

## Context

Lumi now has a working Local Playback and direct Pro DJ Link path to managed
Ableton Link plus SoundSwitch AutoLoop MIDI. The current engine advances direct
input on a 20 ms interval, but that interval shares an asynchronous task with
synchronous application commands and full snapshot construction. USB sync and
library queries can perform substantial SQLite and filesystem work. SwiftUI
polls a broad snapshot at 250 ms. Direct Pro DJ Link ingress also reaches Rust
through an unbounded process-message channel.

This is adequate for proving the concept, but it does not guarantee the most
important product property: a sparse AutoLoop selection must reach CoreMIDI on
the intended musical boundary regardless of UI and library load.

The accepted service ADR also differs from the current child-process lifecycle.
The UI can terminate the engine and can start backup/restore work before the
process has been observed to exit. A local Apple test can meanwhile collide
with MIDI endpoints owned by an installed Lumi process and report a product
failure that is actually an invalid test environment.

## Decision

### One show-critical execution lane

Lumi will use one engine-owned, provider-neutral realtime lane for musical
deadlines and lighting dispatch. It consumes immutable observations from Local
Playback or Pro DJ Link and emits through the lighting-output port. It does not
execute SQLite, USB, IPC request handling, full snapshot projection, logging or
UI work.

The lane is deadline/event driven. A periodic timer may provide watchdog and
freshness checks, but a polling tick is not the authority for an exact beat or
phrase boundary.

Every scheduled item includes source, deck, track-load, plan, phrase and
transport-generation identity. Discontinuities atomically invalidate obsolete
work before it can reach output.

### Bounded communication

Every cross-thread/process queue is bounded and has a type-aware overload
policy. Continuous position and tempo observations may be coalesced to the
newest state. Track loads, master changes, exact beats, discontinuities,
disconnects and operation-state changes are critical and cannot be silently
dropped. Capacity, high-water, coalescing, drops and critical saturation are
observable without unbounded logging.

### Separate workers and read models

Library and USB commands execute on a non-realtime worker and return revisioned
results. UI consumers request or receive topic-specific revisioned read models.
Immutable waveforms, beat grids, phrases, hot cues and plans are cached instead
of reconstructed on every transport poll.

The macOS UI is a client of the reconnectable service defined by ADR-0003. The
engine owns SQLite backup and restore defined by ADR-0029 and confirms graceful
shutdown before service replacement.

### Local evidence is a release input

Lumi uses layered local gates:

1. functional regression: deterministic domain, engine and presentation
   behavior;
2. technical regression: lint, fault/recovery, queue and release performance;
3. security: RustSec and OSV dependency audit plus boundary checks;
4. full: complete repository and Apple bundle verification;
5. show/lab: simulator, real helper, soak and physical hardware evidence.

A gate must fail when its prerequisite is absent. It may not silently skip and
claim evidence. GitHub Actions remains a secondary release confirmation and is
not required for every Dev iteration.

## Performance decision

The release-critical measurement point for AutoLoops is the CoreMIDI boundary,
not SwiftUI animation and not Ableton Link peer display. Normal pre-armed phrase
changes target p95 <= 20 ms as established by ADR-0030. Every performance run
also records p50, p99, maximum, missed, duplicate, stale-cancelled and fallback
counts. Averages alone are insufficient.

UI display interpolation is allowed to be visually behind authoritative input
by a bounded amount or to drop frames. It may never feed the output scheduler.

## Security decision

The desktop session token is scoped to UI-to-engine IPC. Supervised Java and
Link helpers receive an allowlisted environment and never inherit that token.
Public logs contain bounded, redacted diagnostics. Simulator remote control is
development-only. A future iPhone client requires a separate pairing protocol
and scoped credentials.

## Consequences

- more explicit tasks, queues and revisions increase internal structure;
- command results that were previously synchronous become asynchronous job
  state in the protocol;
- full snapshots remain available for bootstrap/recovery but are no longer the
  high-frequency update mechanism;
- service migration and backup are safer but require lifecycle integration
  tests;
- regression gates take local time, but avoid consuming normal GitHub Actions
  minutes and make evidence reproducible before a commit is promoted;
- existing large files are split incrementally behind characterization tests,
  avoiding a risky rewrite.

## Rejected alternatives

- **Only optimize SwiftUI:** cannot protect CoreMIDI deadlines from SQLite or
  ingress contention.
- **Increase the polling frequency:** increases contention and still does not
  create an exact event boundary.
- **Assign OS thread priority before isolating work:** priority cannot make a
  blocking SQLite or serialization path safe.
- **Rewrite the engine:** discards working behavior and creates unnecessary
  regression risk.
- **Rely only on CI:** costly, slower, hardware-blind and currently less
  deterministic than a controlled local Mac/lab gate.
