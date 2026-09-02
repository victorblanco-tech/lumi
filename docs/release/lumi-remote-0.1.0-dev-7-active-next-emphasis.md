# Lumi Remote 0.1.0-dev-7 – active and next phrase emphasis

Date: 2026-09-02  
Status: verified development build

## Outcome

Lumi Remote now exposes the same live-planning hierarchy as the Mac Live view.
The running Phrase and proportional Light Plan block receive a red live glow;
the first future Phrase and block receive Lumi blue with a `NEXT` label. Later
planning remains visually quiet, so the booth view prioritizes what is happening
and what the DJ can still change.

The configured Phrase color remains the base layer. Status is a separate overlay
and does not mutate Phrase data. The upcoming plan block remains a full touch
target for the existing Theme/Bank, AutoLoop and lock editor.

## Verification

- deterministic Remote feature tests cover completed, active, next and planned
  states plus an exact phrase-boundary handoff;
- the signed iPhone Simulator build was connected to the packaged dev-8 Remote
  Gateway and LAN Pro DJ Link simulator;
- headed portrait and landscape playback showed the red `ACTIVE` and blue
  `NEXT` state moving with the same interpolated beat as the waveform;
- the blue next block still opened the existing revision-safe adjustment sheet.

This presentation-only work does not add commands, queues or dependencies to
the Mac engine's Pro DJ Link, SoundSwitch MIDI or Ableton Link lanes.
