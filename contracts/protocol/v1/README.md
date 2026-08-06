# Lumi protocol v1

Protocol v1 is the stable semantic boundary between the autonomous Rust engine
and native clients. It is independent of TCP, Unix sockets, Network.framework,
and process lifecycle.

Every UTF-8 newline-delimited JSON envelope contains the seven fields required
by `envelope.schema.json`. A complete encoded message may not exceed 65,536
bytes. Unknown fields are ignored so optional additions remain backward
compatible within v1. Unknown protocol versions, malformed JSON, invalid
required fields, and oversized input are rejected before application mapping.

`manifest.json` is the reviewable contract index for limits, supported command
and event names, and canonical cross-language fixtures. Field and kind names are
stable English `lowerCamelCase`.

Mutating commands use `messageId` as their idempotency key and include an
expected state or plan revision in their payload. The engine applies a given
command ID at most once. Event sequences are monotonic per connection. A client
that receives a forward sequence gap requests a full snapshot before trusting
incremental state again.

Wire DTOs are mapped into Rust domain and Swift presentation types. They are
never the authoritative runtime state.

State snapshots may include `runtimeCore`, a presentation-safe summary of the
serialized reducer model, health, bounded ingress, processed-event count, and
last structured decision reason. It exposes evidence without leaking Rust
domain types into clients.

The `deckSource` object exposes the product mode (`localPlayback` or
`connectedDecks`), a user-facing display name, provider diagnostics and status.
`leaderDeckId` and `decks` may be null/empty until a real source loads a track.
Clients derive Live and Next from `leaderDeckId`; they never branch on a Beat
Link, Pro DJ Link or future adapter implementation type.
Track metadata uses integer milli-BPM, canonical pitch class and mode,
normalized 24-bit sRGB `colorRgb`, and contiguous beat-based phrases so
snapshots remain deterministic across Rust and Swift. Provider indexes or color
labels never cross the adapter boundary.

`nextPlan` contains the authoritative precomputed plan for the non-leader deck.
Each cue has a contiguous phrase and beat range, origin, lock state,
machine-readable reason, and semantic action. A ready plan also carries one
`themeDecision` with its selected logical Theme, precedence reason, and optional
matched normalized track color. Every concrete cue must match that decision.
`planningOptions` supplies the engine-owned Theme and scene catalog used to
render controls; clients never run planner rules. A plan is either `ready` or
an explicit `fallback`.

Plan mutations carry `planId`, `trackLoadId`, and `expectedPlanRevision`.
Accepted `selectTheme`, `selectScene`, `setCueLock`, and `regeneratePlan`
commands each return a complete authoritative snapshot and increment the plan
revision exactly once. A revision conflict returns a typed error; the client
requests a fresh snapshot before accepting another edit. Plan IDs and seeds are
encoded as decimal strings because protocol v1 clients must not lose 64-bit
integer precision through a JSON floating-point representation.

`selectTheme` records a plan-instance user choice and re-resolves the complete
plan without writing a Theme to library track data. Once a leader change copies
the prepared revision into `activePlan`, later preview mutations cannot affect
that active copy or output until a future explicit safe-boundary workflow.

Each deck also reports `planEligibility`: `readyExact`, `readyTransient` or
`autoHeld`. An unmatched/incomplete connected track may carry an empty phrase
timeline only when it is `autoHeld`; the engine then holds the current look and
does not fabricate analysis. Local Playback adds its audio URI and duration,
while transport updates carry absolute milliseconds and are mapped through the
stored beatgrid by the engine.

`activePlan` identifies the exact immutable plan revision used for the leader.
`outputProvider` reports provider-neutral diagnostics, while bounded
`outputEffects` entries expose scheduled and actual monotonic times, the cue and
semantic action, and an explicit `simulated`, `rejected`, or `skipped` result.
These fields are presentation evidence only; execution remains engine-owned.

Product deck-source, library-load, master-selection and operation mutations
carry `expectedStateRevision`. High-frequency Local Playback transport updates
are instead protected by `trackLoadId`; stale updates fail closed. `timeline`
contains at most 256 ordered engine-owned entries with source, type, monotonic
time, result, and reducer reason. Deterministic simulation commands and the
optional `simulation` payload remain test-only protocol fixtures and are never
exposed by the production Live workspace.
