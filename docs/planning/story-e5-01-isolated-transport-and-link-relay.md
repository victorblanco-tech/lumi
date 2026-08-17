# Story E5-01: Isolated Transport and Ableton Link Relay

- Status: **Implementation complete in dev-54; physical soak evidence pending**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-54`
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
- classify traffic at the Java callback boundary before serialization;
- replace continuous status/position FIFO history with latest-value mailboxes;
- give Link its own latest-master-tempo consumer, independent from exact beats,
  AutoLoop scheduling, display projection and UI commands;
- reject a BPM observation older than the last applied source timestamp;
- expose source-to-Link age, replacement count and critical saturation.

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
- a one-hour CDJ-1500X-profile pitch-ramp soak converges to every final BPM
  within 150 ms, never returns to an older value and shows no increasing age.

## Out of scope

- changing the current automatic AutoLoop executor;
- negative output offset;
- hotcue/seek cue selection;
- physical one-hour release evidence.

Those are delivered in E5-02 through E5-04.

## Dev-50 component evidence

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
- Live Decks discovered simulated Pro DJ Link Player 1 as the playing master
  with `90s Bitch` at `155.000` BPM and Player 2 as paused/Plan Ready;
- the Live header reported `Pro DJ Link · deck 1 · 155.000 BPM · 1 peer` while
  SoundSwitch concurrently reported `1 Link · 155.0`;
- Off, Arm, Start, Pause and Off were exercised through the visible controls;
  all transitions retained exactly one Carabiner helper and did not interrupt
  the independently enabled Link relay;
- Cmd-Q removed the owned Carabiner helper while the intended launchd-owned
  engine remained available.

Physical CDJ pitch movement and the combined one-hour Wi-Fi soak remain
explicit E5-04 release evidence. The direct Pro DJ Link simulator path and the
Local Playback path have both been UI-tested; neither substitutes for the
physical release soak.

## Why this story is reopened

The Link provider owns an independent worker and has no lighting API, but its
observations still arrive after the shared Java FIFO and the engine's shared
20 ms integration pump. That is logical API isolation, not complete scheduling
and backpressure isolation. Dev-52 physical evidence showed late/oscillating BPM
while the final provider remained healthy. Completion now requires the
source-side and task-level boundaries in ADR-0034.

## Dev-54 completion evidence

- callback traffic is classified before JSON serialization as critical, tempo,
  transport or display;
- critical events use a bounded ordered queue; continuous per-deck values use
  latest-value mailboxes and cannot accumulate history;
- the Rust ingress supervisor drains critical, tempo, transport and display in
  priority order and never coalesces critical facts;
- a dedicated `tempoStatus` event carries only the current CdjStatus-derived
  effective BPM and transport facts needed by the Link Relay;
- older Pro DJ Link observations are rejected and repeated unchanged Beats do
  not re-anchor or republish the Link clock;
- the CDJ-1500X network test converged exactly from `155.000` to `161.510` and
  `151.900` BPM with no old-value regression;
- the native dev-54 app and SoundSwitch UI showed those same `161.5` and `151.9`
  values while AutoLoop output remained independently active;
- all Java bridge, Rust engine, macOS package, technical and functional gates
  pass.

The one-hour combined physical pitch-ramp soak remains E5-04 evidence and does
not reopen this implementation story.
