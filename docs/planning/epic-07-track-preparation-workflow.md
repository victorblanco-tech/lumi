# Epic 7 – Track preparation workflow

Status: **In progress** | Priority: **P0**

## Outcome

The Track Editor becomes a preparation workspace in which a DJ can see what
still needs work, mark a track Ready for Show and safely re-review tracks whose
trusted USB source changed after preparation.

## Architecture boundary

All work is Library-only and follows ADR-0037. No workflow query, status or rule
may participate in Pro DJ Link ingestion, Ableton Link, plan compilation at a
phrase boundary or MIDI execution.

## Phases

### Phase 1 — Stable foundation and visible milestone

Status: **Done in 0.5.0-dev-34**

- fixed statuses `Not Started`, `In Progress`, `Ready for Show`;
- fixed system queue `Changed after USB sync`;
- exact attention reasons for metadata, waveform, beatgrid, hot cues and source
  phrases;
- workflow navigation and counts next to Playlists;
- status control and review action in the Track Editor;
- revision-safe persistence, bounded queries and migrations;
- automated Rust/Swift regression coverage and headed macOS UI verification.

### Phase 2 — Configurable workflow

- Settings catalog for user-defined preparation steps;
- rule builder over durable track facts, without arbitrary executable queries;
- order, label, icon and color per step;
- previews and validation that prevent overlapping or unreachable rules;
- migration of the three phase-1 stable identities into the configurable model.

### Phase 3 — Version and replacement workflow

- detect likely new edit/mashup versions as an explicit review item;
- compare beat compatibility and source provenance;
- offer safe Lumi phrase reuse from the archived/older version;
- never delete creative work automatically;
- preserve a recoverable audit trail for relink/replacement choices.

## Epic acceptance

- Preparation work is immediately understandable from the Library UI.
- A recent USB beatgrid correction is visible before a show and cannot silently
  remain Ready for Show.
- Search, sorting, playlists and workflow queues remain paged and responsive at
  10,000 tracks.
- UI disconnects and workflow activity cannot affect live/output timing.
