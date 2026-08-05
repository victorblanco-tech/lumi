# Native Live workspace

E1-08 presents the authoritative engine snapshot as the native macOS `Live`
workspace. SwiftUI does not select the lighting leader, invent a next deck, or
create plan decisions.

## Boundaries

`LumiLiveWorkspace` is split by responsibility:

- `Mapping` validates protocol DTOs and produces immutable engine snapshots;
- `Presentation` maps lifecycle and provider health to explicit UI states;
- `Views` renders only the supplied `Live`, `Next`, and retained-plan presentation;
- `Previews` contains deterministic Ready, Loading, Fallback, Stale,
  Disconnected, and Degraded fixtures;
- `LumiVisualEvidence` renders those fixtures without opening an app window.

The macOS app owns process supervision and supplies the resulting
`LiveWorkspaceState`. Theme, scene, lock, and regenerate controls emit revisioned
intent to the engine. The view never applies an optimistic shadow plan: it
renders the returned authoritative snapshot, or refreshes after a revision
conflict. Key notation remains a presentation preference; canonical key data is
not mutated.

E1-11 extends the same intent boundary to demo and operation controls. The
workspace sends revisioned commands for load, reset, speed, playback, leader,
OFF, ARMED, LIVE, and PAUSED. Provider health and the latest bounded engine
timeline are decoded from snapshots; neither is inferred from button presses.

## Fixed deck surfaces and RGB preview contract

E2B-01 keeps normalized physical deck identity separate from the lighting
leader role:

- snapshot decks are ordered by `deckId`, so Deck A remains left and Deck B
  remains right;
- `leaderDeckId` controls only the `MASTER · LIVE NOW` treatment;
- duration and contiguous track phrases are validated at the Swift mapping
  boundary before presentation;
- an optional `track.waveformPreview` contains explicit provenance, `rgb` style
  and bounded low/mid/high sample values;
- the Swift renderer composites those values into one RGB waveform, overlays a
  normalized beatgrid and authoritative playhead, and renders the validated
  phrase band directly below it;
- absent waveform data produces an explicit unavailable state rather than a
  client-generated substitute.

The simulator is the first waveform provider and marks its deterministic samples
as `simulator`. Production local-library and Beat Link resolution is E2B-02.
The same RGB composition is used by the Library track editor so its overview and
Live deck remain visually consistent.

E2B-03 and E2B-04 move this behavior into the accepted show layout: provider health and
recent engine events are available from one compact `Tech` popover, simulator
controls live in a `Demo` menu, and the separate next-plan workspace is folded
into the stable non-master deck. The master deck presents the current phrase and
selectable remaining phrases in the same surface. Selecting a phrase band opens
one contextual editor directly below it; no duplicate phrase list is rendered.
The current and past phrases are locked, while future Live phrases support a
revision-safe AutoLoop change or a Theme change that applies from that phrase
onward. The UI never keeps an optimistic shadow plan.

Deck transport is equally authoritative. `PlaybackStateChanged` and beat
observations enter through `DeckSourceProvider`; snapshots publish `playing`
and the current beat for every deck. SwiftUI derives the playhead only from that
snapshot. The simulator publishes the same events as a production adapter,
pauses without advancing, and stops at track end instead of looping.

When operation is Live and the Lumi CoreMIDI source is ready, the output worker
sends the currently executed demo cue as a real SoundSwitch bank/button pulse.
This is deliberately a bounded integration slice. Resolving the complete
persisted four-bank/32-button catalog is owned by the output-profile integration,
not by this presentation package.

The bounded v1 transport limit is 128 KiB. This accommodates the two normalized
RGB waveform previews in an authoritative dual-deck snapshot while retaining a
strict decoder ceiling in both Rust and Swift. Production detail data remains a
separate E2B-02 capability and must not grow the live snapshot without bound.

## Headless visual evidence

Generate the review matrix with:

```bash
./scripts/render-visual-evidence.sh
```

The command writes nine 1280×1200 PNGs to the ignored
`build/VisualEvidence/` directory. It uses fixed content, dimensions, locale,
appearance, and key notation. Rendering therefore works while the login session
is locked and does not require an active app window.

The full repository verification also renders the matrix and fails unless all
nine non-empty PNGs are produced. The matrix includes a successful locked cue
edit and revision-conflict feedback. Visual review remains necessary for layout,
contrast, truncation, and semantic state use.

## Verification coverage

Presentation tests read the canonical recorded snapshot contract and prove:

- the engine leader becomes `Live` and the other loaded deck becomes `Next`;
- the next plan belongs to the presented Next track-load instance;
- mismatched plan/deck snapshots fail at the mapping boundary;
- fallback and stale states retain their authoritative content;
- disconnected state never fabricates deck or plan data.

Real-process coverage additionally proves that pause freezes the authoritative
beat, resume advances it again, future Live edits survive a snapshot round trip,
and an edit for a started phrase is rejected without mutating the plan.

All controls and primary workspace regions expose stable accessibility
identifiers. App content remains scrollable at supported window sizes; the
headless evidence renderer uses an equivalent static layout because offscreen
`ScrollView` rendering requires a live window server.
