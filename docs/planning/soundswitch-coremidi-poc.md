# SoundSwitch / CoreMIDI POC

Status: **Prepared – execute after the Library/Settings demo slice**

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
- production binding of all 128 AutoLoop positions;
- ShowNET/laser output;
- automatic live execution;
- iPhone controls;
- Windows MIDI transport.
