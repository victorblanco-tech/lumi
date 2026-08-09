# ADR-0019: Rekordbox XML mirror with read-only analysis enrichment

- Status: **Accepted**
- Date: **2026-08-06**

## Context

Rekordbox XML provides playlist membership, metadata, audio locations and tempo
markers, but does not export Rekordbox RGB waveforms or phrase analysis. Those
capabilities are required before a mirrored track can drive a reliable Lumi
light plan. Rekordbox stores additional analysis in `ANLZ` files. Their `PQTZ`
tag contains the beat grid, `PWV4`/`PWV5` and newer tags contain colored
waveforms, and `PSSI` contains Rekordbox song-structure observations.

Lumi must use this information without making its availability a hard
dependency, writing to a Rekordbox installation, or allowing later source syncs
to overwrite user-owned phrase work.

## Decision

Library ingestion is split into two independently versioned capabilities behind
the existing provider boundary:

1. `Rekordbox XML Mirror` owns selected playlist scope, source metadata and
   archive/restore membership.
2. `Rekordbox Analysis Provider` enriches mirrored source identities with
   read-only observations from analysis files.

The XML mirror is persisted before analysis is available. A mirrored track can
therefore be `analysisPending`, but it is not promoted into the editable and
live-plannable Lumi catalog until required analysis is present. Lumi never
invents a waveform, beat grid or phrase to make an incomplete track appear
ready.

The analysis provider:

- discovers only bounded `ANLZ*.DAT`, `ANLZ*.EXT` and `ANLZ*.2EX` files below an
  explicitly approved Rekordbox analysis root;
- initially used `PPTH` to measure analysis availability; production identity
  is now resolved through the closed-database snapshot in
  [ADR-0020](0020-closed-rekordbox-snapshot-identity-resolver.md);
- copies matched analysis files to an application-owned snapshot before full
  parsing;
- never queries or mutates the production `master.db`; the accepted resolver
  copies it byte-for-byte and queries only the Lumi-owned snapshot;
- exposes raw capabilities and source provenance instead of Rekordbox-specific
  objects to the rest of the application;
- fails closed on malformed lengths, unknown required structures, excessive
  depth/count/size, or source changes during snapshotting.

The installed Rekordbox 7 analysis tree is
`share/PIONEER/USBANLZ`; the older sibling `PIONEER/USBANLZ` is not assumed to
be authoritative. Source discovery must select and report the actual tree
instead of silently combining both.

## POC evidence (2026-08-06)

With Rekordbox closed, the bounded parser scanned the real selected XML scope
without opening `master.db` and without modifying a Rekordbox file:

- 684 requested and locally present audio locations;
- 4,262 analysis sets and 13,954 bounded files inspected;
- zero malformed analysis sets;
- 675 unique, provisional POC matches; three ambiguous filename candidates
  were rejected and nine tracks remained unmatched;
- beatgrid on all 675 matches, `PSSI` phrases on 674, and both colored and
  three-band waveform variants on all 675;
- 2,025 matched `DAT`/`EXT`/`2EX` files copied to a temporary Lumi-owned
  snapshot before full parsing.

The POC also proved an identity limitation: `PPTH` stores stale, non-existing
audio roots for this migrated library, so no exact normalized path match is
possible. The coverage run therefore used an explicitly opt-in, unique-
filename-only POC fallback. That fallback is disabled by default, never treats
an ambiguous name as a match and is not approved as persistent product
identity. That gate is now passed by ADR-0020: XML `TrackID` resolves directly
to `djmdContent.ID` and its current `AnalysisDataPath` in a closed, Lumi-owned
database snapshot.

## Authoritative resolver evidence (2026-08-06)

The complete XML Collection matched 2,954/2,954 active database identities. In
the followed-playlist scope, 684/684 tracks resolved to existing analysis sets,
with beatgrid and colored waveform data on all 684 and PSSI observations on
683. No filename fallback was enabled. See ADR-0020 for the snapshot, secret,
path-confinement and read-only invariants.

Rekordbox `PSSI` labels remain raw source observations. A configurable mapping
creates the initial Lumi timeline. Once created, the timeline and all later
edits remain Lumi-owned. Refreshing XML or ANLZ data can offer an explicit
comparison but never silently replaces that timeline.

Waveform selection is capability-based. Lumi prefers the highest-resolution
colored representation that can be decoded safely. A decoded Rekordbox
waveform may use the same underlying data as Rekordbox/CDJ, but Lumi owns its
rendering and does not promise pixel-identical UI output.

Beat Link remains a separate live enrichment route. It may supply phrase data
for a loaded track that is absent from the local mirror, but it does not replace
offline library enrichment.

Future on-device analyzers, including Apple Music Understanding, implement the
same analysis-provider contract. Their output is marked with its own provenance
and confidence and is never presented as Rekordbox analysis.

## Consequences

- XML Apply Sync can be useful and safe even before analysis is complete.
- Archive and restore operate on stable source identities and retain all
  application-owned work.
- Rekordbox format changes are isolated to one adapter.
- A missing or unreadable analysis cache degrades to `analysisPending`; it does
  not corrupt the mirror or planner state.
- Exact Rekordbox phrase coverage and available waveform variants are measured
  in a read-only POC before production enrichment is enabled.
- The same underlying RGB/three-band samples needed for a Rekordbox-like
  waveform are available. Lumi still owns rendering; pixel parity is a
  separate visual implementation and verification task.
- Analysis enrichment may now use the accepted TrackID-to-AnalysisDataPath
  resolver; persistence and user-facing progress remain separate delivery work.

## Rejected alternatives

### Fabricate a default waveform or one-track phrase

Rejected because it would make an incomplete track appear safe for live use.

### Make the Rekordbox database the primary import source

Rejected for the first implementation because XML already provides explicit
user-controlled scope, while direct database access increases compatibility and
production-library risk.

### Let Rekordbox remain authoritative after import

Rejected because Lumi-specific roles and per-track lighting decisions must
survive source refreshes and provider replacement.
