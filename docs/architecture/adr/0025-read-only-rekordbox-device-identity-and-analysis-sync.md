# ADR-0025: Read-only Rekordbox Device identity and analysis sync

- Status: **Accepted**
- Date: **2026-08-09**

## Context

Connected Decks receives timely transport state through Beat Link Trigger, but
the MIDI frame cannot carry a complete waveform, beatgrid, cue set and Lumi
phrase timeline. More importantly, a track loaded from a CDJ must resolve to
the same canonical Lumi track that was prepared in Library. Matching only by
title at performance time is ambiguous and unsafe.

The BLT Shallow Playback Simulator has a separate limitation: it publishes the
fixed Rekordbox ID `42` for every simulated track. BLT itself remains an
independent, unmodified application.

Users also improve beatgrids and cue points after the initial import. A device
sync must therefore refresh analysis provenance every time without replacing
Lumi-owned phrase and AutoLoop edits.

## Decision

Lumi adds a provider adapter for a mounted current Rekordbox OneLibrary device:

- it opens `PIONEER/rekordbox/exportLibrary.db` and referenced
  DAT/EXT/2EX analysis files
  only for reading;
- it validates that every declared path remains below the selected device root;
- it fingerprints the DeviceSQL database plus the complete DAT/EXT/2EX analysis
  companion set on every sync;
- it stores a durable alias from `(device source, device track ID)` to one
  canonical Lumi track;
- it refreshes matched analysis projections atomically with that alias
  snapshot, while beatgrid/waveform promotion and hot-cue promotion retain
  independent provenance;
- it normalizes hot-cue letter/index, source time, optional loop end, comment
  and RGB color into Lumi's provider-neutral analysis model;
- it never overwrites Lumi phrase timelines, phrase-role choices, Themes or
  track-specific AutoLoop choices.

Canonical matching during sync uses normalized title and artist, BPM, duration
and exact audio-file size. A match is accepted only when it is one-to-one in
both directions. Unmatched or ambiguous rows remain stored for diagnostics but
cannot hydrate a live deck.

Real BLT frames carry the actual Rekordbox Device Library ID. Protocol v3 also
carries a deterministic metadata signature for shallow simulation only, based
on normalized title, artist, original BPM and duration. This bypasses the
simulator's fixed ID without changing or patching BLT. A zero signature selects
real-device ID resolution.

All aliases and refreshed analyses are published in one SQLite transaction. An
interrupted or invalid sync leaves the previous known-good snapshot available.
Hot cues deliberately form a separate replaceable projection. This allows an
existing canonical track whose beatgrid is held by the monotone promotion rule
to receive current cue facts without replacing its analysis revision, waveform
or Lumi-owned timeline. Current OneLibrary point encoding `1`, legacy point
encoding `0` and loop encoding `2` are normalized into the same provider-neutral
model.

## Consequences

- The same prepared Lumi timeline can drive Local Playback and Connected Decks.
- A later USB sync picks up beatgrid or cue edits through changed analysis
  revisions; no manual Lumi reconfiguration is required.
- The lighting engine never depends on UI timing or fuzzy live matching.
- Unknown tracks continue safely as external metadata with `AUTO HELD`.
- Older classic DeviceSQL media is not silently accepted as OneLibrary and
  must be upgraded/exported by a current rekordbox version first.
- Track Editor, Local Playback and Live Decks render the same persisted hot-cue
  facts subtly over their shared waveform and in a compact letter/name strip.
- Upgrading an existing library invalidates only read-only analysis promotion
  evidence, so the next explicit USB sync fills cue data without rebuilding
  the Library or touching authored phrases and lighting configuration.

## Rejected alternatives

### Change or fork Beat Link Trigger

Rejected. Lumi must remain robust against the independently installed BLT
release and should only require its documented user-configurable expression.

### Match a live deck by title only

Rejected because duplicate titles, edits and incomplete metadata could trigger
the wrong lighting plan.

### Replace Lumi timelines during every device sync

Rejected because phrases and per-track lighting decisions are Lumi-owned and
must survive external beatgrid and cue maintenance.
