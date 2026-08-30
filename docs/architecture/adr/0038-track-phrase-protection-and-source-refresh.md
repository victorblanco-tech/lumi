# ADR-0038 – Track phrase protection and source refresh

Status: Accepted · 2026-08-30

## Context

Lumi phrase points, phrase roles and per-phrase Autoloop choices are valuable
creative data. A finished track needs protection against an accidental editor
gesture, history restore or inappropriate source reconciliation. At the same
time, trusted USB analysis remains authoritative for technical source facts such
as beatgrid, waveform and hot cues. Protecting creative data must therefore not
freeze a track's source analysis.

## Decision

- Phrase protection is a per-track, revisioned Library fact with optimistic
  concurrency. It is independent from preparation status and technical
  readiness.
- The engine, not only the macOS UI, rejects phrase edits, boundary moves, role
  changes, Autoloop-choice changes, undo/redo, revision restore, phrase reuse and
  source reconciliation while protection is active.
- Trusted USB analysis promotion remains permitted. It can update beatgrid,
  waveform, hot cues and source phrases, while Lumi-authored phrase data is
  preserved on its beat coordinates and an explicit workflow-attention record
  is created for review.
- Unlocking is an explicit user action. No import or workflow transition may
  silently unlock a track.
- Empty workflow queues are normal product states. A scoped query failure never
  replaces an already healthy Library snapshot with a global error screen.

## Consequences

The user can safely mark mature phrase work as protected without missing later
beatgrid corrections. Live planning continues to read the same immutable
snapshot structures; this feature adds no work to Pro DJ Link, Ableton Link or
MIDI execution lanes.

