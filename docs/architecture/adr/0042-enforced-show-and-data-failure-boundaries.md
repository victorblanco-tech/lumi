# ADR-0042: Enforced show, presentation and data failure boundaries

- Status: **Accepted; implementation in progress**
- Date: **2026-09-05**
- Target: **0.6.2 / Lumi Remote 0.1.1**

## Context

The audit of `8513ca6` found that separate processes/dependency rules alone did
not enforce ADR-0040. Remote projection failures propagated through the engine
pump, desktop I/O could hold that pump, and synchronous database operations
shared its task. Stream reads also discarded partial frames on cancellation.
Additional findings concern USB identity, restore migrations, command outcome
tracking and stale Remote status.

## Decision

1. Presentation is a fallible consumer, never an engine failure authority.
   A bad Remote projection publishes unavailability and can recover on a later
   valid state without changing operation, MIDI, Link or the authoritative plan.
   The two-Player Remote view prioritizes the active and next plans, then fills
   empty positions deterministically. Extra discovered Players remain valid in
   the engine. Display text is normalized only at the presentation boundary;
   Library metadata and beatgrids are never silently rewritten for the phone.
2. Each stream has one persistent, byte-bounded reader. Cancellation retains
   partial bytes; oversize and truncated frames are rejected, never parsed as
   partial messages. No `read_until` allocation may precede the size boundary.
   The shared framing utility knows nothing about domain commands or schemas.
3. Client authentication, socket writes, SQLite and heavy projection generation
   must not hold the show-event pump. Data changes are prepared separately and
   applied with state/track/plan revision validation. The engine remains the
   sole writer of show state. Queue limits and deadlines are explicit.
4. A command ID identifies its actual pending/applied/rejected outcome, not
   merely admission. Expired or revoked work cannot execute later. Deadlines
   use local monotonic time, not a phone's wall clock.
5. Physical USB identity is authoritative; equal labels or tracksets never
   authorize deleting a source. Live matching carries evidence of media/track
   provenance. An older backup is migrated and validated in staging before
   activation, with coherent rollback on failure.
6. Client connected state is not proof of current engine availability. Remote
   keeps the three integration indicators visible, marks stale data honestly
   and requires a fresh snapshot before controls become active.

## Verification and rollout

Epic 10 tracks implementation; these decisions are not evidence of completion.
Regression tests run through production readers, engine processes and client
models, not unused queue prototypes. Adversarial tests cover fragmented input,
slow clients, extra Players, old backups, media ID collisions, SQLite contention
and late/repeated commands. Performance evidence covers source event receipt to
actual MIDI submission/emission, including concurrent UI/data work.

Keep the accepted waveform visuals and tempo-only Ableton Link algorithm.
Prefer small verified changes at ownership boundaries over a broad rewrite.
Do not change the current explicit desktop-exit parking policy as a side effect
of moving client I/O out of the pump.

## Live plan edit commit boundary

The synchronous sole-writer path first prepares the edited timeline, Library
context and materialized plan without changing active data. It reduces the plan
against a candidate runtime state and requires an explicit `PlanAccepted` result.
Only then does it append the timeline using SQLite's expected-head transaction.
After that durable write, publication of the prepared state, context, variation
reservation and modifier plan is infallible; it performs no external MIDI action.
An error before commit leaves both the persisted timeline and the live plan intact.

This staging boundary is also required when preparation moves off the show task.
The current synchronous implementation must not be mistaken for that scheduling
isolation: the future asynchronous apply step must revalidate track load, plan,
timeline, catalog/policy revisions and protection before publishing any result.
