# SoundSwitch MIDI POC

## Goal

Prove that Lumi can coexist with SoundSwitch and a physical Control One while Lumi acts as an independent MIDI controller. SoundSwitch remains responsible for AutoLoops, Ableton Link timing, Control One input, and DMX output.

## Architecture boundary

`Virtual Controller → Lumi engine → MIDI output port → CoreMIDI adapter → Lumi Virtual MIDI → SoundSwitch`

The output port is provider-neutral. CoreMIDI is the first adapter; a later Windows adapter can implement the same port without changing planning or UI behavior. Lumi never targets Control One and does not own DMX.

## Safety rules

- Publishing the virtual source sends no MIDI.
- Both virtual sources publish automatically at app start without sending MIDI;
  explicit Publish/Stop remains available as a diagnostic control.
- The first POC signal is manual only: MIDI channel 16, note 60.
- Every learn signal contains Note On followed by Note Off in one CoreMIDI event list.
- Stop and process exit dispose the virtual source.
- Automatic phrase output, strobes, and lasers are outside this slice.

## Physical checkpoints

1. Publish `Lumi Virtual MIDI` and confirm SoundSwitch discovers it. **Passed 2026-08-05:** SoundSwitch learned channel 16, note 60 from Lumi.
2. Put one harmless SoundSwitch control into MIDI Learn and send one learn pulse. **Passed 2026-08-05:** the original single-bank POC used channel 16 / note 64 for AutoLoop 1. The production 4×32 contract below supersedes that reused address.
3. Validate the Bank 1 / 50 ms wait / AutoLoop 1 runtime sequence. **Passed 2026-08-05:** SoundSwitch activated the expected AutoLoop and fixtures responded through DMX.
4. Connect Control One and DMX fixtures; prove Lumi and Control One work in parallel. **Passed 2026-08-05:** both controllers could override the current AutoLoop and Lumi could take control again.
5. Disconnect and reconnect each component and verify fail-silent recovery.

The POC branch must not merge into `dev` until the relevant physical checkpoint has passed.

## Canonical POC learn addresses

These addresses describe the logical SoundSwitch surface and do not depend on Control One.

| Target | MIDI notes |
| --- | --- |
| Bank 1–4 | Channel 16, Notes 60–63 |
| Bank 1 AutoLoop 1–32 | Channel 13, Notes 64–95 |
| Bank 2 AutoLoop 1–32 | Channel 14, Notes 64–95 |
| Bank 3 AutoLoop 1–32 | Channel 15, Notes 64–95 |
| Bank 4 AutoLoop 1–32 | Channel 16, Notes 64–95 |

The Virtual Controller emits one learn pulse at a time for mapping. `Send Learn
& Next` advances Lumi's selection, but SoundSwitch still requires the user to
arm `Map` for each button and provides no mapping confirmation back to Lumi.
Every Test action sends the selected Bank first, waits 50 ms for SoundSwitch to
apply it, and then sends that Bank's unique AutoLoop address. Each address
includes its Note On and Note Off pair.

The Virtual Controller mirrors SoundSwitch's visual order: AutoLoops 1–8 run from top to bottom in the first column, 9–16 in the second, 17–24 in the third, and 25–32 in the fourth. This is presentation order only; canonical MIDI notes and persisted button numbers remain unchanged.
