# Rekordbox XML playlist sync

- Status: **Accepted; canonical analysis import delivered and real-library verified**
- Accepted: **2026-08-06**
- Source mode: **XML playlist scope plus closed local analysis snapshot, read-only**
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
- no writes, moves or deletes in Rekordbox; the live database is never queried,
  and SQL runs only against a verified Lumi-owned snapshot.

The selected XML export is the disclosure boundary and starts collapsed. Its
summary always shows the exact filename and current followed scope; expanding
it reveals the playlist tree and source-specific behavior. `Initial Phrase
Mapping` remains a separate collapsed disclosure. Playlist selection is not a
top-level page section because it belongs to the concrete XML export.
Playlist folders themselves also start collapsed and expand independently, so
opening a large export does not immediately render its complete hierarchy.

The source card keeps its primary action above the potentially long playlist
tree. `Preview Import`/`Check for Changes` first reloads the newest XML and then
calculates one hash-bound preview. A reviewed preview keeps `Apply Changes` in
its header while detailed playlists, diagnostics and fingerprint are collapsed.
Folder selection is setup; reload plus preview is one normal operating action.
A zero-diff preview reports `Up to date` and disables Apply instead of asking
the user to repeat a no-op transaction.

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

The result is held in memory as a hash-bound apply plan. `Apply Changes` atomically
stores the provider-neutral mirror, archive/restores absent or returning source
identities and retains all Lumi-owned work. It does not replace the active demo
provider or fabricate analysis. Changing the selected paths, future-child
setting or newest export makes the visible preview inapplicable until it is
recalculated.

## Persistent mirror and analysis enrichment

Apply stores the validated XML selection in a separate provider-neutral source
mirror. New and changed identities are upserted, identities absent from every
followed playlist are archived, and reappearing identities are restored. The
transaction does not create placeholder waveforms or phrases and does not touch
existing Lumi-owned timelines.

The applied mirror is enriched separately by the read-only Rekordbox Analysis
Provider described in [ADR-0019](../../architecture/adr/0019-rekordbox-xml-analysis-en-lumi-owned-enrichment.md).
Its first POC measures matching and availability for:

- `PQTZ`: detailed Rekordbox beat grid;
- `PSSI`: phrase boundaries, Rekordbox mood/type and fill information;
- `PWV4`/`PWV5`: colored preview and scrolling waveforms;
- `PWV6`/`PWV7`: newer three-band waveform variants when present.

Mirrored tracks remain visibly `analysisPending` until enough analysis is
available for editing and live planning. Missing information is never
fabricated.

The UI calls this state **Metadata staged**, never merely **Persisted**. It also
states that staged identities are not published in `Library > Tracks` until the
authoritative beatgrid, waveform and phrase enrichment succeeds. This avoids a
false implication that the import is already usable while preserving the
fail-closed canonical track model.

The real read-only POC found beatgrids for 675/675 provisionally matched tracks,
`PSSI` phrases for 674/675 and both colored plus three-band waveform tags for
675/675. The analysis bytes can therefore drive the desired RGB editor
waveform. Exact Rekordbox pixels are not promised because Lumi supplies its own
renderer.

The first POC found stale `PPTH` roots. The accepted closed-database resolver now
maps XML `TrackID` to the current `AnalysisDataPath` from a Lumi-owned SQLCipher
snapshot. The configured 684-track scope resolved 684/684 without a filename
fallback; all had beatgrid and colored waveforms and 683 had source phrases.
See [ADR-0020](../../architecture/adr/0020-closed-rekordbox-snapshot-identity-resolver.md).

## Delivered canonical publication

`Import Analysis` is available only for a current hash-bound XML preview and a
closed Rekordbox installation. It creates a verified database snapshot, resolves
the selected stable IDs, snapshots and parses their DAT/EXT/2EX companions, and
builds the complete provider-neutral baseline before opening the SQLite write
transaction. Activating Rekordbox replaces the demo source atomically; a parser,
identity, path, metadata or persistence error keeps the previous active source.

Detailed waveform data is peak-preserving bounded to 16,384 points per track.
The authenticated local protocol permits at most 1 MiB per message so a real
track's beatgrid, detailed RGB waveform, phrases and catalogs remain bounded
while still supporting the editor's zoomed view.

Native verification on 2026-08-06 published the currently configured 393-track,
13-playlist scope. Lumi persisted 238,708 beatmarkers, 6,429,828 detailed
waveform points and 6,615 raw phrase observations, created 393 initial Lumi
timelines, reopened the first track in the editor, and restored the same source
after a full app restart. No temporary import snapshot remained afterward.
