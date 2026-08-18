# Story E3-03 – deterministic resume and confirmed lighting timing

Target milestone: **0.4.0 – next development cycle**
Status: **Implemented; physical acceptance pending**

## Delivered in `0.4.0-dev-32`

- a Live Deck that changes from stopped to playing reasserts the current cue
  once, even when cued inside the track, while duplicate playing observations
  remain side-effect free;
- Pause still closes output deliberately, and Start now restores the current
  AutoLoop rather than waiting for a later phrase;
- the macOS controls present the requested valid operation state immediately
  while the revision-safe engine acknowledgement completes;
- deterministic domain and engine regressions cover initial play, transport
  restart, operational Pause/Start, repeated packets and paused seek/resume.

## Delivered in `0.4.0-dev-31`

- entering Start while the current Master is already playing executes that
  track's current planned phrase exactly once instead of waiting for a later
  phrase change;
- an unprepared direct-deck Start settles the Bank non-blockingly and emits the
  AutoLoop on the first safe exact beat;
- repeated transport packets remain deduplicated, while a real playback restart
  or operational Pause/Start restores the current AutoLoop;
- the user-facing timing sign is now temporal and unambiguous: negative sends
  early, zero targets the boundary and positive sends late;
- the former positive-early preference is migrated once without changing the
  user's physical compensation;
- pending timing changes are used to schedule the very next phrase transition,
  then become the acknowledged applied value at that boundary.

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
- ordinary repeated playing observations do not duplicate output; a true
  stopped-to-playing generation and Pause-to-Start do reassert output because
  the preceding stop/pause may have closed SoundSwitch playback;
- switching Library ↔ Live cannot suppress the resume reconciliation;
- the desired timing offset and engine-applied timing offset are modelled
  separately;
- Settings shows one central applied, pending or failed confirmation;
- both deck surfaces show the applied value as a small, non-shifting technical
  detail;
- a timing change during playback affects the next cue scheduling and never
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
