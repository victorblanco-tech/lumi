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
intent to the engine. The engine remains authoritative for plan and output. A
client may present an immediately responsive pending selection, but it must
reconcile it with the returned authoritative revision and discard it after a
conflict. Key notation remains a presentation preference; canonical key data is
not mutated.

E1-11 extends the same intent boundary to demo and operation controls. The
workspace sends revisioned commands for load, reset, speed, playback, leader,
OFF, ARMED, LIVE, and PAUSED. Provider health and the latest bounded engine
timeline are decoded from snapshots; neither is inferred from button presses.

## Fixed player surfaces and RGB preview contract

E2B-01 keeps normalized physical deck identity separate from the lighting
leader role:

- snapshot players are ordered by the Pro DJ Link number in `deckId`, so Player
  1 remains left and Player 2 remains right;
- `hardwareModel` is optional presentation metadata copied from the exact
  numbered Pro DJ Link announcement; it is never used as transport identity;
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

The historical simulator waveform remains an internal fixture only. Production
Local Playback resolves exact local-library analysis; connected adapters use
the same provider-neutral preview contract. The same RGB composition is used by
the Library track editor so its overview and Live deck remain visually
consistent.

E2B-03 and E2B-04 move this behavior into the accepted show layout: provider
health and recent engine events are available from one compact `Tech` popover,
internal simulator controls are absent from the product UI, and the separate
next-plan workspace is folded into the stable non-master deck. The master deck
presents the current phrase and selectable remaining phrases in the same
surface. Selecting a phrase band opens one contextual editor directly below it;
no duplicate phrase list is rendered.
The current and past phrases are locked, while future Live phrases support a
revision-safe AutoLoop change or a Theme change that applies from that phrase
onward. A temporary pending presentation is always reconciled with the returned
authoritative plan revision.

Deck transport is equally authoritative. `PlaybackStateChanged` and beat
observations enter through `DeckSourceProvider`; snapshots publish `playing`
and the current beat for every deck. For Local Playback, the native audio
controller publishes measured position anchors. SwiftUI may interpolate a
smooth visual frame from the latest anchor, but uses one shared conversion for
the waveform, phrase band and AutoLoop Plan and never turns that interpolation
into an execution clock. A production Live Decks adapter supplies equivalent
provider-neutral anchors from its external source.

Static track analysis and dynamic transport intentionally travel at different
rates. Waveform, exact beatgrid, phrases and materialized plan are transferred
on load or reconciliation. Frequent transport commands use a lightweight
acknowledgement, avoiding a large dual-deck snapshot on every native audio tick.
This separation is formalized in ADR-0021.

When operation is Live and the Lumi CoreMIDI source is ready, the output worker
sends the executed materialized cue as a real SoundSwitch bank/button pulse.
The persisted four-bank/32-button catalog is owned by the output-profile
integration, not by this presentation package.

The bounded v1 transport limit is one MiB. Deck snapshots carry peak-preserving
RGB previews rather than full editor analysis and retain the same strict decoder
ceiling in Rust and Swift. Production detail data remains a separate capability
and must not grow the live snapshot without bound; normal high-frequency
transport uses lightweight acknowledgements instead of approaching that ceiling.

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
