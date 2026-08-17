# ADR-0032: launchd-owned engine and isolated Ableton Link clock

- Status: **Accepted**
- Date: **2026-08-17**
- Refines: ADR-0003, ADR-0030 and ADR-0031

## Context

The reconnectable engine adapter kept CoreMIDI endpoints stable across UI
sessions, but the engine was still originally created by a foreground app
process. It had no login lifecycle or automatic crash restart. The Link input
also reused transport-generation and discontinuity facts from Lumi's lighting
planner. That made a show-planning correction capable of requesting a Link
phase correction even though Ableton Link and AutoLoop selection are parallel
SoundSwitch inputs with different responsibilities.

Presentation, show execution and clock publication must not be one failure or
control domain.

## Decision

### Per-user service ownership

Each release channel bundles one non-privileged LaunchAgent and registers it
with `SMAppService`:

- Dev: `co.victorblan.tech.lumi.dev.engine` and `Lumi Dev` data;
- RC: `co.victorblan.tech.lumi.rc.engine` and `Lumi RC` data;
- Prod: `co.victorblan.tech.lumi.engine` and `Lumi` data.

The plist is stored at `Contents/Library/LaunchAgents` and resolves the engine
with `BundleProgram`. The standalone Rust executable carries its own embedded
Info.plist. No root helper, installer script or shared database is required.

`launchd`, not SwiftUI, owns start, KeepAlive restart and login availability.
The app authenticates over a loopback endpoint using a channel-local token and
attaches only when version, build, executable path and SHA-256 identity match.
Upgrades unregister the old job before registering the new bundled job. A UI
disconnect still makes the engine fail safe to Off and leave Link, but the
inactive engine and stable CoreMIDI endpoints remain available.

Local ad-hoc signatures have no stable Team ID. If macOS temporarily rejects
the first upgraded helper against a cached launch constraint, the supervisor
waits for Background Task Management to invalidate that record and performs
one bounded re-registration in the same app start. Developer ID builds retain
the normal single-registration path.

The discovery token and record are owner-only files. The engine publishes its
record atomically only after binding its listener, and removes it on exit only
when it still owns the recorded PID. A restarted service can therefore never
erase its successor's discovery record.

### Three independent runtime planes

```text
Deck clock --------> LinkClockObservation ------> Link adapter --> Carabiner --> SoundSwitch Link

Exact position ----> phrase/plan authority -----> realtime MIDI lane ---------> SoundSwitch AutoLoop

Read-only state ---> snapshot/display clocks ---> macOS / future iPhone UI
```

The Link adapter accepts only clock-domain facts:

- source kind and selected deck;
- effective BPM;
- beat within the four-beat quantum;
- source playing state;
- monotonic observation time.

Its public input contains no track position, phrase, Theme, Bank, AutoLoop,
Hot Cue, lighting operation state, output deadline or show-generation field.
Those facts physically cannot request a Link phase command.

Link follows the selected deck's transport in both Pro DJ Link and Local
Playback. `Off`, `Arm`, `Start` and `Pause` gate lighting output only. Tempo
updates preserve the running Link timeline. A hard Link alignment is limited
to Link's own initial acquisition, timing-source/master handover and a genuine
stopped-to-playing transition. Independent future clock-discontinuity support
must be introduced as a clock-provider contract and may not reuse a lighting
generation.

Carabiner remains a separately executed, pinned and replaceable helper. It is
supervised by the engine's timing adapter, while the realtime MIDI worker owns
no Carabiner handle and Link owns no MIDI sender.

## Failure and recovery behavior

- UI Quit: engine remains; lighting becomes Off; Link helper exits; MIDI
  endpoints remain stable.
- UI relaunch: app attaches to the same PID and current snapshot.
- engine crash: launchd starts a replacement; the UI detects the stale socket
  and automatically attaches to the replacement record.
- Link helper failure: Link degrades/restarts within its own adapter; pending
  AutoLoop MIDI is not cancelled or rescheduled by that helper lifecycle.
- MIDI/output failure: Link clock publication is not stopped or re-anchored.
- incompatible app/service identity: fail safe and perform a controlled
  service handover rather than attaching across versions or channels.

## Consequences

- SwiftUI frame rate, app switching and window lifecycle are outside both
  integration streams.
- A phrase or Hot Cue plan change can trigger the correct AutoLoop without
  scrubbing SoundSwitch's running Link timeline.
- Link may be diagnosed and replaced independently from the show scheduler.
- Local unsigned builds can register this per-user agent, while public direct
  distribution still has the existing Gatekeeper/notarization limitations.
- Login Items approval must be surfaced when macOS requires it.

## Acceptance

1. Installed Dev registers a launchd-managed agent with no administrator
   rights and preserves the existing Dev database.
2. UI Quit/relaunch retains the engine PID and launchd run count.
3. Killing the engine produces a new launchd PID and the open UI reconnects.
4. Lighting operation and show-generation changes produce no Link hard
   re-anchor.
5. Continuous tempo observations update Link BPM without a phase command.
6. Initial acquisition, source handover and stopped-to-playing each emit at
   most one phase alignment.
