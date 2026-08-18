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

Transport/AutoLoop, Link and display are independent consumers. They remain in
one launchd-owned engine, but have separate bounded queues/actors and failure
state. The 5 ms engine coordinator always forwards fresh Link clock state
before synchronous library hydration or lighting-plan reduction. AutoLoop may
not wait behind Link provider I/O or UI work. Link may not consume phrases,
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
  consumers no longer share output scheduling or backpressure; one small
  priority coordinator remains the process-level handoff point by design.

## Implementation record — `0.4.0-dev-54`

The bridge implements a 256-event critical FIFO and latest-value mailboxes per
deck for tempo, transport and display. The envelope exposes `trafficClass` and
`bridgeQueueAgeMicros`. Rust preserves the lane priority and never coalesces
critical messages. Link receives a dedicated `tempoStatus`, rejects an older
source timestamp and calls its provider only when deck, BPM or play state
materially changes. The engine-owned integration pump runs independently of
SwiftUI at 5 ms.

Automated evidence covers 50,000 replaceable samples, exact simulator pitch
ramps, stale bursts, forward/backward landings and full functional/technical
regression gates. Headed evidence covers the installed native app plus the real
SoundSwitch interface. Physical one-hour evidence remains a release gate rather
than an architectural assumption.

## Implementation record — `0.4.0-dev-56`

The Rust supervisor now combines source-side bridge queue age with its own
bounded queue residence and retains a constant-space histogram with p50, p95,
p99 and maximum age. These values are exposed through the engine snapshot and
native Integration diagnostics.

Within each 5 ms coordinator turn, freshly decoded clock observations are
forwarded to the isolated Link actor before any SQLite-backed track hydration
or planning work. Authoritative position processing may then feed the
Transport/AutoLoop path, and a second empty-or-discontinuity-only drain catches
new clock acquisition without duplicating the first. UI polling is downstream
of the immutable snapshot and cannot become a source clock.

The configurable release soak exercises four explicit modes: Pro DJ Link-only,
Link-only, realtime MIDI-only and all lanes combined with 40 Hz snapshot
polling, pitch changes, seeks and lighting Pause/Start cycles. It emits one
bounded JSON evidence artifact without track metadata or credentials.

## Implementation record — `0.4.0-dev-57`

Device discovery and deck transport have separate lifecycle semantics. A CDJ
joining the network may temporarily advertise a device while its loaded-track
BPM, source player and beat are still sentinel values. The bridge withholds
those incomplete realtime facts instead of converting normal player warm-up
into a process-wide protocol failure. Already healthy devices and their Link
and AutoLoop consumers continue uninterrupted.

A genuine supervised bridge restart resets only bridge-session state. The
provider object, source ID sequence and track-load allocator survive, because
the domain reducer intentionally remembers the last sequence for that source.
This preserves fail-closed unload semantics while allowing rediscovered decks
to be accepted and hydrated automatically after recovery.

## Rejected alternatives

- increasing the Java FIFO: delays failure and permits a larger stale backlog;
- coalescing only in Rust: serialization and pipe backlog have already occurred;
- using UI smoothness as timing evidence: rendering is not an integration clock;
- treating physical day-long testing as the primary development loop: it is
  slow, non-deterministic and previously missed the unsupported simulator load.
