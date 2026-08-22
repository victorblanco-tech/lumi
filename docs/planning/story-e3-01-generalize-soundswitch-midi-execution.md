# E3-01 – Generalize SoundSwitch bank and AutoLoop MIDI execution

Status: **Functionally and physically proven; repetition and reconnect evidence remain**

## Outcome

Turn the physically proven Bank 1 → 50 ms → AutoLoop 1 path into the generic,
diagnosable MIDI execution primitive used by Lumi for every configured
SoundSwitch target.

## Current baseline

- `Lumi Virtual MIDI` is discovered by SoundSwitch.
- Channel 16 notes 60–63 address Bank 1–4.
- Bank-specific Channels 13–16 and notes 64–95 address all 128 AutoLoops
  uniquely; a later Bank learn cannot overwrite an earlier binding.
- Bank 1 → 50 ms → AutoLoop 1 works physically.
- Control One remains usable in parallel and DMX output visibly works.
- Virtual Controller and MIDI Status are permanent product diagnostics.
- `Lumi Virtual MIDI` now publishes automatically, self-recovers independently
  from the clock endpoint and participates in the compact Tech Ready status.
- A first physical run confirmed that Lumi and Control One can operate in
  parallel while SoundSwitch continues to own DMX output.

## Acceptance criteria

- Virtual Controller can explicitly trigger any selected Bank 1–4 and AutoLoop
  1–32 combination.
- Banks & AutoLoops exposes a Test action on every mapped or empty slot.
- Guided MIDI Learn sends the selected unique address and advances to the next
  slot, while clearly leaving the required SoundSwitch `Map` action and mapping
  confirmation with the user.
- Every trigger sends Bank Note On/Off, waits the validated bank-settle delay,
  then sends AutoLoop Note On/Off.
- The 128 logical `Bank + AutoLoop Name + Phrase Type` mappings resolve to 128
  unique AutoLoop MIDI addresses plus four bank selectors without duplicating MIDI
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

## Handoff to full-song acceptance

The generic bank/AutoLoop primitive and the automatic materialized-cue path are
available in the engine. [E3-02](story-e3-02-full-song-local-playback-to-lights.md)
now proves that path across one complete real Local Playback track, adds the
missing local tempo/beat-clock bridge and captures visible SoundSwitch/DMX
evidence. The repetition and disconnect/reconnect criteria above remain open
until that physical test block is executed. The first automatic full-song light
run has already proven real phrase-boundary execution; the open evidence is a
repeatable 100-trigger/reconnect stability run rather than basic feasibility.
