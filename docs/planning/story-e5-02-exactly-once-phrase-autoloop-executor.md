# Story E5-02: Exactly-once phrase AutoLoop executor

- Status: **Simulator acceptance restored in dev-54; physical soak evidence pending**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-55`
- GitHub tracking: [#118](https://github.com/victorblanco-tech/lumi/issues/118)

## Outcome

Normal sequential playback emits at most one Bank selection and exactly one
AutoLoop selection per phrase execution epoch. SoundSwitch owns all progress
after that selection.

## Scope and acceptance

- replace the predictive/pending/fallback combination with the explicit states
  `Idle -> Scheduled -> BankPrepared -> Triggered -> Completed`;
- identify an execution by deck, track load, playback epoch, plan revision and
  phrase instance;
- never emit progress, seek, correction or periodic retrigger commands;
- start with zero output offset and normal phrase boundaries only;
- deterministic full-track fixtures prove exact expected MIDI sequences and
  zero duplicates under delayed, repeated and reordered observations.

## Dev-51 implementation evidence

- the engine owns a dedicated AutoLoop executor with the states `Idle`,
  `Scheduled`, `BankPrepared`, `Triggered` and `Completed`;
- execution identity contains epoch, deck, track load, plan revision and phrase
  index, and duplicate requests are counted without producing MIDI;
- Start during an already-playing phrase creates one new epoch and reasserts
  exactly the current configured AutoLoop once;
- normal Beat, CdjStatus, PrecisePosition, BPM and UI observations never emit
  progress or correction messages to SoundSwitch;
- a Bank pulse is omitted when the required Bank is already active;
- all engine tests pass, including 63 active engine tests and dedicated
  exactly-once executor tests;
- all 40 Pro DJ Link provider/protocol/supervisor tests pass, including
  regressions for precise-position confirmation after a loop and sparse status
  progress near the exact Beat lane;
- all 52 Live workspace presentation tests pass;
- a physical Player 1 / Wi-Fi run at 155 BPM emitted the current `INTRO BLUE
  PINK` and later `BRIDGE BLUE PINK` selections visibly in SoundSwitch;
- final realtime MIDI evidence recorded 8 requested, 8 completed, 0 duplicate,
  0 late and 0 saturated AutoLoop executions, with 132 microseconds p95 dispatch
  latency;
- a 54.8-second physical loop trace sampled the engine 251 times and observed
  exactly one loop (`beat 192 -> 65`), one transport revision, one position
  discontinuity and zero small backwards steps.

The stored lighting offset is intentionally labelled **Saved** in dev-51. The
exactly-once executor fires on the phrase boundary; scheduled negative or
positive deadlines are E5-03 scope and are not represented as applied yet.

## Dev-52 compatibility correction

A longer live run invalidated the assumption that successful UMP delivery into
CoreMIDI implied continued SoundSwitch consumption. Lumi kept emitting the
correct channel-16 Bank and AutoLoop notes with a 0.1 ms realtime p95 while
SoundSwitch stopped changing loops. Dev-52 keeps the exactly-once executor and
changes only its provider boundary to classic CoreMIDI MIDI 1.0 packet lists.
This correction is a prerequisite for E5-03; it does not reintroduce progress,
seek-correction or periodic AutoLoop output.

## Dev-52 acceptance failure

The classic MIDI 1.0 packet correction made SoundSwitch consume Lumi again, but
the longer physical run still produced increasingly late AutoLoop selections.
The recorded 0.1 ms p95 began only after the cue reached the final realtime MIDI
lane; it excluded age in the Beat Link callback FIFO and shared engine input
pump. Exactly-once therefore remains necessary but is not sufficient.

Completion additionally requires:

- source observation time carried through the MIDI dispatch receipt;
- normal phrase cue p95 <= 20 ms and p99 <= 40 ms end to end;
- stale cues cancelled and reported instead of emitted late;
- no cue latency trend during the one-hour CDJ-1500X simulator profile;
- output unaffected by simultaneous Link updates, display traffic and UI
  foreground/background changes.

## Dev-54 source-isolation evidence

- exact Beats, load/master/play transitions and discontinuities no longer wait
  behind continuous position/display history at the bridge boundary;
- a 50,000-sample display burst collapses to one newest value without losing or
  delaying critical traffic;
- the network acceptance fixture proves one and only one output record after a
  forward seek and one after a backward seek;
- a 300 ms delayed duplicate/stale burst produces zero additional output;
- real headed testing kept AutoLoop execution alive while switching between
  Lumi and SoundSwitch, with diagnostics reporting `realtime p95 0.1 ms`, zero
  late cues and no saturation;
- the 15-second dispatch soak recorded p50 69 µs, p95 10.045 ms, p99 10.060 ms,
  zero saturation; its 28 cancellations are intentional generation replacement,
  not late or duplicate MIDI output.

The simulator and headed acceptance failure from dev-52 is therefore resolved.
The one-hour physical source-to-MIDI soak and its final p95/p99 evidence remain
E5-04 release evidence.
