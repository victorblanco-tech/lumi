# ADR 0048 — read-only Library projection lane

Date: 2026-10-09
Status: accepted; incremental implementation, not a declaration of complete timing isolation.

## Context

The integration pump and desktop command reducer share one runtime owner.
Although desktop sockets and MIDI dispatch are isolated, constructing a full
Library response still performs synchronous SQLite reads on that owner. The
dev-21 combined soak measured Library round trips up to 21 ms and pump lateness
up to 53 ms. Those are not end-to-end lighting latency measurements.

## Decision

Move full Library snapshot reads for browsing and explicit snapshots to a
blocking worker with its own read-only SQLite connection. Capture the selected
query, editor and pending presentation state as owned values. Read the database
inside one consistent transaction; never seed, migrate or import on this lane.
In-memory test libraries retain the synchronous path.

Only one desktop projection is in flight. While awaiting it, the runtime must
continue Pro DJ Link processing, due AutoLoop servicing, Remote publication and
Remote commands. Desktop requests remain serialized. A Remote library mutation
invalidates an outstanding projection by revision rather than publishing stale
Library data. The response uses fresh transport state after the read completes.
Read failures return an actionable command error, not engine termination.
There is no stale-result cache and no change to USB identity or matching policy.

## Boundaries and remaining work

This removes repeated presentation reads, not every synchronous operation.
Library mutations, initial bootstrap, editor opening, waveform extraction and
source startup need separate measurements and, where necessary, subsequent
worker boundaries. Never label all integration scheduling isolated on the
strength of this change. Keep Ableton Link tempo-only and AutoLoop output sparse;
do not add continuous beat/phase correction.

## Verification

Regression coverage must compare detached and direct projections with a real
temporary database, including query/editor identity. Measure the same combined
pitch/seek/operation/library workload before and after. Desktop acceptance must
include rapid playlist/search changes during Start and clean shutdown. No Main
release until these and the remaining Epic 11 gates pass.
