# SoundSwitch MIDI POC

## Goal

Prove that Lumi can coexist with SoundSwitch and a physical Control One while Lumi acts as an independent MIDI controller. SoundSwitch remains responsible for AutoLoops, Ableton Link timing, Control One input, and DMX output.

## Architecture boundary

`Test Controller → Lumi engine → MIDI output port → CoreMIDI adapter → Lumi Virtual MIDI → SoundSwitch`

The output port is provider-neutral. CoreMIDI is the first adapter; a later Windows adapter can implement the same port without changing planning or UI behavior. Lumi never targets Control One and does not own DMX.

## Safety rules

- Publishing the virtual source sends no MIDI.
- No source is published automatically at app start.
- The first POC signal is manual only: MIDI channel 16, note 60.
- Every learn signal contains Note On followed by Note Off in one CoreMIDI event list.
- Stop and process exit dispose the virtual source.
- Automatic phrase output, strobes, and lasers are outside this slice.

## Physical checkpoints

1. Publish `Lumi Virtual MIDI` and confirm SoundSwitch discovers it.
2. Put one harmless SoundSwitch control into MIDI Learn and send one learn pulse.
3. Learn four bank actions and 32 AutoLoop actions, then validate bank/wait/slot sequences.
4. Connect Control One and DMX fixtures; prove Lumi and Control One work in parallel.
5. Disconnect and reconnect each component and verify fail-silent recovery.

The POC branch must not merge into `dev` until the relevant physical checkpoint has passed.
