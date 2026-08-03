# Native Library workspace

E2A-03 activates the native macOS Library destination without requiring a
Rekordbox installation. The local Rust engine explicitly starts the synthetic
demo provider, imports its baseline into an in-memory SQLite repository, and
serves only bounded presentation pages over the authenticated loopback session.

## Boundaries

- `lumi-library-sqlite` owns search, playlist membership, ordering, and paging.
- `lumi-engine` owns the active provider and query session; it serializes
  provider-neutral library facts into the existing versioned snapshot envelope.
- `LumiLibraryWorkspace` decodes wire facts into Swift presentation models and
  owns native Library views. It imports neither SQL nor provider adapters.
- the app target owns process supervision and composes the shared navigation,
  Live feature, and Library feature.

A query contains search text, an optional stable playlist ID, offset, and limit.
Input is bounded to 200 search bytes and 200 tracks per page. SQL search escapes
wildcards and runs against title, artist, and source track ID. The view never
loads the complete collection into memory.

## Visible states

The presentation contract distinguishes empty, importing, ready, stale,
degraded, and error. Track readiness separately distinguishes ready, missing
analysis, stale source, and source conflict, with explicit missing capabilities
and warnings. The current demo baseline is complete; deterministic presentation
fixtures prove the non-ready states until a fallible external source exists.

Track selection exposes BPM, global Camelot/Classic key formatting, duration,
source identity, analysis revision, Lumi timeline revision, color, and readiness.
The editor action opens the selected track's engine-backed Track Lighting
Editor analysis. Its waveform and isolated audio behavior are documented in
[`track-editor-preview.md`](track-editor-preview.md).

## Verification

The feature package tests authoritative decoding, 200-row wire bounds,
readiness filtering, English localization, and a 10,000-result/50-row native
page benchmark. The real Swift client integration test launches the Rust helper
and performs a server-side Library search plus editor open/close. Eight Library
PNGs cover dark/light, Camelot/Classic, and all operational states; two more
prove the fixed-dark editor under both host appearances.
