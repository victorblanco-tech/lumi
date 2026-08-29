# E7-02 – Configurable track-preparation workflow

Status: **Done in 0.5.0-dev-35** | Priority: **P0** | Effort: **8**

## User value

As a DJ, I can shape Lumi's preparation process to my own way of working while
keeping USB safety items fixed and unmistakable.

## Acceptance criteria

- Settings exposes ordered workflow steps with stable ID, name, icon and color.
- Users can add, remove and reorder custom steps; three migration anchors remain.
- Smart eligibility uses typed Library facts only and is bounded to eight rules.
- Catalog and per-track assignments use optimistic revisions and survive restart.
- Removed custom-step assignments move safely to `In Progress`.
- Workflow navigation and the editor use the same dynamic catalog and counts.
- Queries are paged and do no work on live, Ableton Link or MIDI lanes.

## Verification

- schema-17 migration and persistence tests;
- catalog validation, assignment and dynamic-query tests;
- command encode/decode and Swift snapshot tests;
- headed Settings Save and Track Editor assignment acceptance.
