# SoundSwitch / CoreMIDI POC

Status: **Core path passed physically on 2026-08-05; repetition and reconnect remain**

## Question

Can Lumi act as its own virtual MIDI controller for SoundSwitch while a physical
Control One remains usable and continues to provide working DMX output?

## Timeboxed scope

1. enumerate local CoreMIDI endpoints;
2. publish `Lumi Virtual MIDI` as a virtual source;
3. make SoundSwitch discover and select that source for MIDI mapping;
4. map one bank selector and multiple Autoloop buttons;
5. send explicit Note On/Off sequences from a manual test harness;
6. measure bank-switch delay and repeat behavior;
7. operate Control One between Lumi triggers and verify coexistence;
8. connect at least one fixture and visibly prove DMX through Control One;
9. disconnect/reconnect Lumi MIDI and Control One independently;
10. capture MIDI transcript, screenshots, DMX evidence, versions, limitations,
    and a go/no-go decision.

## Evidence status

- **Passed:** SoundSwitch discovers `Lumi Virtual MIDI`.
- **Passed:** Bank 1 learns channel 16 / note 60 and AutoLoop 1 learns channel
  16 / note 64.
- **Passed:** the runtime sequence selects Bank 1, waits 50 ms, then activates
  AutoLoop 1.
- **Passed:** Control One can override Lumi and Lumi can take control again.
- **Passed:** fixtures visibly respond through Control One DMX while Lumi drives
  SoundSwitch.
- **Passed:** Virtual Controller and MIDI Status remain permanent diagnostics.
- **Pending:** deterministic repetition over at least 100 triggers.
- **Pending:** independent disconnect/reconnect of Lumi and Control One without
  unsolicited MIDI or light changes.

## Safety

- no automatic phrase-boundary output;
- no startup in LIVE;
- one explicit test action at a time;
- no strobe or hazardous laser output;
- fail-silent when the endpoint disappears;
- targetstate remains `ASSUMED` without acknowledgement;
- an emergency stop action is always available.

## Acceptance

- SoundSwitch sees the Lumi virtual source after documented setup;
- bank plus Autoloop selection is deterministic over at least 100 repetitions;
- Control One remains responsive before, between, and after Lumi commands;
- a fixture responds through Control One DMX during the coexistence test;
- reconnect produces no unsolicited MIDI message or light change;
- the measured sequencing contract fits `SoundSwitchMidiOutputProvider` and
  `MidiTransportProvider`.

## Explicitly deferred

- generic Output Profile Builder;
- automatic live binding of all 128 logical AutoLoop mappings;
- ShowNET/laser output;
- automatic live execution;
- iPhone controls;
- Windows MIDI transport.
