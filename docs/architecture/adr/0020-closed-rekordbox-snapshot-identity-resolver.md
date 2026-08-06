# ADR-0020: Closed Rekordbox snapshot as authoritative analysis resolver

- Status: **Accepted and POC-proven**
- Date: **2026-08-06**

## Context

The Rekordbox XML mirror deliberately owns playlist scope, but its audio paths
are insufficient to locate analysis reliably after a library or disk migration.
The first ANLZ POC proved this: many `PPTH` values still contained an old root.
A unique-filename fallback found most tracks, but filenames are not a safe
persistent identity and ambiguous matches must never enter the Lumi library.

Rekordbox 7 stores the stable track `ID` and its current `AnalysisDataPath` in
an encrypted SQLCipher `master.db`. Lumi needs that relationship without
querying a live production database, writing any Rekordbox file, or making
Rekordbox's schema visible outside its adapter.

## Decision

The XML export remains the explicit user-controlled playlist selector. A
separate Rekordbox identity resolver enriches only those selected track IDs:

```text
selected XML TrackID
  -> closed master.db
  -> byte-identical Lumi-owned database snapshot
  -> SQLCipher read-only + query_only
  -> djmdContent.ID / AnalysisDataPath
  -> path confined below approved share root
  -> Lumi-owned ANLZ snapshot
  -> bounded analysis parser
```

The resolver follows these invariants:

- Rekordbox must be closed. A source with `master.db-wal`, `master.db-shm` or a
  journal is rejected rather than merged or queried.
- The production database is opened only as a normal read-only file stream for
  copying and hashing. SQL is executed exclusively against a newly created
  Lumi-owned snapshot.
- Source metadata and SHA-256 are checked around the copy. Any source change
  deletes the incomplete copy and fails closed.
- The copied database is marked read-only. SQLCipher is invoked with both its
  read-only open flag and `PRAGMA query_only=ON`.
- Database credentials are supplied through a dedicated secret boundary,
  redacted from debug output and never placed in process arguments or logs.
- SQL selects only `ID`, `FolderPath`, and `AnalysisDataPath` for the requested
  numeric XML identities. Track titles and other library content are not
  needed by this resolver.
- `TrackID == djmdContent.ID` is authoritative. Audio-path equality is a
  diagnostic, not an alternative identity rule.
- An analysis path must resolve to an existing `ANLZ*.DAT` below the approved
  Rekordbox `share` root. Absolute paths, traversal, symlink escapes, missing
  files and duplicate database identities fail or remain explicitly missing.
- Full DAT/EXT/2EX parsing happens only after the resolved set has been copied
  into a Lumi-owned analysis snapshot.

The SQLCipher process adapter is an implementation boundary. Development may
point it at the locally installed executable. A distributed Lumi build must
bundle and sign a reviewed helper/library; Homebrew is not a runtime dependency
of the product architecture.

## Real-library verification

On 2026-08-06, with Rekordbox closed and after the user completed a library and
music-file backup:

- the complete XML Collection contained 2,954 identities and all 2,954 matched
  an active `djmdContent.ID` in the snapshot;
- the configured followed-playlist scope contained 684 unique tracks;
- all 684 resolved to an existing analysis DAT with zero ID, analysis-path or
  selected-scope audio-path conflicts;
- 2,052 DAT/EXT/2EX files were copied before parsing;
- beatgrid and RGB/three-band waveform capabilities were present for all 684;
- Rekordbox phrase observations were present for 683, leaving one track
  honestly phrase-missing.

No production Rekordbox or audio file was modified. The database and analysis
snapshots used for this verification were temporary Lumi-owned copies.

## Consequences

- The filename fallback is no longer part of the production path.
- XML remains useful instead of being replaced: it controls what enters Lumi;
  the database supplies authoritative identity and analysis location.
- A missing database row or analysis file degrades that track to
  `analysisPending`; it never guesses.
- Direct library ingestion can now proceed to persistence and UI integration
  without making Rekordbox the long-term owner of Lumi phrases.
- Rekordbox schema, encryption and process details remain isolated behind one
  replaceable adapter.

## Rejected alternatives

### Query the live production database read-only

Rejected because read-only SQL does not guarantee a transactionally complete
view while Rekordbox and its WAL are active, and it unnecessarily couples Lumi
to the production file.

### Replace XML with a complete database import

Rejected because the DJ explicitly wants playlist-scoped import and mirror
semantics. The database is enrichment, not scope.

### Keep unique-filename matching

Rejected because filenames are mutable and non-unique. It remains available
only as opt-in POC evidence in the old scanner and is disabled by default.
