# Epic 2B – Live deck intelligence and rolling plans

Status: **Refined; E2B-01 complete, E2B-02 ready**

Target milestone: **0.2.0 – Deck Intelligence**

## Product outcome

Lumi presents a fixed, CDJ-recognizable Deck A/B workspace. The master role,
playhead and phrase state follow the authoritative deck source while the DJ can
prepare both the loaded next track and every not-yet-started phrase of the
current Live track.

## Accepted UX

- Desktop keeps Deck A left and Deck B right at all times.
- `MASTER · LIVE NOW` moves between those surfaces without reordering them.
- The master card is visually dominant; `PLAN READY` is deliberately quieter.
- Both cards show an RGB waveform, beatgrid, playhead and phrase band.
- The current phrase is locked; future Live phrases are editable.
- A future AutoLoop is phrase-specific. A future Theme choice applies from that
  phrase onward and is committed at its boundary.
- Technical component health is summarized behind one compact status control.
- iPhone portrait stacks the decks; landscape may show them side by side; both
  show phrases immediately below their waveform.

## Story map

### [E2B-01 – Fixed dual-deck Live surface](https://github.com/victorblanco-tech/lumi/issues/84)

Build the first visible vertical slice using the simulator:

- decode normalized track duration, phrases and RGB waveform preview;
- preserve Deck A/B ordering independently from `leaderDeckId`;
- render two fixed native deck surfaces side by side;
- move the strong master treatment from authoritative state;
- render RGB waveform, beatgrid, playhead and phrase band for both decks;
- retain the existing plan editor and revision-safe mutations;
- cover decoder validation, stable ordering and master changes with local tests.

### [E2B-02 – Production waveform resolution](https://github.com/victorblanco-tech/lumi/issues/86)

- extend the deck-source capability contract with preview/detail availability;
- resolve an exact local-library waveform by stable track identity;
- accept Beat Link preview/detail data behind the same normalized contract;
- cache immutable waveform analysis locally and overlay live transport state;
- show explicit unavailable/stale provenance without fabricated data.

### [E2B-03 – Promote and retain the rolling Live plan](https://github.com/victorblanco-tech/lumi/issues/83)

- publish full plan snapshots for the current and next track;
- retain the next plan when its deck becomes master;
- expose current phrase and future editability per cue;
- reject changes to active/past phrases and stale track-load or plan revisions;
- apply phrase AutoLoop changes and Theme-from-phrase changes at the boundary.

### [E2B-04 – Live planning interaction and diagnostics](https://github.com/victorblanco-tech/lumi/issues/85)

- integrate the remaining-Live-track editor into the master deck surface;
- keep next-track planning available alongside it;
- collapse engine, source, planner, timing and MIDI health behind compact status;
- preserve explicit degraded, disconnected and revision-conflict feedback;
- keep simulator/test controls available without dominating the show view.

### [E2B-05 – Companion presentation contract](https://github.com/victorblanco-tech/lumi/issues/82)

- define a local-client snapshot suitable for the later native iPhone app;
- keep master, deck, waveform, phrase and plan semantics engine-authoritative;
- specify portrait stacking and landscape dual-deck behavior;
- defer discovery, pairing and native iOS implementation to the iPhone epic.

## Exit criteria

- Master switches are reproducible without moving the Deck A/B surfaces.
- A loaded non-master track has a ready, editable plan before transition.
- After transition, future phrases of the current track remain safely editable.
- Both waveforms and phrase bands use provider-neutral data with visible
  provenance and no internet dependency.
- Stale input or stale edits cannot regress or mutate authoritative state.
- macOS Swift tests, Rust tests, native build and headless visual evidence pass
  locally without GitHub Actions.

## Dependencies and boundaries

- Epic 2A supplies the canonical library, Lumi phrase timeline and RGB analysis.
- ADR-0010 remains the deck-provider boundary.
- ADR-0016 defines stable deck identity and rolling-plan behavior.
- SoundSwitch execution stays in Epic 3; this epic prepares authoritative cues.
- Native iPhone implementation remains in the iPhone Remote epic.
