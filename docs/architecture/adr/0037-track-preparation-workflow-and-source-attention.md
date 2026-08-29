# ADR-0037: Track preparation workflow and source-change attention

Status: **Accepted**

Date: **2026-08-29**

## Context

Lumi already distinguishes technical Library readiness from source-sync health,
but a DJ also needs a personal preparation workflow. A technically usable track
may still need phrase editing, and a track previously marked Ready for Show must
be reviewed when a later USB sync changes its beatgrid, waveform, hot cues or
source phrases.

This workflow is preparation-time state. It must never enter Pro DJ Link,
Ableton Link, Light Plan execution or the realtime MIDI lane.

## Decision

### Separate technical readiness from preparation status

Lumi stores a manual preparation status independently from technical readiness:

- `Not Started`;
- `In Progress`;
- `Ready for Show`.

The three identities remain mandatory migration anchors, while Settings exposes
a revisioned catalog of up to twelve ordered steps. Every step has a stable ID,
label, SF Symbol, color and a bounded conjunction of typed rules. Arbitrary SQL,
scripts and live-state predicates are prohibited. Manual assignment and smart
eligibility are separate: assignment identifies the track's bucket; rules can
add durable Library-only quality gates.

Catalog replacement is optimistic, validates required IDs, order, field values,
unreachable rules and overlap against the current collection. Removed custom
steps migrate safely to `In Progress` rather than orphaning tracks.

### Durable source-change attention

Every promoted trusted-USB analysis compares the incoming projection with the
active Lumi projection before mutation. Lumi records exact attention reasons for:

- metadata;
- waveform;
- beatgrid;
- hot cues;
- source phrases.

Attention is revisioned, source/revision scoped and merged until explicitly
reviewed. Repeated observation cannot silently clear it. A track is effectively
Ready for Show only when its manual status is `Ready for Show` and no unresolved
attention remains.

Lumi-authored phrases stay attached to their authored beat indices. A changed
beatgrid therefore creates a review task; it does not rewrite or discard the
creative timeline.

### Bounded query model

Workflow queues are normal paged Library queries. Counts are calculated with
set-based SQL and the visible page is enriched with at most two bounded queries.
There is no per-row database request and no workflow computation on a live tick.

The first fixed queues are:

- Changed after USB sync;
- Not Started;
- In Progress;
- Ready for Show.

`Changed after USB sync` and `New track versions` remain fixed system queues.
They cannot be renamed or hidden by user workflow rules because they represent
data safety, not personal process.

### Version succession and creative-work reuse

Lumi detects a likely successor from normalized version-family title plus exact
artist identity. Typical suffixes such as `v004` are ignored for family matching.
The editor shows source revision, beat count, BPM delta and duration delta.

Phrase and AutoLoop choices may be copied only when total beat counts match
exactly. Lumi creates a new target timeline revision; it never edits the source
or stretches boundaries. `Reuse Phrases` and `Keep Separate` are durable review
resolutions with an append-only audit record. Resolved pairs leave the system
queue until either track changes again.

### Revision-safe mutations

Status changes and attention resolution use optimistic revisions. Stale UI
actions fail with a typed revision conflict and trigger a fresh snapshot instead
of overwriting newer state.

## Consequences

- DJs get a visible preparation inbox without confusing it with technical
  compatibility.
- A last-minute Rekordbox beatgrid or cue change becomes reviewable immediately.
- Existing Lumi phrases and lighting configuration remain intact across USB
  updates.
- Live performance remains isolated because workflow state is only read and
  written by Library commands and snapshots.
- Configurable steps and version-replacement assistance stay entirely within
  the Library worker and cannot enter the realtime architecture.
