# E3-01 – Generalize SoundSwitch bank and AutoLoop MIDI execution

## Outcome

Turn the physically proven Bank 1 → 50 ms → AutoLoop 1 path into the generic,
diagnosable MIDI execution primitive used by Lumi for every configured
SoundSwitch target.

## Current baseline

- `Lumi Virtual MIDI` is discovered by SoundSwitch.
- Channel 16 notes 60–63 address Bank 1–4.
- Channel 16 notes 64–95 address AutoLoop 1–32.
- Bank 1 → 50 ms → AutoLoop 1 works physically.
- Control One remains usable in parallel and DMX output visibly works.
- Test Controller and MIDI Status are permanent product diagnostics.

## Acceptance criteria

- Test Controller can explicitly trigger any selected Bank 1–4 and AutoLoop
  1–32 combination.
- Every trigger sends Bank Note On/Off, waits the validated bank-settle delay,
  then sends AutoLoop Note On/Off.
- The 128 logical `Bank + AutoLoop Name + Phrase Type` mappings resolve to the
  canonical four bank and 32 AutoLoop MIDI addresses without duplicating MIDI
  state in the library domain.
- Publishing the virtual source sends no MIDI; stopped or missing endpoints fail
  silent.
- MIDI Status shows current source state, last command, pulse count and errors.
- Manual SoundSwitch/Control One input may override Lumi; the next valid Lumi
  trigger may take control again.
- At least 100 repeated triggers preserve exact command order without hangs,
  duplicate pulses or unexpected bank/AutoLoop selection.
- Disconnect/reconnect of Lumi and Control One produces no unsolicited MIDI or
  light change and recovers through an explicit user action.
- Rust unit tests, real engine/process integration, Swift package tests and one
  native macOS build pass locally without GitHub Actions.

## Boundaries

- No automatic LIVE start.
- Phrase-boundary scheduling and OFF/ARMED/LIVE/PAUSED ownership remain in the
  wider SoundSwitch Live MVP epic.
- SoundSwitch remains responsible for AutoLoops, Ableton Link timing and DMX.
- Control One remains outside the Lumi domain.
- Output Profile Builder, ShowNET/laser and Windows MIDI remain deferred.
