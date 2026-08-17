# Story E5-00: Representative CDJ-1500X simulator traffic

- Status: **Implemented and packaged locally; two-host verification pending**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-53`
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

## Local evidence

- generated PrecisePosition packets are exactly 60 bytes and round-trip device,
  track length, playback milliseconds, pitch and effective BPM through
  beat-link;
- 14 simulator unit/packet tests pass;
- the complete simulator verification and shaded-JAR build pass;
- the Apple Silicon DMG passes checksum, bundle-signature and architecture
  verification and identifies itself as build `0.4.0 (53)`.

Two-host discovery, sustained packet counters and the one-hour engine soak are
separate gates. Passing this story proves the traffic generator, not Lumi's
consumer refactor.
