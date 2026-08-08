# ADR-0022: Separate CoreMIDI sources for lighting commands and playback timing

- Status: **Accepted**
- Date: **2026-08-08**

## Context

Local Playback must supply SoundSwitch with two semantically different MIDI
flows:

- sparse, learned bank and AutoLoop commands at phrase boundaries;
- continuous transport timing at 24 pulses per quarter note, including Start,
  Continue, Stop and Song Position messages.

The existing `Lumi Virtual MIDI` endpoint had already been physically proven
for SoundSwitch command mapping alongside Control One. Mixing continuous system
realtime traffic into that learned command route would couple two failure
domains, make diagnostics ambiguous and risk changing a known-good integration.
SoundSwitch can select a local MIDI port for sync independently from learned
controller commands.

## Decision

Lumi publishes two provider-owned CoreMIDI sources:

- `Lumi Virtual MIDI` carries only bank selection, AutoLoop triggers and
  explicit learn/test pulses;
- `Lumi Clock` carries only MIDI Clock and transport timing for Local Playback.

Both sources are lifecycle-managed by the lighting-output integration. Publish
must establish both endpoints; a partial publish is rolled back. Stop removes
both. Each source has independent status and error diagnostics, while the user
performs one coherent publish/stop operation.

The timing scheduler runs in the Rust output layer on a dedicated thread. It
derives tempo, Song Position and phase from the authoritative Local Playback
transport plus exact imported beatgrid. It is active only when Lumi is Live and
the leader deck is actually playing. SwiftUI may display status and interpolate
visual motion, but never generates clock ticks or lighting commands.

Connected Live Deck timing remains external: Beat Link Trigger/Ableton Link or
a future Pro DJ Link adapter owns that source-specific clock. `Lumi Clock` is
therefore a Local Playback rehearsal clock, not a competing live-deck master.

## Consequences

- Existing SoundSwitch AutoLoop MIDI mappings and Control One coexistence remain
  isolated from continuous clock traffic.
- SoundSwitch configuration is explicit: command learning uses
  `Lumi Virtual MIDI`; MIDI Sync In uses `Lumi Clock`.
- Clock loss cannot be mistaken for a failed AutoLoop note, and the two routes
  can expose separate diagnostics and counters.
- Publishing is stricter because both local endpoints must succeed atomically.
- A future generic output profile may choose different adapters, but it must
  preserve the semantic separation between discrete commands and transport
  timing.

## Rejected alternatives

### Send clock and command notes through one virtual source

Rejected because it changes a physically proven command route, combines a
continuous realtime stream with sparse learned controls and weakens failure
isolation.

### Generate MIDI Clock from the Swift audio/UI layer

Rejected because UI scheduling is not authoritative or realtime-safe and can
stall independently from the engine output path.

### Make Lumi Clock the timing master for connected DJ decks

Rejected because connected decks remain the external transport authority. The
dedicated clock exists only to complete the offline Local Playback path.
