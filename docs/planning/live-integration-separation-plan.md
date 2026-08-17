# Live integration separation — implementation plan (one-page)

- Status: **Approved direction; implementation starts after simulator gate**
- Priority: **P0 Critical**
- Release gate: `0.4.0` cannot become RC until every phase below passes
- Governing ADRs: ADR-0031 through ADR-0034

## Outcome

Live Decks must remain current indefinitely: SoundSwitch receives one correct
AutoLoop selection on its musical boundary, Ableton Link follows only the
latest master BPM, and the UI stays acceptably smooth. A slow UI, burst of
CDJ-1500X position packets, library operation or delayed packet may not add
latency to either integration output. Latency must be bounded, observable and
must not grow with run duration.

## Failure being removed

The current design separates the final Link and MIDI providers, but both still
consume facts after one shared Pro DJ Link path. The Java bridge first places
every callback in a 4,096-item FIFO and serializes/flushed them individually.
Rust coalesces continuous position and status only after that FIFO. Physical
CDJ-1500X players add high-rate PrecisePosition traffic and bursts which the
old simulator did not emit. Stale Beat, BPM and display observations can
therefore arrive together and increasingly late. Existing output p95 measured
only the final CoreMIDI dispatch, not source-to-output event age.

## Target flow

```text
Beat Link callbacks
  ├─ critical lane: load/master/play/beat/discontinuity ──> Transport Authority
  │                                                        └─> AutoLoop Executor ─> MIDI
  ├─ tempo mailbox: latest CdjStatus BPM per deck ─────────────> Link Relay ───────> Link
  └─ display mailbox: latest position/status per deck ─────────> Read Model ───────> UI
```

Every arrow is bounded and has an explicit overload policy. A continuous
mailbox replaces its current value; it never queues history. Critical events
retain ordering and either arrive within budget or move the integration to an
explicit fail-closed state. No consumer can wait behind work for another
consumer.

## Delivery phases

### 0 — Representative simulator and measurements

Make `cdj-1500x` the mandatory simulator profile. It emits status at 10 Hz,
PrecisePosition at 50 Hz, exact beats, and deterministic stale-position bursts.
Expose packet counts, cadence, burst count and traffic errors. Add a manual
burst control. First add source timestamp, ingress age and oldest-event age to
the bridge/engine evidence; without those values a green queue-depth metric is
insufficient.

### 1 — Source-side separation

Replace the Java FIFO with two latest-value mailboxes per deck (tempo and
display) plus a small ordered critical queue. Serialize only values that are
still current. Add sequence/timestamp watermarks so an older status can never
overwrite a newer BPM. Rust receives the same three traffic classes and does
not recreate one combined backlog. Saturation of the critical lane degrades
Live Decks and suppresses late lighting rather than replaying history.

### 2 — Independent consumers

Run Transport/AutoLoop, Link and display projection as independent engine
tasks. The AutoLoop task consumes exact beats and confirmed transport epochs;
it never consumes UI state or continuous position history. Link consumes only
the newest selected-master BPM/play fact and preserves its own phase. The UI
receives low-rate immutable anchors and interpolates locally; frames may be
dropped, source events may not.

### 3 — End-to-end cue and transport completion

Finish explicit start/resume, Hot Cue, seek, beatjump, loop-wrap, master
handoff and output-offset epochs. A cue carries source observation time through
its final MIDI receipt. If its freshness/deadline guard fails, cancel and report
it; never emit it seconds late. SoundSwitch receives only sparse Bank/AutoLoop
selection pulses and owns all later progress.

### 4 — Soak and physical release gate

Run deterministic simulator matrices and a one-hour combined soak with UI
foreground/background switches, pitch ramps, bursts, Hot Cues and master
handoffs. Only after this passes, repeat a bounded confirmation set on the two
physical CDJ-1500X players over Wi-Fi and Ethernet.

## Measurable acceptance

- source-to-MIDI normal phrase cue: p95 <= 20 ms, p99 <= 40 ms, zero late
  replay and zero duplicate/wrong cue;
- master BPM: latest stable slider value visible at Link within 150 ms and no
  regression to an older value;
- continuous input: queue age does not trend upward; latest-value mailbox depth
  is at most one per deck/type;
- display: anchor age <= 100 ms during normal traffic; animation may drop frames
  but never changes integration timing;
- one Link peer, zero implicit helper restart and zero SoundSwitch progress
  correction command;
- every metric covers the whole path from Beat Link callback to its consumer,
  not only the final provider call.

## Release rule

Dev builds may demonstrate individual phases. No story is marked Done and no
RC is produced from component-only evidence. Simulator evidence is mandatory
for performance; a final physical run is mandatory for protocol compatibility.
