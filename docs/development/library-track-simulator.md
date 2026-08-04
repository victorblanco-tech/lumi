# Library track simulator flow

E2A-11 connects the application-owned Library timeline to the existing dual-deck
simulator without coupling the simulator to SQLite, Rekordbox, MIDI, or
SoundSwitch.

## Runtime flow

1. Library exposes **Load on Deck 1** and **Load on Deck 2** for a track with a
   Lumi timeline.
2. The command carries the library track ID, target deck, expected timeline
   revision, and expected runtime state revision.
3. `LibraryWorker` resolves that exact track and revision. It builds normalized
   `TrackMetadata` from the current `LumiPhraseTimeline`; raw source phrases are
   never read for playback planning.
4. The normalized deck event carries stable provider kind, library source ID,
   source track ID, analysis revision, and Lumi timeline revision. Unknown or
   stale identities fail closed before simulator state changes.
5. `SimulatorDeckSourceProvider` allocates a new monotonic `TrackLoadId` and
   publishes an ordinary provider-neutral `TrackLoaded` observation.
6. A non-leader load creates the Next plan. Theme selection remains late-bound;
   after Theme selection, every phrase strategy resolves through the frozen
   logical Autoloop catalog revision.
7. Next exposes the exact source, Lumi revision, Theme reason, phrase role,
   strategy, variant, resolution reason, and logical dry-run Autoloop entry.
8. Leader change activates the already prepared plan through the existing
   single-writer reducer. Accelerated playback emits one dry-run action at each
   bar-aligned phrase boundary.

The logical entry ID is the future output-adapter target. Existing scene
bank/slot values remain simulator evidence and are not interpreted as
SoundSwitch hardware configuration.

## Safety and boundedness

- Both runtime and timeline revisions use optimistic concurrency.
- Track identity is exact; no title/artist heuristic can silently select another
  library row.
- Missing tracks, timelines, catalog coverage, or stale revisions return typed
  errors while preserving the last valid Library view.
- The planning worker retains at most 256 library load contexts.
- Loading the current leader resets its position without fabricating a leader
  change. Loading the other deck prepares Next.
- No DJ hardware, Rekordbox process/database, network, SoundSwitch, or MIDI
  target is needed.

## Verification

The Rust tests cover both decks, stable identity facts, exact and stale timeline
matching, logical Autoloop resolution, atomic activation, and exactly-once
phrase-boundary output. The reviewed golden transcript is
`fixtures/demo-library-v1/simulator-e2e.json`.

The Swift process integration test runs the same flow through the real local
engine executable. Native evidence is rendered as
`library-simulator-next-dark-camelot.png`, and the exact Terminal-launched app is
used for the final hands-on check.
