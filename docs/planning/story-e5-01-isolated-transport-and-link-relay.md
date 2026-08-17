# Story E5-01: Isolated Transport and Ableton Link Relay

- Status: **Done; physical CDJ/Wi-Fi soak remains in E5-04**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-50`
- Components: Pro DJ Link, Engine, Ableton Link
- GitHub tracking: [#117](https://github.com/victorblanco-tech/lumi/issues/117)

## User outcome

While the master CDJ plays, SoundSwitch follows every effective BPM change in
real time through Ableton Link. Lumi lighting Off, Arm, Start and Pause have no
effect on that clock.

## Scope

- extract all Link provider state and freshness policy from the MIDI/lighting
  `OutputWorker` into an isolated Link Relay;
- retain CdjStatus as the only effective-tempo authority;
- use Beat packets for phase observations without accepting their BPM as a
  competing authority;
- forward playing master BPM changes immediately and preserve Link phase;
- never expose phrase, plan, Bank, AutoLoop or lighting-operation state to the
  Link contract;
- remove Link hold behaviour from Lumi Off/Pause commands;
- forbid implicit helper reconnect after an active-session failure;
- retain explicit user enable/disable and fail-closed source-loss behaviour.

## Acceptance criteria

- a playing CdjStatus BPM change immediately produces a Link clock observation;
- later PrecisePosition and Beat packets cannot overwrite that BPM;
- a sequence of master BPM changes reaches the Link provider without Hold,
  Stop, phase-correction or republish calls;
- Off, Arm, Start and Pause do not invoke the Link provider;
- normal beat jitter produces exactly one initial Link alignment;
- helper failure cannot create a second peer until explicit disable/enable;
- existing Carabiner lifecycle, provider and engine regression suites pass;
- local build and version consistency gates pass.

## Out of scope

- changing the current automatic AutoLoop executor;
- negative output offset;
- hotcue/seek cue selection;
- physical one-hour release evidence.

Those are delivered in E5-02 through E5-04.

## Dev-50 implementation evidence

- Link Relay state and freshness policy no longer live in the lighting/MIDI
  `OutputWorker`;
- Link observations have no operation, phrase, plan, Bank, AutoLoop or MIDI
  fields;
- Off/Arm/Start/Pause no longer hold the independently enabled Link relay;
- playing CdjStatus tempo changes are forwarded immediately, while Beat and
  PrecisePosition retain the last CdjStatus tempo instead of competing with it;
- continuous Link updates change BPM with phase preserved;
- a failed active helper session remains degraded and cannot reconnect until
  explicit disable/enable;
- the functional and technical local gates pass;
- the real bundled Carabiner lifecycle tests pass, including `130 -> 140 BPM`
  and zero helper left after drop;
- the signed-local package verification produced the Apple Silicon dev-50 DMG;
- the packaged app was installed in `/Applications/Lumi/Dev` and exercised
  through the real macOS UI against a running SoundSwitch session;
- UI enable/disable left and rejoined the Link session without restarting
  SoundSwitch;
- Local Playback of `90s Bitch` changed SoundSwitch from `140.0` to `155.0`
  BPM through the production Link path;
- Off, Arm, Start, Pause and Off were exercised through the visible controls;
  all transitions retained exactly one Carabiner helper and did not interrupt
  the independently enabled Link relay;
- Cmd-Q removed the owned Carabiner helper while the intended launchd-owned
  engine remained available.

Physical CDJ pitch movement, Pro DJ Link over Wi-Fi and the combined one-hour
soak remain explicit E5-04 release evidence. They are not silently claimed by
this Local Playback UI test.
