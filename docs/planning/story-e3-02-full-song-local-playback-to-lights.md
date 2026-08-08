# E3-02 – Full-song Local Playback to SoundSwitch and physical lights

Status: **Implemented; initial physical run passed, stability confirmation pending**

Target milestone: **0.3.0 – SoundSwitch Live MVP**

## Outcome

Lumi plays one real imported track from beginning to end in Local Playback and
automatically executes its materialized phrase plan through SoundSwitch to the
connected fixtures. No Pro DJ Link hardware is required. SoundSwitch remains
the owner of AutoLoops, fixture control and DMX output.

```text
Local audio + exact beat grid
          │
          ▼
Lumi phrase/plan scheduler ──► Bank + AutoLoop MIDI ──► SoundSwitch ──► DMX
          │
          └──────── local tempo/beat clock ───────────► SoundSwitch sync
```

The command path already exists and has been proven physically for an explicit
bank/AutoLoop trigger. This story hardens automatic phrase-boundary execution
and supplies the timing source that Local Playback lacks without DJ decks.

## Gate 0 – SoundSwitch timing compatibility

The implementation publishes a dedicated `Lumi Clock` CoreMIDI source while
preserving `Lumi Virtual MIDI` for bank/AutoLoop commands. This keeps timing
traffic isolated from learned command notes and lets SoundSwitch select the
appropriate endpoint for each function. ADR-0022 records the decision.

The spike must prove:

- SoundSwitch receives 24 PPQN MIDI Clock plus Start/Continue/Stop from Local
  Playback;
- displayed/effective BPM follows the locally played track;
- commands and clock can coexist without dropped or mislearned notes;
- pause/resume and restart from track start do not create a false downbeat;
- measured drift at phrase boundaries remains at most one eighth of a beat over
  one complete test track.

The remaining physical gate records a short compatibility transcript and DMX
evidence from the real SoundSwitch installation.

## Build slices

### 1. Local playback timing bridge

- derive timing only from the authoritative local audio/beatgrid state;
- publish MIDI Clock and transport messages outside SwiftUI;
- make clock availability, BPM, transport and last error visible in Lighting
  Output diagnostics;
- publish no clock or transport while the relevant output is Off;
- recover the affected command source automatically without coupling clock and
  lighting-command failures; explicit Stop remains authoritative.

Implementation status: **complete in code and local automated verification**.
The Rust output worker owns a dedicated 24 PPQN scheduler, emits MIDI Start,
Continue, Stop and Song Position messages, derives BPM and phase from the exact
imported beatgrid, and exposes separate clock diagnostics to SwiftUI.

### 2. Automatic full-song execution

- resolve every upcoming Lumi phrase to its materialized SoundSwitch bank and
  AutoLoop slot before playback starts;
- in Armed, show the complete executable plan but emit no lighting command;
- in Live, emit exactly one bank-settle-AutoLoop sequence at each crossed phrase
  boundary;
- never replay the active cue because of UI polling, a status refresh or a
  duplicate position observation;
- preserve Control One coexistence: manual input can override the current look
  and Lumi may reclaim control only at the next valid phrase boundary;
- hold the last look at track end; do not invent a blackout.

Implementation status: **complete in code and local automated verification**.
Playback state is applied before phrase activation, stopped/Armed playback is
output-silent, and identical phrase cues are deduplicated by deck, track-load,
phrase, cue and plan revision.

### 3. Discontinuity and fail-closed behavior

- pause stops clock advancement and produces no phantom phrase transition;
- resume does not duplicate the active cue;
- a Live seek never emits a burst of skipped cues: Lumi establishes at most the
  destination phrase target once and records the discontinuity;
- missing MIDI, an unmapped phrase, stale plan or unavailable timing holds the
  current safe look, keeps local audio usable and surfaces a precise diagnostic;
- Off and Paused suppress automatic output immediately.

Implementation status: **complete for the local product path**. A paused seek
updates the visible phrase but emits nothing; resume establishes the destination
phrase once. Position discontinuities are detected against monotonic elapsed
time and rephase the clock rather than replaying skipped phrase cues.

### 4. Physical acceptance run

Implementation status: **initial end-to-end light run passed on 2026-08-08;
repeat stability/timing run pending after hardening**.

- choose one ready imported track with a complete beatgrid, Lumi phrase timeline
  and mapped AutoLoop for each phrase used in the run;
- load it into Local Playback, inspect the plan in Armed, switch to Live and
  start from the beginning;
- observe one uninterrupted complete-song run through SoundSwitch and physical
  DMX fixtures;
- repeat a short run containing pause/resume and one manual Control One override;
- retain an event transcript and screen/video evidence with expected versus
  observed bank, AutoLoop, phrase boundary and clock state.

### Configuration-continuity regression

Resolved on 2026-08-08 before the physical run:

- Live planning options and generated cues now consume the persisted Autoloop
  catalog Theme/Bank names instead of the original demo planner labels;
- the exact user-edited Lumi timeline revision and its phrase-role assignments
  are refreshed into the Local Playback browser and loaded atomically;
- `Off` closes the output gate and clears execution deduplication, but retains
  the prepared leader plan, so `Off → Arm → Start` no longer waits for a deck
  or master change before automatic MIDI can resume;
- the production database was inspected read-only and confirmed to retain the
  user's four renamed Banks, custom phrase roles, Bank 1 button mappings and
  edited track timeline. No configuration recovery or re-entry is required.

### SoundSwitch slot-coordinate regression

Resolved after the first physical phrase-boundary run exposed deterministic
but incorrect AutoLoops:

- Banks & AutoLoops had stored four-column positions row-major, while MIDI Learn
  and SoundSwitch number 1–8 vertically before continuing with 9–32;
- catalog defaults version 4 transposes the existing stable mapping IDs once,
  preserving user names and Phrase Types without manual re-entry;
- generated placeholder mappings no longer count as executable physical slots;
- automatic planning is restricted to a Theme that has complete real mappings
  for the loaded track, so an unfinished Bank cannot enter rotation;
- the mapping view now uses the same 1,9,17,25 first visual row as Learn and the
  Test Controller.

## Acceptance criteria

- No bank/AutoLoop command is emitted before both Live and actual playback.
- Every crossed phrase boundary emits exactly its planned bank and AutoLoop,
  once and in the validated bank-settle order.
- SoundSwitch stays beat/bar aligned for the full song within the Gate 0 drift
  threshold.
- Waveform, phrase state, AutoLoop Plan and emitted command identify the same
  active phrase throughout the run.
- Pause/resume, one seek safety check, track end and one manual override behave
  as specified without a burst, duplicate, hang or unsolicited blackout.
- Endpoint loss fails closed and is visible in Diagnostics; explicit recovery
  or automatic command-source recovery restores operation without restarting
  the full Lumi application.
- Rust scheduling/MIDI tests, real process integration, Swift package tests and
  the native macOS build pass locally without GitHub Actions.
- The user confirms visible physical light changes for one complete real track.

## Boundaries

- No Pro DJ Link or Beat Link Trigger is required for this acceptance path.
- Local Playback is an end-to-end rehearsal source, not the eventual production
  deck source for a live DJ performance.
- Lumi does not parse or edit SoundSwitch projects, AutoLoops or fixture data.
- SoundSwitch owns DMX output; Control One remains outside the Lumi domain.
- Output Profile Builder, iPhone control and generic non-SoundSwitch devices
  remain separate stories.

## Local verification record

Completed on 2026-08-08:

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`, including CoreMIDI and the real engine process;
- all macOS Swift package tests with warnings as errors;
- Swift-to-Rust process integration proving both `Lumi Virtual MIDI` and
  `Lumi Clock`, clock ticks only during `LIVE + Play`, and pause suppression;
- native unsigned arm64 macOS app build;
- repository architecture checks and 22-item visual evidence gate.

The subsequent configuration-continuity regression additionally passed the
planner/domain suites, all 41 engine unit tests, the real engine process test,
Rust Clippy with warnings denied, and a fresh native app build. Desktop
validation loaded timeline revision 29 and showed all 16 user-edited phrases
with their persisted Bank 1 Autoloop names; the plan remained visible after
`Off`.

These tests prove the application route and safety semantics. They do not
replace the remaining physical SoundSwitch, Control One and DMX fixture run.

### Post-run lifecycle and timing hardening

The first physical complete-path run proved automatic phrase-driven light
output, then exposed navigation-related output loss and variable late triggers.
ADR-0023 records the remediation:

- `Lumi Virtual MIDI` auto-publishes, self-recovers and no longer stops when the
  separate `Lumi Clock` route reports an error;
- Tech Ready now includes command-MIDI and clock health;
- Local Playback transport cadence is 10 ms and is flushed ahead of queued UI
  work;
- every phrase reasserts its bank 50 ms before the AutoLoop target, preserving
  Control One coexistence without adding 50 ms to the visible light change;
- a persisted signed -250…+250 ms timing offset is available in Settings and as
  a compact Live adjustment.

The hardening passed the complete local Apple gate on 2026-08-08: the Rust
workspace, real engine-process integration, all Swift packages, native arm64
app build, architecture checks and visual-evidence gate are green. A separate
preference regression covers neutral defaults, signed persistence and safe
clamping. Desktop validation confirmed automatic publication of `Lumi Virtual
MIDI` and `Lumi Clock` in Tech without sending a light cue. A repeat physical
full-song run remains the acceptance check for end-to-end timing and the
Live → Library → Live continuity fix.
