# Lumi Remote 0.1.0-dev-5 – compact Live layout

This iPhone development build makes the Remote Live view useful as a two-Player
booth surface rather than a reduced desktop card.

## Changed

- Waveforms use the exact shared Lumi RGB mapping and amplitude curve.
- Two numbered Player positions remain visible when only one track is loaded.
- Landscape keeps Player numbers left-to-right and combines Player, track,
  transport and role metadata into one compact row.
- The operation controls occupy one compact landscape toolbar row.
- Phrase Type labels and the full proportional Light Plan remain visible below
  each waveform.
- Portrait remains scrollable and moves the actual Master Player first.

## Automated evidence

- one loaded Player produces one real surface and one stable empty Player slot;
- landscape preserves physical Player-number order while portrait prioritizes
  the Master without renaming either Player;
- a known red PWV5 sample maps through the same `LumiRGBWaveformSample` contract
  used by the Mac Live and Track Editor surfaces.

## Headed evidence

- the signed dev-5 build reconnected through its stored Keychain identity;
- portrait showed the loaded Master plus the stable second-Player waiting slot;
- landscape showed two equal numbered Player surfaces, a compact one-line
  toolbar and an expanded RGB waveform while keeping phrases and the complete
  proportional Light Plan on screen;
- the running gateway projection displayed the shared cyan, blue, red and pink
  waveform palette rather than the previous flat white/green rendering;
- `ARM` reached the authoritative Mac reducer, returned the live 155.0 BPM Link
  status and operation styling, and `OFF` restored the safe resting state.

Physical-iPhone visual acceptance remains separate; the Remote remains outside
every realtime Pro DJ Link, Ableton Link and lighting output lane.
