# E3-02 – Full-song Local Playback to SoundSwitch and physical lights

Status: **Ready for build and physical acceptance**

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

Before expanding implementation, verify on the real installation whether
SoundSwitch can consume MIDI Clock and Lumi bank/AutoLoop commands from the
same virtual CoreMIDI source. If not, Lumi exposes a separate `Lumi Clock`
source while preserving the existing `Lumi Virtual MIDI` command source.

The spike must prove:

- SoundSwitch receives 24 PPQN MIDI Clock plus Start/Continue/Stop from Local
  Playback;
- displayed/effective BPM follows the locally played track;
- commands and clock can coexist without dropped or mislearned notes;
- pause/resume and restart from track start do not create a false downbeat;
- measured drift at phrase boundaries remains at most one eighth of a beat over
  one complete test track.

The result is recorded as a short compatibility transcript. A separate virtual
clock source is the default fallback, not a failure of the story.

## Build slices

### 1. Local playback timing bridge

- derive timing only from the authoritative local audio/beatgrid state;
- publish MIDI Clock and transport messages outside SwiftUI;
- make clock availability, BPM, transport and last error visible in Lighting
  Output diagnostics;
- publish no clock or transport while the relevant output is Off;
- recover only through an explicit reconnect/restart action after endpoint loss.

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

### 3. Discontinuity and fail-closed behavior

- pause stops clock advancement and produces no phantom phrase transition;
- resume does not duplicate the active cue;
- a Live seek never emits a burst of skipped cues: Lumi establishes at most the
  destination phrase target once and records the discontinuity;
- missing MIDI, an unmapped phrase, stale plan or unavailable timing holds the
  current safe look, keeps local audio usable and surfaces a precise diagnostic;
- Off and Paused suppress automatic output immediately.

### 4. Physical acceptance run

- choose one ready imported track with a complete beatgrid, Lumi phrase timeline
  and mapped AutoLoop for each phrase used in the run;
- load it into Local Playback, inspect the plan in Armed, switch to Live and
  start from the beginning;
- observe one uninterrupted complete-song run through SoundSwitch and physical
  DMX fixtures;
- repeat a short run containing pause/resume and one manual Control One override;
- retain an event transcript and screen/video evidence with expected versus
  observed bank, AutoLoop, phrase boundary and clock state.

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
  restores operation without restarting the full Lumi application.
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
