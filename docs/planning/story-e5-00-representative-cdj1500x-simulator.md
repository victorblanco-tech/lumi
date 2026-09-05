# Story E5-00: Representative CDJ-1500X simulator traffic

- Status: **Implemented with playlist Auto Mix and deterministic recovery faults; two-host verification pending**
- Priority: **P0 Critical**
- Simulator target: `0.4.0-dev-56`
- GitHub tracking: [#121](https://github.com/victorblanco-tech/lumi/issues/121)

## Outcome

Lumi can reproduce the sustained and bursty Pro DJ Link input observed from a
physical CDJ-1500X without leaving the decks powered on.

## Scope and acceptance

- emit real Beat Link `PRECISE_POSITION` packets at 50 Hz;
- retain status at 10 Hz and exact beat packets from the USB beat grid;
- emit deterministic bursts containing stale positions followed by the current
  position, so latest-value behavior is testable;
- make `cdj-1500x` the default and mandatory release-performance profile;
- retain `classic` only as a low-traffic simulator control;
- expose profile, cadence, packet counters, burst count and last error over the
  authenticated status API and browser UI;
- provide an authenticated manual burst trigger;
- parse every generated packet family back through pinned beat-link 8.0.0;
- package a self-contained Apple Silicon simulator app for the Mac mini.
- publish two independently controlled player identities from one simulator;
- provide bounded per-player loops and a deterministic Auto Mix mode which
  alternates exclusive Master/On Air ownership for unattended soak tests.
- read the USB playlist tree and let Auto Mix preload a different track from a
  selected playlist onto the idle player before each handoff.
- deterministically suppress exact position, timing or all traffic without
  changing the authoritative Player clock;
- expose Player leave/join, Master handover, Hot Cue and beat-jump actions;
- run a repeatable Recovery Soak sequence alongside playlist Auto Mix.

## Local evidence

- generated PrecisePosition packets are exactly 60 bytes and round-trip device,
  track length, playback milliseconds, pitch and effective BPM through
  beat-link;
- 28 simulator unit/API/packet tests pass, including authenticated HTTP controls, loop wrapping, playlist
  rotation, deterministic fault expiry and Recovery Soak state transitions;
- the complete simulator verification and shaded-JAR build pass;
- the Apple Silicon DMG passes checksum, bundle-signature and architecture
  verification and identifies itself as `0.4.0-dev-56`;
- headed validation against a 1,197-track Rekordbox USB loaded separate tracks
  on Players 1 and 2, wrapped Player 1's 10–15 second loop repeatedly and
  completed three automatic exclusive-Master handoffs in 16 seconds.
- headed `0.4.0-dev-56` validation read 82 nested playlists from the same USB,
  selected a four-track playlist, automatically loaded both players and
  completed repeated five-second shuffled handoffs while preloading the idle
  player with a different track.

Two-host discovery, sustained packet counters and the one-hour engine soak are
separate gates. Passing this story proves the traffic generator, not Lumi's
consumer refactor.
