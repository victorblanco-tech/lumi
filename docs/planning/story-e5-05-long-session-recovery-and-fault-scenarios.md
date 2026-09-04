# Story E5-05: Long-session recovery and deterministic fault scenarios

- Status: **Implemented; two-host duration evidence pending**
- Priority: **P0 Critical**
- Lumi target: `0.6.2`
- Simulator target: `0.4.1`

## Outcome

A long-running Live session recovers cleanly after a Player joins, leaves or
temporarily loses timing traffic. A recovered Master immediately restores the
global Live state to green when its exact Light Plan is valid, without hiding
an `AUTO HELD` state on another Player.

## Product rules

- only the active Master plan is show-critical for the global Phrase warning;
- an idle or next Player remains visibly `AUTO HELD` on its own Player card;
- a Master without a trusted plan remains fail-safe and globally visible;
- recovery presentation cannot restart or couple the AutoLoop, Ableton Link or
  Lumi Remote realtime lanes;
- the simulator applies faults at the packet boundary while its authoritative
  Player clock continues uninterrupted;
- every fault is opt-in, bounded, visible and recoverable without restarting
  either product.

## Acceptance

- Player join/remove and a five-second full traffic interruption recover;
- an exact-position gap and deterministic timing-packet loss recover;
- Master handover, Hot Cue and positive/negative beat jump remain explicit
  discontinuities;
- the repeatable Recovery Soak cycles position gap, packet loss, disconnect,
  beat jump and Master handover;
- Auto Mix can run simultaneously to rotate playlist tracks and Master Player;
- status/API telemetry identifies every active fault, remaining duration and
  suppressed-packet count;
- global Phrase status becomes green as soon as the current Master has a valid
  non-fallback plan, irrespective of an idle held Player;
- unit, package, headless API and headed browser/native UI checks pass.

## Evidence

- `LiveWorkspacePresenterTests` covers recovered-Master, idle-held and
  held-Master presentation separately;
- simulator transport tests prove deterministic lane loss, timed expiry,
  continuous authoritative playback and the five-step recovery sequence;
- simulator controls cover beat jump, fault clearing and exclusive Master
  handover;
- two-host duration and SoundSwitch observation remain release evidence rather
  than a unit-test assertion.
