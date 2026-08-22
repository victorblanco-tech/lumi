# E6-06 – Automatic Static Look execution

Status: **Implemented in `0.5.0-dev-6`** | Priority: **P0** | Effort: **5**

## User value

As a DJ I can let a compiled Light Plan temporarily apply a verified
SoundSwitch Static Look, while the normal AutoLoop keeps running for fixtures
that the Static Look does not override.

## Acceptance criteria

- Only enabled Static Looks with separately verified activation and release are
  eligible for automatic compilation.
- The complete choice is deterministic and made before playback from Application
  Rate, Selection Weight, exact Phrase Role, cooldown, scope and track color.
- Every phrase visibly shows either the selected Static Look or `No Override` in
  Plan Preview; Live shows the selected look inside the proportional plan item.
- Runtime receives only immutable MIDI addresses; it never evaluates policy,
  reads SQLite or waits for SwiftUI.
- Entering a selected look emits one pulse, changing look emits one replacement
  pulse, leaving it emits one release pulse, and an unchanged desired look emits
  nothing.
- Pause/resume and equivalent deck observations cannot replay a Static Look.
- A seek/hotcue only emits when its destination requires a different state.
- `Off` and an explicit MIDI-source stop attempt one release of a Lumi-managed
  active look.
- Static Look execution does not change AutoLoop generations, deadlines or
  Ableton Link state.
- Failed or unavailable output leaves Lumi's assumed state unchanged and is
  counted in diagnostics.

## Physical evidence

The 2026-08-22 POC proved that one learned note pulse selects a Static Look, the
same pulse releases it and selecting another look replaces it. SoundSwitch does
not return selected-look state. Lumi therefore tracks only its own successful
commands and never continuously reasserts a Static Look.

## Verification

- deterministic compiler tests for verified, unverified, phrase and whole-track
  selection;
- engine regression tests for the existing exactly-once AutoLoop path;
- strict macOS decoding and visible Plan Preview/Live projection;
- headed Local Playback test followed by a physical SoundSwitch/DMX test.
