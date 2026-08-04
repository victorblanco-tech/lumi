# Source reconciliation

Lumi treats an imported library as a versioned source baseline and the Lumi phrase timeline as user-owned work. A source refresh is therefore always compared before it can affect a timeline.

## Flow

1. The source adapter produces a complete immutable baseline.
2. Lumi classifies each stable track identity independently as metadata, waveform, beat-grid, and/or raw-phrase changes.
3. Metadata-only changes can be accepted with **Keep Lumi** without creating or changing a timeline revision.
4. Beat-grid or raw-phrase changes require an explicit choice: **Keep Lumi**, **Rebase**, **Merge**, or **Replace**.
5. Analysis and a resulting timeline revision are committed in one SQLite transaction. An error leaves both unchanged.

The demo provider exposes deterministic V1 and V2 baselines so this workflow is testable without touching a Rekordbox installation.

## Strategies

- **Keep Lumi** retains the complete Lumi timeline and loop strategies. It is only valid when the beat duration still matches.
- **Rebase** proportionally moves Lumi boundaries onto whole beats. Any rounded boundary is shown as an ambiguity before applying.
- **Merge** requires one explicit Lumi/source choice for every conflict. Gaps, overlaps, duplicate choices, and incomplete coverage are rejected.
- **Replace** adopts the newly mapped source phrases. The prior Lumi timeline remains in revision history and can be restored or reached through undo.

All successful timeline strategies create a `sourceReconcile` revision with an optimistic expected-head check. Metadata-only refreshes use an optimistic analysis-revision check and deliberately leave the timeline head untouched.

## Safety properties

- Track matching uses the stable source track ID, never title or list position.
- Preview is read-only and does not update the active baseline.
- Source mappings are used only to build the incoming candidate; existing Lumi phrase roles are not silently remapped.
- Every Phrase Point stored by Lumi is expressed as a whole-beat position.
- The database transaction covers baseline evidence, source analysis, and the timeline head.
- A replaced or rebased timeline is recoverable through immutable revision history.

The current desktop UI exposes classified changes, baseline revisions, rebase ambiguities, per-conflict merge choices, and all four strategies in the track editor.
