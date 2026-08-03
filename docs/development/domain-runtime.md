# Domain runtime

`lumi-domain` owns Lumi's authoritative runtime model. It is a pure Rust crate
without async runtime, serialization, platform, provider, filesystem, network,
MIDI, or UI dependencies.

## Flow

Every source observation, user command, and effect result enters a bounded
`BoundedEventQueue`. `SerializedRuntime` removes one event at a time and passes
the current immutable state plus that event to the pure `reduce` function. Only
the returned state becomes authoritative.

```text
bounded ingress -> pure reducer -> state + decision + effects
                                      |
                                      +-> effect results re-enter as events
```

Effects are semantic requests. The reducer never performs I/O. Adapter workers
will execute effects and return typed results in later increments.

## Invariants

- Source, command, and effect sequences are monotonic per producer.
- Duplicate or older sequences never repeat a mutation.
- State revisions advance only for accepted material state changes; ignored
  duplicates and stale input still increase the separate processed-event count.
- Playback observations apply only to the exact current `TrackLoadId`.
- Plan revision 1 is initial; later accepted revisions increase exactly by one.
- A plan applies only to the deck, track, and track-load instance it was built
  for.
- Runtime decisions use injected `MonotonicTime`; wall-clock time stays at the
  wire/logging boundary.
- Expected conflicts and invalid transitions return typed errors.
- Diagnostics and queue storage are bounded.

## Queue saturation

The main event queue has a fixed capacity. A critical event can evict a queued
non-critical observation, but is never discarded silently. Every saturation
also occupies one reserved diagnostic slot. That diagnostic is reduced before
normal work, marks runtime health degraded, closes the logical output gate, and
produces `EnsureOutputClosed`. If a queue contains only critical events, ingress
returns a typed saturation error to the caller.

## Current visible evidence

On startup the engine submits `RuntimeStarted` through the same queue and
reducer. The protocol snapshot exposes a presentation-safe `runtimeCore`
summary. The macOS workspace shows its model, health, queue usage, state
revision, processed-event count, and latest structured decision. Domain structs
remain private to Rust and are not wire DTOs.

The application planning worker processes a Next `TrackLoaded` observation,
generates outside the reducer, and re-enters the result as `PlanGenerated`.
That effect is fully reduced before the following leader event is accepted, so
the complete plan exists before the transition path can use it.

The operation transition table exists in the domain core, but the visible
controls and output effects remain disabled until simulator preflight and dry-
run execution are implemented in E1-06 through E1-10.
