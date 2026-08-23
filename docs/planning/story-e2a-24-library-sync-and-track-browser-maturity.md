# Story E2A-24: Library sync and Track browser maturity

## Outcome

A DJ can refresh last-minute Rekordbox beatgrid/cue changes from either trusted
USB, find the affected track immediately and carry Lumi phrases from an older
edit or mashup version into its exact-beat-compatible successor without hidden
identity or timeline mutation.

## Acceptance criteria

- CHRM and GRAY remain independent sources even when they share manufacturer,
  model, playlist names and tracks.
- USB inspection keeps all Rekordbox data read-only; sync is source-scoped,
  playlist-scoped, atomic and idempotent. An explicit Add, Refresh or Sync may
  create one hidden `.lumi-source.json` identity marker at the volume root.
- A same-source changed analysis promotes the coherent beatgrid, waveform, cue
  and raw-phrase projection while preserving Lumi-owned phrase work.
- Older and incomparable revisions remain protected or reviewable.
- Source, scan and review status never inserts a transient page-level block
  that moves the USB lanes.
- Track search updates within 200 ms while typing, stale results cannot replace
  the newest query, and a visible clear action resets it.
- Every Track table header sorts the complete server-side result, not only the
  visible page; pagination remains deterministic.
- `Reuse Lumi Phrases` creates a new target revision, preserves the source and
  applies only to exact beat-compatible authored timelines.
- Version-like title suffixes such as `v003` and `v004` influence suggestion
  ranking only; they never cause implicit merging.

## Quality evidence

- Rust repository tests cover stable server-side ordering across pages and
  revision-safe creative reuse.
- Existing USB identity tests cover equal-model media, duplicate FAT identity,
  persistent marker identity, stable filesystem identity and legacy migration.
- Swift package tests cover bounded decoding and source identity presentation.
- The development app is exercised through the real macOS UI with both mounted
  trusted USB sources, live search, clear, sort and source-lane expansion.

## Non-goals

- Lumi never changes Rekordbox library, analysis or media data. Its only USB
  write is the explicit, atomic identity marker used to separate otherwise
  indistinguishable FAT media.
- Lumi does not automatically rebase phrases across a changed total beat count.
