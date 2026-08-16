# Story E4-03B: Realtime AutoLoop execution lane

- Status: **Implementation complete — one-hour and physical RC evidence pending**
- Priority: **P0 Critical**
- Effort: **8**
- Components: Engine, Deck sources, MIDI & SoundSwitch
- GitHub tracking: [#101](https://github.com/victorblanco-tech/lumi/issues/101)

## User outcome

As a performing DJ, every configured AutoLoop starts on the intended musical
boundary even when I seek, use a hotcue, switch Master, browse the library or
when the UI is busy.

## Scope

### B1 — Isolated execution

- move musical-deadline calculation and due-output dispatch to a dedicated
  engine-owned lane;
- replace the polling tick as the scheduling authority with explicit deadlines
  and exact source observations;
- keep CoreMIDI emission non-blocking and independent from command handling,
  snapshots, SQLite and rendering;
- preserve one provider-neutral path for Local Playback and Pro DJ Link.

### B2 — Bounded ingress and work queues

- replace unbounded Pro DJ Link ingress with a bounded message-aware queue;
- coalesce continuous position/tempo observations but never overwrite a newer
  track load, transport generation, exact beat, master change or disconnect;
- expose capacity, high-water mark, coalesced, dropped and critical-saturation
  counters;
- fail closed and diagnose when critical ingress cannot be preserved.

### B3 — Generation-safe scheduler

- identify every Bank and AutoLoop stage by deck, source, track load, plan
  revision, phrase and transport generation;
- cancel stale work on load, seek, hotcue, beatjump, master handoff, plan edit,
  Pause or Off;
- apply an offset change only to the next safe scheduled boundary;
- pre-arm Bank without sleeping and emit AutoLoop at the exact eligible beat;
- if a Bank cannot settle after a discontinuity, emit on the first safe next
  beat and count the fallback.

### B4 — Stress and soak evidence

- run deterministic bursts, delayed packets, gaps, duplicates and bridge
  restarts;
- run USB/library work and aggressive snapshot/UI polling during a complete
  show fixture;
- add a configurable-duration soak, with one hour required for RC evidence;
- retain timing histograms and correctness counters in a bounded artifact.

## Acceptance criteria

- no valid cue is missed or duplicated in deterministic sequences;
- no stale generation reaches the lighting provider;
- normal pre-armed transitions meet p95 <= 20 ms at the CoreMIDI boundary;
- p99 and maximum are recorded and reviewed; averages alone are rejected;
- input bursts cannot create unbounded memory or starvation;
- snapshot polling at its maximum supported rate and a 10k-track sync do not
  move the p95 outside the accepted budget;
- hotcue/beatjump lands on the correct current phrase on that beat when armed,
  or exactly the first safe next beat when Bank settling is required;
- lighting readiness is never green while its realtime lane or output provider
  is failed/stale;
- Local Playback regression tests remain bit-for-bit deterministic.

## Implementation rule

Refactor behind the existing deck-source, timing-output and lighting-output
ports. Do not make SwiftUI aware of scheduler internals and do not couple the
realtime lane to SoundSwitch-specific domain types beyond the output profile.

## Verification

Implemented in `0.4.0-dev-34`:

- Java stdout is decoded off the engine session task and enters a bounded
  512-message, type-aware queue;
- continuous deck status/metadata/signature updates are coalescible, while
  exact beats, source lifecycle, discovery and errors are critical and ordered;
- all-critical saturation terminates the bridge path explicitly instead of
  growing memory or dropping a musical boundary;
- queue capacity, depth, high-water, coalesced count and critical saturation
  are available in engine snapshots and macOS Diagnostics;
- a release-mode burst of 50,000 continuous updates completed in 11.10 ms with
  depth 1, high-water 1 and 49,999 safe coalesces;
- Live polling sends a lean snapshot on three of four 250 ms polls, keeping
  library projection away from the high-frequency deck presentation path.

Completed in `0.4.0-dev-35`:

- a dedicated thread owns the MIDI provider, scheduled deadlines and bounded
  command channel; engine snapshot, SQLite and SwiftUI work cannot execute on
  that lane;
- deadline items carry a generation and cancellation invalidates obsolete work
  after discontinuities and operation-state changes;
- predictive Pro DJ Link and exact-beat fallback scheduling use the same
  provider-neutral lane as Local Playback;
- Live and Diagnostics expose capacity, depth, high-water mark, scheduled,
  emitted, cancelled, saturation and p50/p95/p99/max latency;
- the configurable 60-second Dev soak scheduled 2,127 items, emitted 2,016,
  deliberately cancelled 111 stale items, saturated zero times and measured
  p95 10.032 ms with a 10.108 ms maximum.

`./scripts/verify-rc-soak.sh` rejects durations below one hour. That one-hour
run and the physical CDJ/SoundSwitch/DMX timing capture are required before RC,
but do not block completion of the implementation.

### Dev-39 physical-CDJ correction

A prepared direct Pro DJ Link transition is now placed on the isolated MIDI
deadline lane up to sixteen beats before the phrase, instead of relying on the
final beat packet. A deterministic regression accepts beat 16 and rejects beat
17, while existing generation tests protect seeks, Hot Cues, Master handoffs
and plan changes. Orange Live warnings identify the exact provider whose timing
or output health is degraded. Evidence is recorded in
[`0.4.0-dev-39-prolink-autoloop-stability.md`](../release/0.4.0-dev-39-prolink-autoloop-stability.md).

### Dev-40 boundary-race and lifecycle correction

The physical loop soak identified three timing races not represented by the
original lane-only latency test. Dev-40 now verifies that phrase entry does not
cancel its prepared deadline, that preparing the next phrase in the same
transport generation preserves the due pulse, and that a start/Hot Cue without
a prepared Bank uses an exact 50 ms lane deadline rather than the next beat.
Stable-tempo packet jitter cannot churn generations; changed BPM still replaces
a meaningfully drifted prediction.

Diagnostics retain the action kind and address, scheduling lead, last dispatch
lateness and number of dispatches above 20 ms. This separates delay before lane
submission from CoreMIDI dispatch delay during physical acceptance testing.
The accompanying app lifecycle test must also prove that no engine, bridge,
Carabiner or Ableton Link peer remains after Quit.

The first physical Dev-40 soak then exposed two environment-only lifecycle
conditions. Short runs of missing UDP beats no longer toggle Link fail-closed:
the stale window is eight beats, clamped to 3–8 seconds, while the already
prepared four-bar AutoLoop deadline remains authoritative. The managed
Carabiner helper also runs in the foreground so its actual process remains a
child of the engine and is killed and waited during teardown. Live and
Diagnostics expose last dispatch lateness and the cumulative count above the
20 ms budget for hardware acceptance.

The owned child handle is shared with the output object's drop path. Shutdown
terminates that exact process before joining the worker, so an in-flight helper
socket exchange cannot outlive the macOS graceful-exit budget. The real bundled
Carabiner lifecycle test must leave no process on its isolated test port.

### Dev-41 monotonic Link correction

Physical SoundSwitch acceptance exposed that trigger latency and continuous
Link phase are separate concerns. Phrase and AutoLoop triggers were timely,
while 575 hard and 238 soft Link corrections over 843 applied anchors made the
active SoundSwitch AutoLoop scrub backwards and forwards. The maximum observed
correction was 768 ms.

Dev-41 makes continuous Pro DJ Link phase error observational only. The Link
timeline is changed once for initial start/resume, a Hot Cue or seek, track
load, or Master handoff; stable playback and pitch changes preserve monotonic
phase. A local fake-Carabiner regression sends conflicting continuous beat
phases and asserts that no additional `force-beat-at-time` or
`request-beat-at-time` command is emitted.

### Dev-42 false-seek and helper-lifecycle correction

The remaining intermittent scrub was traced one boundary earlier: delayed
deck-status progress could exceed the old fixed two-beat threshold and falsely
advance the transport generation. Dev-42 validates that progress using elapsed
monotonic time and effective BPM. While playing, only precise Beat packets
publish beat boundaries; a late status packet cannot rewind their canonical
position. Provider-level regressions prove that delayed normal progress emits
neither a seek nor a Link anchor, while real Hot Cues, seeks and loop wraps
remain discontinuities.

The real bundled-Carabiner regression now also drops the timing provider
without calling Stop and waits for its isolated control port to close. Together
with installed-app normal-Quit and forced-UI-exit checks, this is the acceptance
evidence that SoundSwitch cannot retain a Lumi-owned ghost Link session.
