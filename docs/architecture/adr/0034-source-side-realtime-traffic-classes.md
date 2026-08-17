# ADR-0034: Source-side realtime traffic classes and end-to-end freshness

- Status: **Accepted**
- Date: **2026-08-17**
- Refines: ADR-0027, ADR-0031, ADR-0032 and ADR-0033

## Context

The Link and MIDI providers were separated, but physical CDJ-1500X testing
showed increasing delay in Live Decks, AutoLoop selection and BPM relay. The
Java Pro DJ Link bridge queued every callback in one 4,096-item FIFO. Rust
coalesced continuous values only after JSON serialization and pipe delivery.
The old simulator emitted status and beats but not the modern player's
high-frequency PrecisePosition traffic, so component tests could not reproduce
the backlog. Final-provider latency remained low while source facts became old.

## Decision

Traffic is classified at the first Beat Link callback boundary:

1. critical ordered events: device/load/master/play/exact beat and confirmed
   discontinuity;
2. latest tempo state: one CdjStatus-derived BPM value per deck;
3. latest display/position state: one current position/status value per deck.

Continuous state uses replacement mailboxes, never FIFO history. Critical
events use a small bounded ordered queue and fail closed on saturation. Each
fact carries source observation time through its consumer, and diagnostics
report current/maximum age in addition to depth, coalescing and saturation.

Transport/AutoLoop, Link and display are independent consumers. AutoLoop may
not wait behind tempo, position or UI work. Link may not consume phrases,
lighting operation or old BPM values. Display can drop intermediate values and
interpolates only from the newest immutable anchor.

The default simulator profile mirrors this boundary with 10 Hz status, 50 Hz
PrecisePosition, exact beats and deterministic stale-position bursts. The
classic profile is not valid release-performance evidence.

## Consequences

- a current value can replace an unconsumed continuous value without being
  considered data loss;
- critical overflow is visible degradation, never a late replay;
- provider-only dispatch latency no longer qualifies as end-to-end evidence;
- simulator soaks precede short physical protocol confirmation;
- the production engine remains one launchd service, but its three integration
  consumers no longer share scheduling or backpressure.

## Rejected alternatives

- increasing the Java FIFO: delays failure and permits a larger stale backlog;
- coalescing only in Rust: serialization and pipe backlog have already occurred;
- using UI smoothness as timing evidence: rendering is not an integration clock;
- treating physical day-long testing as the primary development loop: it is
  slow, non-deterministic and previously missed the unsupported simulator load.
