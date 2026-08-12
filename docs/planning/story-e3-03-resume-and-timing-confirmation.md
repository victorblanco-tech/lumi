# Story E3-03 – deterministic resume and confirmed lighting timing

Target milestone: **0.4.0 – next development cycle**
Status: **In progress**

## Delivered in `0.4.0-dev-1`

- an offset requested while the Live leader is playing remains engine-owned
  pending state and cannot shift the active phrase;
- the pending value activates only on the next authoritative, actually playing
  leader phrase-boundary; pause and paused seek do not activate it;
- engine snapshots keep applied and pending values separate;
- Live shows `APPLIED`, `NEXT` or `SYNC`, and both deck headers retain the
  applied value with an optional next-phrase value;
- deterministic Rust and Swift regressions cover the boundary and presentation.

The deterministic pause → cue/seek → play reconciliation and engine-owned
pending/applied timing acknowledgement are implemented and remain in the local
regression gate. Bounded client retry after a command-busy/reconnect path and
repeat physical SoundSwitch/DMX acceptance remain.

## Physical finding

The 2026-08-09 `0.3.0-rc.1` lighting run was accepted end to end and proved
parallel Control One feedback. It also exposed two bounded follow-up items:

1. pause → cue/seek → play can intermittently leave automatic lighting silent;
   a later state change can recover it;
2. a Live timing change persists locally and is sent to the engine, but the UI
   does not prove whether that exact value was accepted and applied.

## Required behavior

- resume after pause establishes exactly the correct current cue when needed;
- cue/seek while paused never emits skipped cues or a burst of output;
- the current bank and AutoLoop are safely reasserted at resume when output
  continuity requires it;
- ordinary pause/play without a cue change does not duplicate a valid active
  AutoLoop;
- switching Library ↔ Live cannot suppress the resume reconciliation;
- the desired timing offset and engine-applied timing offset are modelled
  separately;
- Settings shows one central applied, pending or failed confirmation;
- both deck surfaces show the applied value as a small, non-shifting technical
  detail;
- a timing change during playback affects subsequent cue scheduling and never
  retroactively replays a cue;
- retries are bounded and cannot block or delay real-time lighting output.

## Evidence

- deterministic engine regression for pause → seek/cue → resume;
- regression for ordinary pause → resume without duplicate output;
- command-busy/reconnect test proving desired/applied timing reconciliation;
- Swift presentation tests for pending/applied/failed and stable deck layout;
- repeat physical SoundSwitch/Control One/DMX run at a clearly visible offset,
  followed by the intended fine-tuning range.

## Safety

Lighting execution remains engine-owned. UI acknowledgement and retry work may
not move timing responsibility to Swift or introduce blocking work on the
real-time output path.
