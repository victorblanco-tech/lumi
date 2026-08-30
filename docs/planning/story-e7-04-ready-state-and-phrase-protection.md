# E7-04 – Ready-state clarity and phrase protection

Status: **Done; hardened in 0.5.0-dev-37** | Priority: **P0** | Effort: **5**

## User value

As a DJ, I see Ready for Show as an unmistakable green check, empty workflow
steps as a normal state, and I can protect completed Lumi phrase work without
blocking later USB beatgrid improvements.

## Acceptance criteria

- `Ready for Show` defaults to a green `checkmark.circle.fill` everywhere.
- A migration updates only the legacy default presentation and preserves custom
  workflow styling.
- An empty workflow queue says `Nothing to review` and never presents red
  Library-unavailable errors merely because zero tracks match.
- Each track has a revisioned `Protect Phrases` toggle in the Track Editor.
- Protection blocks every creative phrase mutation in both UI and engine.
- Trusted USB analysis, including beatgrid changes, remains importable and
  creates the existing source-attention review signal.
- Protection persists across restarts, is included in Library snapshots and is
  covered by migration, persistence, command and UI tests.
- Workflow selection and its result page are published atomically; a concurrent
  protection mutation cannot expose rows from another workflow step.
- Protection state is communicated in the fixed-size editor control without
  inserting or removing layout rows above the waveform.

## Architecture

ADR-0038 owns the separation between protected Lumi creative data and mutable,
reviewable source analysis. No workflow/protection code participates in live or
lighting timing lanes.
