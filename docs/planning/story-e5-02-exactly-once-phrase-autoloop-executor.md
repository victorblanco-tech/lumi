# Story E5-02: Exactly-once phrase AutoLoop executor

- Status: **Done; scheduled output offset and explicit hotcue epochs continue in E5-03**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-51`
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
