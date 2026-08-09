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

The Library always composes the engine-backed Track Lighting Editor above a
native, paged track table. The first available track loads automatically;
double-clicking any table cell loads that row into the editor. Selection alone
does not replace the editor, which prevents accidental loads while browsing.

Track color/title, artist, BPM, global Camelot/Classic key formatting, duration,
source, Lumi timeline revision, and readiness are native table columns. Source
track ID and analysis revision are hidden by default but can be enabled. AppKit
owns resize, reorder, sorting affordances, horizontal/vertical table scrolling,
and accessibility; `TableColumnCustomization` persists the user's column
arrangement locally. There is no separate metadata inspector.

The upper and lower panes use a native vertical splitter. The phrase inspector
contains no nested vertical scroll view, while the detailed waveform keeps its
dedicated native horizontal trackpad/mouse-wheel monitor and overview drag
navigation. Its waveform and isolated audio behavior are documented in
[`track-editor-preview.md`](track-editor-preview.md).

## Verification

The feature package tests authoritative decoding, 200-row wire bounds,
readiness filtering, English localization, exact fractional waveform pan/zoom,
and a 10,000-result/50-row native page benchmark. The real Swift client
integration test launches the Rust helper and performs a server-side Library
search plus editor analysis load. Native application verification covers the
persistent editor, row double-click loading, fixed inspector layout, and table
column behavior.
