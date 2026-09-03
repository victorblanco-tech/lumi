# Lumi Remote 0.1.0-dev-10 – stable fixed waveform

This iPhone-only development release replaces the waveform's per-frame SwiftUI
redraw with the same architectural pattern already proven by the macOS Live
surface: a stable high-resolution RGB track raster moves below a fixed playhead
through Core Animation. The remote still consumes the authoritative transport
anchor from Lumi; visual interpolation never feeds the show engine.

Pinch zoom now changes only the number of visible bars. On the Master Player the
playhead remains at 22% and the waveform continues following the live CDJ
position. Drag inspection remains available only for a prepared non-Master
Player, where it is clamped to the track bounds.

The change is scoped entirely to Lumi Remote. No macOS waveform, Pro DJ Link,
SoundSwitch MIDI or Ableton Link code is changed.

## Verification

- the actual iOS/UIKit path builds for the iPhone Simulator;
- all 13 Lumi Remote presentation tests pass with warnings treated as errors;
- fixed-playhead math is covered at 2, 10, 40 and full-track bar-equivalent
  zoom levels, including hostile inspection/drag input;
- prepared-Player inspection remains bounded;
- headed portrait and landscape playback against the LAN Pro DJ Link simulator
  showed the waveform moving below one unchanged playhead position;
- a 1,187-frame Simulator capture contained no dark waveform frames while the
  track advanced, providing a deterministic regression signal for the reported
  full-surface flicker;
- physical iPhone build and live gesture acceptance are the final delivery
  checks for this development release.
