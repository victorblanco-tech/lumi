# Demo control and event timeline

Epic 1 exposes the complete license-safe demo through versioned protocol v1
commands. The macOS client never mutates simulator or operation state locally.
Every command returns a complete authoritative snapshot.

Session mutations include `expectedStateRevision`. A stale client receives a
typed `stateRevisionMismatch` and refreshes before allowing another command.
The supported controls are:

- load or reset the canonical demo without restarting the engine;
- select deterministic 1x, 4x, 16x, or 64x simulation speed;
- pause or resume simulated playback;
- advance the simulated monotonic clock in bounded increments;
- change the Live leader to the other deck;
- transition through OFF, ARMED, LIVE, and PAUSED using the domain reducer.

The native app advances a running demo in 250-tick commands. This keeps test
time deterministic and avoids putting a wall-clock dependency in the domain.
Changing speed changes only when events arrive, never their semantic order.

## Timeline

The pure domain records the latest 256 processed events. Each entry has a
strictly increasing sequence, monotonic timestamp, provider-neutral source and
event type, explicit result, and the reducer decision reason. Snapshots expose
the bounded list for diagnostics; the client does not reconstruct events from
view actions.
