# ADR-0027: USB-backed Pro DJ Link test player and remote control

- Status: **Accepted**
- Date: **2026-08-09**

## Context

Lumi's direct Pro DJ Link integration must be exercised before the physical
CDJ-1500X players arrive. Beat Link Trigger's shallow playback simulator invokes
application listeners internally and does not place a complete player identity
and transport on the network. Running a full DJ application or maintaining a
second simulated Rekordbox database would make tests slow and could produce
identity differences from the media used on the eventual players.

The available Mac mini can be a separate network host. The same Rekordbox USB
can first be synced read-only into Lumi's persistent mirror on the MacBook and
then moved to the Mac mini, matching the intended show workflow.

## Decision

Lumi owns a development-only `lumi-prolink-simulator` tool. It is a narrow test
player rather than a general CDJ emulator. It reads `PIONEER/rekordbox/export.pdb`
and referenced ANLZ files directly from a mounted USB and exposes no file write
operation.

The simulator emits the real Pro DJ Link packet families required by Lumi:

- player keep-alive announcements;
- CDJ status with player number, local USB slot, Rekordbox track ID, transport,
  pitch, master, on-air, beat number and beat within bar;
- beat packets timed from the USB beat grid;
- modern-player PrecisePosition packets at the CDJ-1500X traffic cadence;
- deterministic stale-position bursts followed by the current position.

It deliberately does not emulate audio playback, a display, a jog wheel, NFS,
dbserver, track loading by other players, or any player command receiver. Lumi
resolves metadata, waveform, cues and phrases against its previously synced USB
mirror using the received media identity.

The tool exposes an HTTP control surface on the Mac mini for browser and agent
use. Track listing and every control require a bearer token; the token is either
provided explicitly or generated at startup. Supported controls are load,
play, pause, seek, pitch, master and on-air. The unauthenticated health endpoint
contains no USB or transport data. The control port is LAN-only and must never
be internet-exposed.

Remote control is out-of-band HTTP directed at the development simulator. It
does not weaken ADR-0026's read-only rule for real Pro DJ Link devices.

The default `cdj-1500x` profile emits status at 10 Hz and PrecisePosition at
50 Hz. It periodically sends a short deterministic cluster from five beats
behind, ending with the current position. This does not claim every physical
packet is stale; it reproduces the observed load and worst-case ordering needed
to prove that continuous observations replace rather than queue. The `classic`
profile omits PrecisePosition and is a diagnostic control only.

## Verification contract

- Generated announcement, status, beat and PrecisePosition packets are parsed
  in tests by the pinned beat-link library itself.
- The simulator reports whether each selected track has an exact ANLZ beat grid.
- Invalid USB roots, escaping paths, missing analysis files and invalid
  player/network choices fail closed; a present analysis without a beat-grid
  tag is exposed as a marked synthetic-grid fallback.
- A two-host test must prove discovery and deck observations through the same
  direct bridge used with physical players.
- Performance evidence must use `cdj-1500x` and record packet/burst counters;
  the lower-traffic `classic` profile cannot satisfy a release gate.
- Physical hardware remains the final compatibility and timing authority.

## Consequences

- Direct-input releases can be regression-tested without the decks being present.
- Tests use real Rekordbox IDs and beat grids instead of hand-maintained fixtures.
- The simulator can be controlled repeatably by Codex through a small CLI wrapper.
- Local development requires Java 21 and Maven; the Mac mini uses a generated
  self-contained archive with a minimal Java runtime and needs neither installed.
- One simulated player is sufficient for the first POC. A second instance/player
  is added only when two-host port behavior and master handoff need dedicated
  regression coverage.

## Rejected alternatives

### Full CDJ emulation

Rejected because audio, UI, media serving and player command support do not
increase confidence in Lumi's input and lighting-output use case.

### BLT shallow simulation as the direct-input acceptance test

Rejected because it does not prove network discovery or rich USB identity.

### Copy the USB library into a simulator-owned database

Rejected because it creates sync work and identity drift. The mounted USB is
already the most representative test source.
