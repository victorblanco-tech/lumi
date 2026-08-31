# Story E9-00: Physical master-tempo propagation latency

- Status: **Planned — first delivery item for 0.6.0**
- Priority: **P0 performance baseline**
- Target: `0.6.0-dev-1`
- Components: Pro DJ Link, Engine, Ableton Link, Diagnostics

## User outcome

Moving the BPM slider on the playing master Player reaches SoundSwitch through
Ableton Link as quickly and consistently as the accepted pre-0.6 live path.
Lumi can prove where time is spent without coupling the Link Relay to lighting,
planning or UI work.

## Reason

Physical acceptance of `0.5.2-dev-9` confirmed that all integrations remain
functional, but slider changes felt slightly slower than before the runtime
refactor. This is not a `0.5.2` release blocker. It is the first `0.6.0` item so
the companion work starts from a measured realtime baseline.

The observation is not yet evidence that the refactor caused a regression.
Display refresh delay, Pro DJ Link delivery, bridge classification, engine
mailbox consumption, Link-helper communication and SoundSwitch presentation
must be measured separately.

## Scope

- record monotonic, correlation-safe timestamps at these existing boundaries:
  1. CDJ status received by the Pro DJ Link bridge;
  2. latest-tempo mailbox publication and Rust ingestion;
  3. authoritative master-tempo selection;
  4. Link Relay request and helper application;
- expose bounded counters/histograms instead of synchronous hot-path logging;
- measure p50, p95, p99 and maximum age for stable, ramped and rapidly changed
  slider input;
- distinguish actual Link publication latency from macOS and SoundSwitch UI
  refresh latency;
- compare the current runtime against retained simulator evidence and, where a
  like-for-like build is practical, the accepted pre-refactor baseline;
- optimize only the boundary proven to contribute material delay;
- retain exactly one Ableton Link peer and latest-value semantics throughout.

## Acceptance criteria

- every final physical slider value converges in the Link session without
  regression to an older BPM;
- physical CDJ status receive to Link-helper application has a measured p95 at
  or below 150 ms, with a lower target adopted if the baseline demonstrates it
  is safely achievable;
- a rapid slider sweep does not build FIFO history: intermediate tempo values
  may be coalesced, but the newest value wins;
- UI foregrounding, waveform rendering, USB work and Light Plan compilation do
  not measurably change the Link-lane distribution;
- AutoLoop MIDI timing and exactly-once behavior remain unchanged during the
  combined test;
- instrumentation is bounded, disabled or cheaply aggregated in normal use,
  and performs no synchronous disk I/O on a realtime lane;
- simulator regression, headed macOS validation and a physical Player plus
  SoundSwitch acceptance transcript are recorded before completion.

## Out of scope

- changing Ableton Link into a lighting timeline;
- continuous correction of SoundSwitch AutoLoops;
- making the future iPhone client part of the realtime path;
- treating visual waveform smoothness as proof of integration latency.

