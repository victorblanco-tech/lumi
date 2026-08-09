# Versioned phrase-timeline editing

E2A-05 makes `LumiPhraseTimeline`—not raw source phrases—the authoritative
lighting structure for an imported track. The first successful import
composition performs the deterministic initial source mapping once; opening an
older/uninitialized track also repairs that missing baseline safely. Every
accepted edit after that appends a new immutable SQLite revision.

## Safety model

The canonical aggregate stores ordered start points in beats. A valid timeline therefore:

- starts at beat zero and ends at the track's total beat count;
- contains one or more non-zero phrases with contiguous indexes;
- assigns every beat to exactly one phrase;
- permits any whole-beat boundary, including beats inside a bar;
- retains the imported analysis revision as provenance;
- references only earlier parent and restored-from revisions.

The engine requires `expectedTimelineRevision` on every mutation. A stale
client receives the typed `timelineRevisionMismatch` response with
`actualTimelineRevision`; it never overwrites the newer head.

## Edit semantics

| Command | Result |
| --- | --- |
| Place Phrase Point | Adds a role-bearing start point on the quantized playhead; its end is derived from the next point or track end |
| Split | Creates two non-zero phrases; both inherit the role, the left keeps its loop choice, the right becomes `AUTO` |
| Merge previous/next | Absorbs the chosen neighbour while retaining the selected phrase's role and strategy |
| Move boundary | Resizes exactly two adjacent phrases without allowing either to become empty |
| Delete/absorb | Requires an explicit previous or next target and cannot create a gap |
| Change role | Changes the selected phrase, resets its loop strategy to `AUTO`, and rejects a no-op |
| Change loop strategy | Stores a validated logical `AUTO`, fixed-Variant, or exact Theme override choice without selecting a Theme |
| Undo/redo | Appends a restore revision and moves a persistently reconstructed history cursor |
| Restore revision | Copies an immutable historical state into a new head; history is never rewritten |

Undo and redo stacks are reconstructed by replaying revision reasons and
restored-from references. They therefore survive an engine or app restart and
do not depend on transient Swift state.

## Runtime and UI boundaries

The Rust engine is the single writer. The macOS editor submits typed commands,
then renders only the returned authoritative snapshot. The fixed-dark editor
provides a continuous RGB waveform, continuous pan/zoom, beat-quantized Phrase
Points, split/merge/delete controls, role selection, boundary steppers, revision
history, undo/redo, and an explicit saved-revision badge. Bars remain stronger
visual and keyboard landmarks.

The local development app passes this durable database path to its engine
helper:

```text
~/Library/Application Support/Lumi/library.sqlite
```

Package/process tests omit that path and use isolated in-memory or temporary
databases. No Rekordbox or audio file is mutated.

Library edits are isolated from the active show reducer and planner. They leave
the session revision, current plan revision, operation state, and output record
count unchanged. Audio preview also remains independent: accepted edits keep
playback active, and an enabled selected-phrase loop adopts the new valid range
without exposing a transient invalid boundary.

## Verification

- randomized aggregate sequences prove contiguous whole-beat coverage;
- typed rejection tests cover zero-length selections, invalid split/boundary
  moves, missing absorption targets, stale revisions, and corrupt history;
- golden transcripts cover edit ordering and edit-during-preview loop adoption;
- SQLite tests cover v1→v2 migration, strategy/history round-trips,
  optimistic concurrency, and restart recovery;
- the real Swift client test executes edit → stale rejection → undo → engine
  restart → redo against the Rust process and a temporary SQLite database;
- Swift tests cover strict wire validation, whole-beat mutation snapping,
  keyboard/accessibility identifiers, and loop-safe playback;
- repository visual evidence and hands-on testing use the exact Terminal-built
  `Lumi.app`.

## E2A-13 migration

ADR-0014 supersedes the bar-only edit granularity. E2A-13 migrates canonical boundaries from bar indexes to beatgrid
positions and replaces range-first editing with ordered Phrase Points. Existing
bar-aligned revisions remain valid and migrate losslessly because a bar start is
also a whole-beat position. SQLite schema v5 performs this conversion and stores
only the Phrase Point beat; every end boundary is derived on load.
