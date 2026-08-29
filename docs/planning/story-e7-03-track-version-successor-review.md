# E7-03 – Track-version successor review and phrase reuse

Status: **Done in 0.5.0-dev-35** | Priority: **P0** | Effort: **8**

## User value

As a DJ replacing an edit or mashup with a newer file, I can identify the likely
predecessor, compare the versions and safely carry forward my Lumi phrases and
AutoLoop choices without recreating them.

## Acceptance criteria

- A fixed `New track versions` system queue detects matching title families and artists.
- The editor shows source revision, total beats, BPM delta and duration delta.
- Automatic reuse is enabled only for an exact total-beat match.
- Reuse appends a new target timeline revision and preserves the source revision.
- `Keep Separate` suppresses that exact suggestion without deleting either track.
- Both decisions are persisted and recorded in an append-only audit table.
- Version review is Library-only and cannot influence a running show.

## Verification

- exact-beat phrase and loop-strategy reuse regression;
- incompatible beat-count fail-closed regression;
- persisted review filtering and revision-conflict regression;
- headed editor review acceptance.
