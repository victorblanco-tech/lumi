# Story E9-03: Native iPhone Live presentation

- Status: **Implementation and headed Simulator visual acceptance complete; physical-device acceptance pending**
- Priority: **P1 product**
- Target: `0.6.0-dev`
- Components: Shared Live Presentation, iOS SwiftUI

## User outcome

The DJ sees the actual Master and prepared next Player in a booth-focused native
iPhone UI that matches Lumi's macOS visual language.

## Scope

- extract cross-platform Live mapping, phrase palette and beat-space rendering;
- build a dedicated iPhone SwiftUI composition rather than resizing the desktop
  workspace;
- portrait Master-first and next-Player layout;
- landscape fixed numbered Players side by side;
- render Player model, metadata, track color, RGB waveform, beatgrid, Hot Cues,
  fixed playhead, phrases and proportional Light Plan;
- add compact Pro DJ Link, Light Output and Ableton Link health;
- support pinch zoom, future inspection and `Follow Live` without seeking decks;
- implement connected, reconnecting, unpaired and unavailable states.

## Acceptance

- snapshot fixtures render deterministically at supported iPhone sizes,
  orientations, appearances and Dynamic Type sizes;
- all primary controls meet 44-point hit targets and VoiceOver exposes Player,
  role and action context;
- presentation interpolation never becomes input to the engine;
- no Local Playback, Library or developer surface is reachable in the product
  target.

## Implemented evidence

- independent native SwiftUI app target with Dev/RC/Production identities;
- portrait Master-first ordering without renaming physical Players;
- landscape side-by-side composition;
- detected Player model, track color, title/artist, BPM/key/remaining time;
- RGB waveform viewport with fixed Master playhead, beatgrid, Hot Cue letters,
  pinch zoom, horizontal inspection and `Follow Live`;
- aligned phrase and proportional Light Plan bands;
- compact integration/offset/operation controls and operation-state styling;
- explicit discovery, pairing, unavailable and version-mismatch empty states.
- native app lifecycle reconnect, no background command queue and foreground
  keep-awake behavior;
- Camera-deep-link pairing with matching confirmation code;
- one fixed command-feedback line and accepted/rejected haptics without moving
  either Player surface.
- headed landscape and portrait acceptance displayed the running `LUMI-SIM`
  Player, track metadata, 155 BPM, RGB waveform, phrases and proportional Light
  Plan from the real gateway projection; distinct captured frames proved
  transport movement.
- headed dev-5 acceptance verified the exact shared waveform color curve, a
  stable second-Player waiting surface, equal landscape Player sizing and the
  compact operation bar; a real `ARM` command round-trip and safe return to
  `OFF` completed without moving or hiding either Player surface.

## Remaining gate

Complete deterministic visual evidence across the remaining supported iPhone
sizes, Dynamic Type and VoiceOver; then perform actual-device gesture, rotation,
keep-awake and booth-legibility acceptance.
