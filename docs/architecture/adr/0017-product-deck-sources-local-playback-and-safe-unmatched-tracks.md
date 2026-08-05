# ADR-0017: Product deck sources, Local Playback and safe unmatched tracks

- Status: **Accepted**
- Date: **2026-08-05**

## Context

The first Live slices used the deterministic simulator as both a test input and
a visible product mode. That made the UI testable, but it also leaked legacy
phrase names such as `Verse` and `Build` into Live and made a test clock look
like a real deck source. Lumi now needs a production Live experience before the
Beat Link Trigger adapter is connected.

The DJ must be able to prepare and rehearse sets without physical decks, using
the audio, beatgrid, RGB waveform and Lumi-owned phrase timeline already stored
in the local Library. A physical or simulated Beat Link deck can also load a
track that is not present locally or cannot be matched safely. Missing analysis
must never crash Lumi or create confident but fabricated light changes.

## Decision

### Product source modes

Exactly one authoritative deck source remains active per session:

1. **Connected Decks** receives normalized transport and track observations
   from the selected live adapter, initially Beat Link Trigger.
2. **Local Playback** exposes two application-owned decks. Tracks are loaded
   from the Lumi Library and played through local native audio. Play, pause,
   seek, master selection and track end publish the same normalized observations
   as a connected provider.

The selected deck source is independent from the output operation state. Local
Playback can therefore run with output `Off`, `Armed`, `Live`, or `Paused` and
supports both dry rehearsal and real SoundSwitch output.

The deterministic simulator and replay providers remain internal test adapters.
They are not selectable, named, badged or controlled from production Live UI.
Demo Library tracks remain valid product-visible local content.

### Canonical phrase identity

Live planning and presentation use the stable, configurable Lumi
`PhraseRoleId` and its current display name. Provider labels are mapped once at
an adapter or library boundary and are never presented as the Live role.

The legacy fixed `PhraseKind` taxonomy is not a valid Live presentation model.
For example, source `Verse` may map to Lumi `Bridge`, and source `Build` may map
to `Buildup 1`; Live shows only the latter names. Autoloop resolution keeps the
exact role identity, including `Breakdown 1–3`, `Buildup 1–3`, `Synth` and
`Pre-drop`.

### Phrase execution states

Execution immutability and a user-pinned cue are different concepts:

- `Completed`: the phrase ended and is read-only;
- `Live`: the phrase started and is read-only;
- `Planned`: the phrase has not started and remains editable;
- `Pinned`: an optional user choice retained during regeneration.

Reloading a track creates a new `trackLoadId`. Local rehearsal may explicitly
restart a load from the beginning; it never silently rewrites the execution
history of an existing load.

### Library matching and safe fallback

A deck load has one plan-eligibility state:

- `READY_EXACT`: an exact Lumi Library identity and timeline were resolved;
- `READY_TRANSIENT`: no Library match exists, but the provider supplied a
  complete, beat-aligned phrase timeline whose labels all map to active Lumi
  roles;
- `AUTO_HELD`: analysis is absent, incomplete, stale or unmapped.

Transient provider analysis is session-owned and never mutates the Library
without an explicit import action. Unknown labels never use a wildcard role or
silently become `Bridge`.

`AUTO_HELD` is scoped to the affected track load. Lumi keeps the current safe
look and sends no new automatic cue for that plan. Manual MIDI controls and a
valid plan on the other deck remain available. A late complete analysis can
prepare future phrases, but an already Live deck requires explicit resume before
automatic output continues.

## Consequences

- Live opens without fabricated loaded tracks.
- The same Live UI and engine path serve local rehearsal and connected decks.
- Local playback position is derived from native audio and then normalized into
  deck events; SwiftUI never runs an independent playhead timer.
- Beat Link Trigger can be added behind the existing deck-source boundary after
  the Live product states are complete.
- Unknown tracks degrade visibly and safely instead of crashing or continuing a
  guessed plan.
- Test fixtures can keep deterministic simulation without exposing it as a
  product feature.

## Rejected alternatives

### Keep Demo/Simulator as a Live source mode

Rejected because it exposes engineering scaffolding to the DJ and does not
prove local audio or real adapter transport.

### Treat Local Playback as a special SwiftUI preview

Rejected because it would bypass planning, execution safety and MIDI paths and
would provide false rehearsal confidence.

### Map every unknown phrase label to Bridge

Rejected because it can execute an unrelated Autoloop with high confidence and
hides missing configuration.
