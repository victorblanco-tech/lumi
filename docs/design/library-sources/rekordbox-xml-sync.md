# Rekordbox XML playlist sync

- Status: **Accepted; engine-owned source-scope preview delivered**
- Accepted: **2026-08-06**
- Source mode: **Official XML export, read-only**
- Stories: [E2A-20](https://github.com/victorblanco-tech/lumi/issues/92), [E2A-21](https://github.com/victorblanco-tech/lumi/issues/91), [E2A-22](https://github.com/victorblanco-tech/lumi/issues/90)

## Product outcome

Lumi follows selected Rekordbox playlists instead of importing the complete
Collection. The configured XML folder and import behavior live with the source
under `Library > Sources & Import`, not in global Settings.

## Source settings

- local folder containing user-created Rekordbox XML exports;
- deterministic newest-export selection with explicit file identity; validation
  must pass before it is presented or used;
- followed playlists and playlist folders;
- `Include future child playlists` per followed folder, enabled by default;
- `Mirror playlist membership`, enabled for the first provider version;
- `Archive tracks removed from all followed playlists`, mandatory and visible;
- no automatic writes, moves, deletes, exports, or Rekordbox database access.

Settings are modeled as source behavior so future adapters can expose their own
capabilities without adding unrelated switches to global application Settings.

## Sync semantics

```text
configured XML folder
  -> discover immutable exports
  -> read playlist tree only
  -> resolve followed playlists/folders
  -> extract only referenced track records
  -> preview diff against last successful source snapshot
  -> atomically apply active membership
```

- A source track exists once even when it belongs to multiple followed playlists.
- Repeated references to the same track inside one Rekordbox playlist are
  normalized to one Lumi membership and reported in preview diagnostics.
- Playlist order and membership mirror the validated XML snapshot.
- Removal from one playlist does not remove a track still referenced elsewhere.
- A track absent from all followed playlists becomes hidden and archived.
- Archived tracks retain Lumi phrases, loop choices, revision history, and source
  identity for automatic restoration when re-added.
- Source refresh never rewrites a Lumi-owned phrase timeline.
- Selecting a folder with future-child inclusion follows new descendant
  playlists without importing unrelated sibling folders.

## Capability contract

The XML adapter exposes playlists, metadata, track color, local audio locations,
beatgrid, cues, and loops where present. It explicitly does not claim Rekordbox
RGB waveform, phrase analysis, vocal analysis, My Tag, or smart-playlist logic.
Missing capabilities are never fabricated. RGB waveform work is lazy and only
targets tracks referenced by followed playlists.

## Safety and scale

- XML parsing is bounded by file-size, node-count, depth, text-length, and track-
  reference limits.
- Folder discovery ignores partial, hidden, and non-XML files.
- Preview records the XML content hash; Apply rejects a changed source file.
- Parse, validation, or persistence failure retains the last complete snapshot.
- The XML and all Rekordbox/audio files remain read-only.

## Delivered preview boundary

`Preview Sync` is an engine command, not a UI-only estimate. It reparses the
newest XML export using the bounded Rust adapter and returns:

- export filename, Rekordbox version and SHA-256 content identity;
- exact normalized playlist scope and per-playlist membership counts;
- unique source-track count versus total Collection count;
- missing metadata, beatgrid, color, waveform and phrase capabilities;
- the number of duplicate Rekordbox playlist references normalized by Lumi.

The result is held in memory as `previewOnly`. It does not mutate the SQLite
library, does not replace the demo provider, and cannot be applied from the UI.
Changing the selected paths, future-child setting or newest export makes the
visible preview inapplicable until it is recalculated.
