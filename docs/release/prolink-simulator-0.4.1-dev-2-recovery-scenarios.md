# Pro DJ Link Simulator `0.4.1-dev-2`: Recovery scenarios

## Outcome

The simulator can now reproduce Player and timing failures without changing
its authoritative playback clock. This makes long-running recovery behavior
repeatable instead of dependent on unplugging hardware or Wi-Fi at an exact
moment.

## New controls

- take either Player offline and bring it back as the same identity;
- apply a bounded exact-position gap, deterministic timing-packet loss or full
  temporary disconnect to the current Master;
- trigger an exact USB-beat-grid jump, Hot Cue seek or Master handover;
- clear all faults immediately;
- run Recovery Soak, a deterministic five-step cycle of position gap, packet
  loss, disconnect, beat jump and Master handover;
- combine Recovery Soak with playlist Auto Mix for changing tracks and Master
  Players over a long unattended run.

Every active fault, remaining duration and suppressed-packet count is exposed
through the authenticated status API and browser UI.

## Evidence

- 28 Java tests pass across packet parsing, player clocks, loops, Auto Mix,
  deterministic faults and authenticated HTTP controls;
- browser JavaScript parses successfully;
- the shaded simulator package builds successfully;
- version, structure, architecture and documentation gates pass.
