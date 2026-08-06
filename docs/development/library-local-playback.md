# Library Local Playback flow

Local Playback is a product deck source, not a demo or simulator. It lets a DJ
prepare and dry-test the same Lumi-owned phrase timeline and lighting plan that
will later be driven by Live Decks.

## Runtime flow

1. Live embeds a compact, independently scrollable Library browser beneath the
   two decks. Collection, imported playlists, search and pagination lead to
   **Load Deck A** and **Load Deck B** for a track with a Lumi timeline. The
   full Library screen exposes the same engine-authoritative workflow. The
   embedded browser expands into all remaining window height; a ready row can
   also be dragged onto either empty or loaded deck using an exact typed
   track/revision transfer.
2. The command carries the exact library track ID, target deck, expected
   timeline revision, and expected runtime state revision.
3. `LibraryWorker` resolves that track and revision and provides the original
   audio URI, duration, beat grid, RGB waveform, Lumi phrase timeline, and
   stable track identity.
4. `LocalPlaybackDeckSourceProvider` owns the two product deck slots, absolute
   playback position, play/pause state, and leader selection. It emits the same
   provider-neutral observations that a Live Decks adapter will emit.
5. The Swift audio controller plays the selected local source and reports its
   measured position to the engine. The Live playhead therefore follows actual
   playback and never an autonomous UI timer.
6. The planning worker uses only canonical Lumi phrase role IDs. Raw source
   phrase names never become planning roles after import.
7. Each planned phrase resolves to the actual mapped SoundSwitch bank and
   Autoloop button. Live only offers compatible, physically mapped choices for
   that Theme and Phrase Type.
8. Theme and Autoloop changes are plan-instance overrides. They do not mutate
   the Library timeline or catalog and are applied only to a phrase that has
   not started.

## Live behavior

- Deck A stays left and Deck B stays right. Changing master changes status; it
  never reorders the surfaces.
- Space toggles the Local Playback master deck and the Track Editor preview.
  A focused text field retains the key, so search terms can contain spaces.
- Live and Editor use the same continuous waveform viewport and persisted
  mouse/trackpad preferences. A zoomed phrase band follows the visible waveform.
- Finished and current phrases are read-only. Future phrases are editable and
  may optionally be pinned.
- The same source selector offers **Live Decks** and **Local Playback**.
  Internal simulator/replay providers never appear as product modes.
- A connected track with an exact Lumi library identity is `READY_EXACT`.
- An unmatched track may be displayed as `READY_TRANSIENT` when a provider
  supplies sufficient normalized phrase data, but does not silently persist it.
- Without trustworthy phrase data, the deck becomes `AUTO_HELD`; Lumi keeps
  the last safe look and suppresses automatic plan/output for that track.

## Safety and boundedness

- Track and timeline identity use optimistic concurrency; title/artist guessing
  cannot silently select a Library row.
- Missing audio fails closed for playback while analysis remains available.
- Source switching clears stale deck state rather than presenting Local
  Playback data as if it came from Live Decks.
- Ordinary app-screen navigation does not switch the deck source. Loaded deck
  identity, transport position, plan and master remain in the engine and are
  restored when Live is shown again.
- Full editor analysis remains available for deep zoom. A deck snapshot carries
  at most 1,024 peak-preserving RGB preview points so two real imported tracks
  remain safely below the authenticated one-megabyte protocol bound.
- The planning worker retains at most 256 library contexts.
- No network, DJ hardware, Rekordbox process/database, SoundSwitch process, or
  MIDI target is required to use Local Playback.

## Verification

Rust tests cover both slots, exact identity, absolute beat-grid transport,
mapped Autoloop materialization, plan overrides, leader changes, and safe
unmatched tracks. Swift tests cover decoding, fixed deck placement, editing
boundaries, RGB waveform presentation, and the real Rust process. Native visual
evidence is rendered as `local-playback-library-next-dark-camelot.png`.

The historical deterministic simulator fixtures remain available only for
internal adapter and regression tests.
