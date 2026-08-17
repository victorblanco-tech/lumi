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

## Implementation note — `0.4.0-dev-35`

The first implementation of this decision is complete behind the existing
ports. `RealtimeMidiController` owns a bounded command channel, the CoreMIDI
provider and deadline dispatch on one dedicated thread. Engine commands publish
immutable scheduled work and generation invalidation; no snapshot, SQLite, USB
or SwiftUI work runs on that thread. Its bounded health and latency histogram
are exposed to Diagnostics and Live readiness.

Library state now carries a monotonic revision, allowing lean transport polling
without rebuilding or decoding unchanged library/editor data. Backup and
restore moved to the owning SQLite connection and use online backup, validation,
atomic staging and rollback.

The macOS lifecycle is deliberately delivered in two steps. Dev-35 implements
the reversible reconnect adapter: a channel-specific persistent engine accepts
sequential authenticated UI sessions and survives UI relaunch. Promotion to a
login-capable `SMAppService` remains a pre-RC lifecycle task; the adapter does
not claim automatic restart after an engine-process crash.

## Implementation note — `0.4.0-dev-38`

Live presentation follows the same isolation boundary. Waveform, phrase and
AutoLoop-plan interpolation use Core Animation from an immutable visual-clock
anchor. Routine 4 Hz deck polls do not republish an equivalent SwiftUI tree;
only session, transport, phrase, plan, input or output changes do. The model
still refreshes authoritative state immediately after seeks, Hot Cues, loads,
master handoffs and output-health changes.

Application minimum sizing is owned once by the AppKit window instead of by
nested SwiftUI root frames. This avoids a hosting-view size-constraint feedback
loop observed on macOS 26 without weakening the minimum supported layout. None
of these presentation clocks can schedule, cancel or authorize MIDI output.

## Implementation note — `0.4.0-dev-39`

Physical CDJ testing exposed two gaps in the first implementation. First, an
AutoLoop was only delegated to the realtime lane during approximately the last
beat before its phrase. Losing that exact observation meant the domain learned
about the phrase later and could emit seconds late even though the plan had
been ready for minutes. Direct Pro DJ Link now delegates the immutable Bank and
AutoLoop deadline up to sixteen beats ahead. The lane owns that absolute
deadline; plan, track-load, transport and Master generations still invalidate
obsolete work immediately.

Second, a new UI process could attach to a persistent engine from a different
Dev build. Service identity therefore includes release channel version, build,
resolved executable and executable SHA-256. Replacement uses graceful SIGTERM,
which the Rust service handles inside both idle and connected loops so Pro DJ
Link and Ableton Link child supervisors are dropped before the process exits.

The connected-deck visual clock is monotonic within one transport revision.
Delayed or out-of-order status polls cannot rewind it; a real seek, Hot Cue,
beatjump, load or Master discontinuity changes the revision and remains
immediate. AppKit root-host sizing is configured from `viewDidMoveToWindow`,
after the real `NSHostingView` exists, rather than racing window creation from
SwiftUI `onAppear`.

## Implementation correction — `0.4.0-dev-40`

Physical SoundSwitch testing showed that deadline ownership also requires a
strict distinction between **scheduled** and **emitted**. A Pro DJ Link status
or phrase event may arrive on either side of the already prepared deadline.
Entering the target phrase therefore keeps that deadline alive; only an actual
realtime-lane dispatch receipt suppresses the boundary fallback. Preparing the
following phrase reuses the current transport generation and cannot cancel a
due pulse. Generations advance only when transport context or a changed-tempo
prediction invalidates queued work.

After a start, Hot Cue or beatjump with no safely selected Bank, the realtime
lane emits the AutoLoop exactly after the Bank's 50 ms settle deadline. It does
not wait for the next deck beat. This is the first safe boundary available to
the output adapter and is explicitly counted as late/preparation fallback.

The release also reverses the temporary persistent-UI adapter decision for the
current desktop product. Until `SMAppService` provides a separately managed
service lifecycle, the Lumi macOS process owns the engine, Pro DJ Link bridge
and Carabiner helper as one process tree. Last-window close and Quit perform a
graceful stop with a bounded forced fallback; an unexpected client disconnect
causes the app-owned engine to tear down. Link anchors queued before shutdown
cannot relaunch a helper after teardown begins.

The managed Link helper must not daemonize. A daemonized Carabiner forks away
from the engine's `Child` handle and survives a correct Rust drop path. Lumi
therefore owns the foreground helper process and waits for it during teardown.
Likewise, direct Pro DJ Link freshness tolerates eight missing beat packets
(bounded to 3–8 seconds): the realtime lighting lane continues its immutable
four-bar-ahead deadline, while a sustained outage still holds Link fail-closed.
The owner also terminates its tracked helper before joining the timing worker;
this closes in-flight loopback I/O and prevents the app supervisor's bounded
fallback from orphaning the child. An externally supplied helper is not owned
and is therefore never killed by Lumi.

## Implementation correction — `0.4.0-dev-41`

Direct Pro DJ Link Beat packets remain authoritative for show-critical
AutoLoop deadlines, but their local receive timestamps are not a stable clock
discipline signal. Network and scheduler jitter must never continuously steer
the shared Ableton Link phase. Doing so caused SoundSwitch AutoLoop progress to
move backwards and forwards even while the CDJ transport itself was stable.

The timing-output adapter therefore owns a monotonic Link projection after the
first valid anchor. Continuous beat observations may update BPM while
preserving that projection, and their phase error remains diagnostic only.
Only a new timing generation, an explicit track/master/seek discontinuity, or
a stopped-to-running transport transition may force one new phase anchor.
AutoLoop MIDI scheduling remains driven by the exact Pro DJ Link beat and is
not coupled to Link's presentation or phase-correction policy.

Each app-owned engine also selects an isolated loopback control port for its
managed Carabiner process. The TCP port is not part of Ableton Link discovery;
it only controls the local helper. Dev, RC and Prod can therefore coexist, and
a new engine cannot silently attach to an orphaned helper whose `Child` handle
belongs to an earlier process. Externally configured adapters remain possible
through an explicit configuration boundary rather than accidental port reuse.
Port selection stays inside `20000...32767`, because Carabiner rejects larger
ephemeral ports even though macOS may assign them when binding port zero.

## Implementation correction — `0.4.0-dev-42`

The input-side transport generation now has the same monotonicity guarantee as
the Link adapter. Playing deck-status positions are classified against elapsed
time and effective BPM rather than a fixed beat delta. A delayed normal status
cannot create a false seek, invalidate a prepared realtime item or authorize a
Link re-anchor. Out-of-order status cannot rewind the precise-beat baseline.
Provider-level tests exercise both cases through the complete bridge decoder
and domain-observation boundary.

The direct bridge no longer starts Beat Link media/signature discovery. Trusted
USB mirrors remain the sole metadata, waveform, beat-grid, cue and phrase
source, while the realtime bridge consumes only device, status and precise
Beat facts. This removes an unnecessary retry fault domain from occupied
physical networks without changing the show-critical input contract.

## Implementation correction — `0.4.0-dev-43`

Physical process sampling separated output-lane health from a third-party MIDI
device-list failure. Lumi sustained bounded Pro DJ Link ingress and sub-budget
CoreMIDI dispatch while SoundSwitch's main thread was blocked joining its
Control One/JLC1 storage worker during a MIDI device reset. Starting
SoundSwitch after Lumi's endpoints were stable removed the hang for the same
Link and AutoLoop workload.

The channel engine therefore retains virtual MIDI endpoint ownership across UI
sessions. Disconnect is an explicit safety boundary: operation becomes Off,
pending output is invalidated, local clock stops and Link leaves before the UI
transport closes. Reattachment reuses the exact engine process and cannot
duplicate endpoints. An incompatible build is still replaced rather than
attached, preserving build-exact process ownership.

## Implementation correction — `0.4.0-dev-44`

Physical Hot Cue testing disproved the earlier assumption that a Pro DJ Link
Beat packet is an exact absolute-position observation. It contains a musical
beat and bar-relative position, but may arrive before the asynchronous deck
status that reports the new absolute track position. Combining both streams
could therefore authorize a valid future phrase from the old position after a
Hot Cue landed in Intro.

For modern players, Beat Link's `PrecisePosition` callback is now the only
authority for connected-deck playback position, seek detection, phrase
selection and automatic lighting output. Its playback milliseconds are mapped
through Lumi's trusted local Rekordbox beat grid. Beat packets remain the
low-latency timing input for Ableton Link, while playing deck status may update
tempo without changing Link phase. Neither can authorize a phrase.

Prediction is retained without allowing stale MIDI to escape: future Bank and
AutoLoop work is stored as guarded deadlines, not provider commands. At each
release deadline the source, deck, track load and transport generation must
still match and exact position authority must be no older than 250 ms. Missing
or stale authority cancels the item fail-closed. A precise-position
discontinuity invalidates every older queued generation before the landing
phrase is evaluated.

Precise-position continuity is intentionally asymmetric. A UDP/callback sample
that still advances but arrives behind its wall-clock projection is ordinary
receive jitter and may not create a timing generation. A materially backward
position or a forward position ahead of elapsed time remains an explicit
Hot Cue, loop or beat-jump discontinuity. This prevents late network delivery
from continuously re-anchoring Link while preserving fail-closed cancellation
for real transport changes.

## Implementation correction — `0.4.0-dev-45`

Physical CDJ-1500X playback exposed that an asymmetric threshold is still not
enough: Beat Link explicitly documents that high-frequency precise-position
packets may contain too much jitter to discipline Ableton Link directly. An
isolated packet may therefore never create a transport epoch. Lumi now requires
three consecutive samples that follow one coherent new position trajectory
before accepting a Hot Cue, seek, beatjump or loop wrap. Interleaved packets
from the prior trajectory reset confirmation. Once confirmed, the old output
generation is cancelled before the landing phrase is reduced, preserving the
existing fail-closed output barrier with about 60 ms confirmation latency.

macOS application activation is a presentation-only event. Core Animation may
pause an occluded window's layer clock; when Lumi becomes active, waveform and
AutoLoop-plan layers restart from the current app-side visual clock. This path
may not issue an engine command, alter transport generation, re-anchor Link or
touch MIDI. UI lifecycle and show timing are therefore mechanically separated.
