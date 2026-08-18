# Story E5-03: Explicit playback epochs, hotcues and output offset

- Status: **Implemented and simulator/UI verified in `0.4.0-dev-55`**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-55`
- GitHub tracking: [#119](https://github.com/victorblanco-tech/lumi/issues/119)

## Outcome

Start during playback, pause/resume, master handoff, hotcue, seek and beatjump
select the correct landing cue exactly once and in beat, without affecting the
independent Link timeline beyond a confirmed transport boundary.

## Scope and acceptance

- create a new playback epoch only for a confirmed user-visible transport
  boundary;
- cancel only not-yet-emitted cue work from the previous epoch;
- reselect the current cue once on Start/resume when required;
- land hotcue/seek/beatjump on the confirmed phrase and eligible beat;
- add negative/positive offset as a replaceable future deadline;
- allow BPM changes to move an unsent deadline, then freeze it after emission;
- run a deterministic 100-action matrix with zero wrong or duplicate cues.

## Dev-55 implementation evidence

- the execution identity now carries an explicit playback epoch and one of the
  confirmed causes `OperationStart`, `PlaybackStarted`, `PositionLanding`,
  `MasterHandoff` or `TrackLoad`;
- pause/stop and a new execution epoch cancel only not-yet-emitted work;
  completed output remains immutable;
- Start during playback, resume, forward/backward seek and same-phrase Hot Cue
  landings each reassert the current configured cue exactly once;
- a negative timing offset pre-schedules the next cue, while a positive offset
  delays it; a BPM update may replace only an unsent deadline and a running
  preference change becomes active at the next phrase boundary;
- the deterministic 100-action reducer and executor matrices complete with 100
  triggered cues, 100 completed cues, zero failures and no duplicate MIDI;
- the network acceptance test covers stopped-to-playing, Pause/Start, forward,
  backward and same-phrase landings against the representative CDJ-1500X
  simulator and observes exactly one output per confirmed action;
- the 60-second optimized realtime MIDI soak records 2,165 scheduled actions,
  2,052 emitted actions, 113 intentional generation cancellations, zero queue
  saturation, p95 10.036 ms and p99 10.048 ms;
- headed macOS acceptance exercised Arm, Start, Pause/resume, same-phrase seek,
  live BPM change, deferred offset activation and foreground switching in the
  actual Lumi and SoundSwitch UIs. SoundSwitch retained one Link peer, followed
  `155.000 -> 161.510` BPM and remained responsive to AutoLoop selection;
- the complete technical, functional and Apple application gates pass.

Physical CDJ-1500X, SoundSwitch and DMX duration evidence remains E5-04 scope;
it is not substituted by the simulator or desktop acceptance above.
