# ADR-0018: BLT MIDI deck frames and separate CoreMIDI routes

- Status: accepted
- Date: 2026-08-06

## Context

Lumi needs live Deck A/Deck B state before a future native PRO DJ LINK adapter
exists. Beat Link Trigger (BLT) already normalizes player status and can execute
small custom expressions for tracked updates. The proven SoundSwitch integration
already uses Lumi as a virtual CoreMIDI source. Reusing that endpoint or its MIDI
channel would couple deck input to lighting output and could create feedback.

## Decision

Lumi publishes two independent CoreMIDI routes:

- `Lumi Deck Input` is a virtual destination used only for BLT deck state;
- `Lumi Virtual MIDI` remains a virtual source used only for SoundSwitch output.

BLT Player 1 uses MIDI channel 1 and Player 2 uses channel 2. SoundSwitch output
remains on channel 16. A BLT Tracked Update expression sends a versioned group of
Control Change values followed by controller 119 as an atomic commit marker.
Only a complete supported frame may mutate deck state.

The portable `lumi-blt-midi` adapter decodes frames and implements the existing
`DeckSourceProvider` port. CoreMIDI types stop at the transport boundary. The
domain receives only normalized load, position, playback and leader observations.

## Safety and lifecycle

- The CoreMIDI receive callback writes to a bounded 256-packet buffer.
- The single-writer engine drains input at authenticated command boundaries.
- Connected Decks polls snapshots every 250 ms. Other modes poll once per
  second for process health and raw integration diagnostics, but discard deck
  frames until Connected Decks is selected.
- Foreign channels, non-CC messages, unknown protocol versions, partial frames
  and duplicate sequence numbers are ignored and counted.
- Unknown external tracks are represented as transient, unanalyzed loads and
  remain `AUTO HELD`; Lumi sends no automatic lighting change for them.
- Switching away from Connected Decks unloads its transient deck state.
- A physical controller and SoundSwitch can remain active in parallel because
  input and output use different endpoints and channels.

## Consequences

This first adapter carries numeric facts that MIDI can represent reliably:
player identity, rekordbox/source identity, loaded, playing, master, on-air,
BPM, beat and duration. Title, artist, musical key, RGB waveform and phrase
analysis need either an exact Lumi Library match or a later richer metadata
transport. The adapter boundary allows that transport, BLT itself, or a native
PRO DJ LINK implementation to be replaced without changing Live or the domain.
