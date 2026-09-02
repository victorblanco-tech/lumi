# Story E9-03: Native iPhone Live presentation

- Status: **Planned after design acceptance**
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

