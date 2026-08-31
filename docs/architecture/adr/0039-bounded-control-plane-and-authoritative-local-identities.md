# ADR-0039 – Bounded control plane and authoritative local identities

Status: Accepted · 2026-08-31

## Context

Lumi's show-critical Pro DJ Link, Ableton Link and SoundSwitch lanes are already
separated from SwiftUI. A codebase review after the first public beta found
three remaining reliability boundaries around that core: UI-to-engine requests
could wait forever, an obsolete service record identified a process only by
PID, and removable-media migration could choose a trusted identity from a
display label instead of current physical evidence.

The production engine also retained silent Beat Link Trigger and legacy
Rekordbox XML paths after the product had standardized on direct Pro DJ Link
and read-only OneLibrary USB sources. Those paths increase behavior and test
surface without representing a supported user workflow.

## Decision

- Every local control-plane operation has a bounded deadline. A timeout closes
  that connection, releases queued callers and starts the existing supervised
  reconnect path. Long data work runs outside the realtime engine.
- A stale service record is metadata, not proof of process ownership. Lumi may
  signal a recorded PID only after verifying the running executable path and
  expected service identity.
- Current physical USB evidence is authoritative. Display-name matching is
  limited to a deliberate legacy migration path; an ordinary stable identity
  is never silently rebound by label.
- Connected Decks means direct Pro DJ Link. Failure of that provider is visible
  and fail-closed. Beat Link Trigger may remain in historical tests or isolated
  development tooling, but not as a production runtime fallback.
- Rekordbox XML and closed local-database imports are retired from the product
  protocol and UI. Supported ingestion is OneLibrary USB, read-only and
  isolated from the show runtime.
- SQLite has an explicit bounded contention policy. Only the data lane may
  retry a transient busy state; a show-critical lane never waits on SQLite.

## Consequences

- UI failure is recoverable instead of looking like an indefinitely frozen app.
- Process replacement can leave stale metadata behind rather than risk
  terminating an unrelated process.
- A reformatted or truly replaced USB can require explicit authorization or
  migration, which is safer than silently merging source histories.
- Removing retired providers shrinks the runtime and makes diagnostics describe
  the path that is actually active.
- Existing domain, MIDI and planning contracts remain unchanged, so 0.5.2 is a
  compatible patch release.
