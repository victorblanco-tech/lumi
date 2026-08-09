# Epic 2B – Live deck intelligence and rolling plans

Status: **Local Playback product slice user-accepted; BLT MIDI adapter functionally proven, connected-hardware hardening remains**

Target milestone: **0.2.0 – Deck Intelligence**

## Product outcome

Lumi presents a fixed, CDJ-recognizable Deck A/B workspace. The master role,
playhead and phrase state follow the authoritative deck source while the DJ can
prepare both the loaded next track and every not-yet-started phrase of the
current Live track.

## Accepted UX

- Desktop keeps Deck A left and Deck B right at all times.
- `MASTER` moves between those surfaces without reordering them. `LIVE NOW` is
  added only while Lumi is in Start and that Master deck is actually playing.
- The Master card and selected operation control share one state language:
  Off is white, Armed is orange, Live is red and Paused blinks orange.
- The master card is visually dominant; `PLAN READY` is deliberately quieter.
- Both cards show an RGB waveform, beatgrid, playhead and phrase band.
- Editor and Live share one continuous beat-space interaction model: slider,
  vertical wheel/pinch zoom, horizontal pan, mouse/playhead zoom anchor and
  optional reversed horizontal scrolling. Phrase bands stay aligned while
  zooming and no redundant `1–4` beat labels are rendered.
- The current and completed phrases are read-only; future Live phrases are
  editable. An explicit pinned choice remains distinguishable from time state.
- A future AutoLoop is phrase-specific. A future Theme choice applies from that
  phrase onward and is committed at its boundary.
- Technical component health is summarized behind one compact status control.
- iPhone portrait stacks the decks; landscape may show them side by side; both
  show phrases immediately below their waveform.

## Story map

### E2B-07 – Replace visible simulation with product deck sources

Status: **Complete and locally verified**

- remove Demo/Simulator state, controls, labels and automatic test playback
  from production Live;
- start Live with two empty deck surfaces and an explicit source selector;
- expose `Live Decks` and `Local Playback` as the only product modes;
- keep simulator/replay providers exclusively in automated tests.

### E2B-08 – Local Playback vertical slice

Status: **Complete, locally verified and user-accepted on 2026-08-07**

- load a ready Lumi Library track on Deck A or B;
- browse Collection and imported playlists directly below the fixed deck
  surfaces, with search, native row selection and bounded pagination;
- let that browser consume all vertical space left beneath the fixed decks;
- keep the selected playlist independently scrollable and make the complete
  row and Load Deck A/B controls reliable click targets;
- play the actual local/demo audio, pause, seek and choose the lighting master;
- toggle the master Local deck with Space while keeping normal spaces available
  to text fields; the Track Editor uses the same shortcut contract;
- publish authoritative normalized position and playback observations;
- use the exact Lumi timeline, role identity, RGB waveform and AutoLoop
  resolution through planning and MIDI execution;
- list only real mapped Autoloops for the selected Theme and Phrase Type;
- materialize the chosen SoundSwitch bank/button into the executable plan so
  preview, plan state and emitted MIDI can never disagree;
- support dry rehearsal through the existing independent operation states.

The Live-embedded Library is now the primary Local Playback loading workflow.
Loading a full Rekordbox-derived analysis no longer duplicates its complete
16,384-point waveform in the authenticated snapshot. The editor retains full
detail while each deck receives a peak-preserving preview of at most 1,024
points, keeping real track loads below the bounded protocol message size.
Deck loads, playback position and master state remain engine-authoritative when
the user navigates between Live, Library, Integrations and Settings.

The stabilized transport uses one authoritative local-audio position anchor and
one exact beat-grid conversion for the waveform, phrase band and AutoLoop Plan.
The native client interpolates presentation frames between measured anchors;
that visual clock never schedules lighting output. Track loads and leader
changes reconcile through bounded engine snapshots, while frequent transport
updates use lightweight acknowledgements. Real imported tracks whose final beat
marker lies just beyond their nominal audio duration are accepted and clamped
safely rather than intermittently failing to load.

Manual acceptance on 2026-08-07 covered two real Rekordbox-enriched tracks,
play/pause, seek, zoom, six consecutive leader switches, exact phrase/AutoLoop
alignment and navigation away from and back to Live. The user confirmed that
the complete Local Playback UI behaves exactly as intended. Physical light
output is intentionally the next Epic 3 acceptance slice, not part of E2B-08.

### E2B-09 – Plan eligibility and unmatched-track safety

Status: **Complete and locally verified**

- resolve exact Library identity before planning a connected deck load;
- allow complete mapped provider analysis as an explicitly transient plan;
- represent missing, incomplete, stale and unmapped analysis as `AUTO HELD`;
- hold the current look without disabling manual MIDI or the other deck;
- never fabricate a role, phrase timeline, waveform or ready plan.

### [E2B-01 – Fixed dual-deck Live surface](https://github.com/victorblanco-tech/lumi/issues/84)

Status: **Complete and locally verified**

Build the first visible vertical slice using a deterministic acceptance source
(now retained as an internal test fixture only):

- decode normalized track duration, phrases and RGB waveform preview;
- preserve Deck A/B ordering independently from `leaderDeckId`;
- render two fixed native deck surfaces side by side;
- move the strong master treatment from authoritative state;
- render RGB waveform, beatgrid, playhead and phrase band for both decks;
- retain the existing plan editor and revision-safe mutations;
- cover decoder validation, stable ordering and master changes with local tests.

### [E2B-02 – Production waveform resolution](https://github.com/victorblanco-tech/lumi/issues/86)

Status: **Local-library slice complete; richer connected metadata remains planned**

- extend the deck-source capability contract with preview/detail availability;
- resolve an exact local-library waveform by stable track identity and render
  it through the same RGB deck presentation in both source modes;
- accept Beat Link preview/detail data behind the same normalized contract;
- cache immutable waveform analysis locally and overlay live transport state;
- show explicit unavailable/stale provenance without fabricated data.

### [E2B-03 – Promote and retain the rolling Live plan](https://github.com/victorblanco-tech/lumi/issues/83)

Status: **Complete and locally verified**

- publish full plan snapshots for the current and next track;
- retain the next plan when its deck becomes master;
- expose current phrase and future editability per cue;
- reject changes to active/past phrases and stale track-load or plan revisions;
- apply phrase AutoLoop changes and Theme-from-phrase changes at the boundary.

### [E2B-04 – Live planning interaction and diagnostics](https://github.com/victorblanco-tech/lumi/issues/85)

Status: **Complete, locally verified and user-accepted on 2026-08-08**

- integrate the remaining-Live-track editor into the master deck surface;
- keep next-track planning available alongside it;
- collapse engine, source, planner, timing and MIDI health behind compact status;
- preserve explicit degraded, disconnected and revision-conflict feedback;
- keep deterministic source controls out of the product show view.

Delivered behavior also makes the deck source authoritative for transport:
`playing` and the current beat enter through the provider event contract, the
playhead never advances independently in SwiftUI, and a stopped track does not
wrap back to its beginning. Selecting a phrase band opens its Theme and AutoLoop
controls directly beneath the waveform. Started phrases fail closed; future
Live phrases commit revision-safe changes to the retained active plan.

The Local Playback and connected Live Deck modes use the same Master status
component. Their operation-state border, top control and badge therefore cannot
drift into source-specific variants. Pause animation is restricted to the
lightweight outlines and does not invalidate or redraw the waveform timeline.

The accepted operation-state language is now implemented on both source modes:
Off uses a white Master outline, Armed orange, Live red and Paused a lightweight
blinking orange outline. The selected top operation control uses the same color.
The badge reads only `MASTER` outside active execution. During `Start` and real
deck playback it reads red `MASTER` plus green `LIVE NOW`; a prepared but stopped
deck is never presented as live. The user confirmed this behavior on 2026-08-08.

The first real-output slice maps the small demo Theme/AutoLoop vocabulary to
SoundSwitch bank and button MIDI pulses at phrase execution. The full persisted
four-bank/32-button output-profile mapping remains part of the SoundSwitch
integration work and is not duplicated in the Live view.

### [E2B-05 – Companion presentation contract](https://github.com/victorblanco-tech/lumi/issues/82)

Status: **UX contract accepted; native companion delivery remains planned**

- define a local-client snapshot suitable for the later native iPhone app;
- keep master, deck, waveform, phrase and plan semantics engine-authoritative;
- specify portrait stacking and landscape dual-deck behavior;
- defer discovery, pairing and native iOS implementation to the iPhone epic.

### [E2B-06 – Beat Link Trigger simulated-deck MIDI PoC](https://github.com/victorblanco-tech/lumi/issues/87)

Status: **Functional PoC and first adapter complete; latency/loss and reconnect evidence remain open**

- publish a dedicated Lumi virtual MIDI destination for deck-source input;
- configure two Beat Link Trigger simulation decks to send a documented,
  versioned MIDI mapping to that destination;
- decode the messages in an experimental provider behind `DeckSourceProvider`;
- prove two stable deck identities, load/play state, master switches, BPM and
  beat timing without leaking MIDI concepts into engine state or SwiftUI;
- measure latency, loss, ordering and reconnect behavior and explicitly record
  which metadata cannot be transported safely over MIDI;
- keep waveform resolution outside the MIDI mapping and use E2B-02 for local or
  Beat Link waveform data;
- deliver a go/no-go decision for MIDI as an initial BLT adapter transport, not
  an assumption that it becomes the production integration.

The two BLT simulation decks now drive Lumi's stable Deck A/B identity, load,
play/pause, master, pitch-adjusted BPM and beat position through a dedicated
`Lumi Deck Input` CoreMIDI destination. SoundSwitch uses a separate virtual MIDI
source. The simulator-only pitch normalization is isolated in the copied BLT
expression and does not change real-player tempo semantics. Manual product
validation passed; the story remains open only for measured latency/loss and a
repeatable disconnect/reconnect transcript.

## Exit criteria

- Master switches are reproducible without moving the Deck A/B surfaces.
- A loaded non-master track has a ready, editable plan before transition.
- After transition, future phrases of the current track remain safely editable.
- Both waveforms and phrase bands use provider-neutral data with visible
  provenance and no internet dependency.
- Stale input or stale edits cannot regress or mutate authoritative state.
- macOS Swift tests, Rust tests, native build and headless visual evidence pass
  locally without GitHub Actions.

The Local Playback form of these criteria is met and user-accepted. Final Epic
closure still requires the E2B-06 latency/loss and reconnect transcript and a
connected-deck hardware acceptance run when Pro DJ Link equipment is available.

## Dependencies and boundaries

- Epic 2A supplies the canonical library, Lumi phrase timeline and RGB analysis.
- ADR-0010 remains the deck-provider boundary.
- ADR-0016 defines stable deck identity and rolling-plan behavior.
- SoundSwitch execution stays in Epic 3; this epic prepares authoritative cues.
- Native iPhone implementation remains in the iPhone Remote epic.
