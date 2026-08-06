# ADR-0016: Stable deck identity, rolling Live plans and waveform sources

- Status: **Accepted**
- Date: **2026-08-05**

## Context

The Live workspace must remain recognizable to a DJ. Physical Deck A and Deck B
must not swap places when the mixer master changes. At the same time, Lumi must
make the currently authoritative lighting deck unmistakably Live and keep the
loaded other deck ready for advance planning.

The DJ must also be able to change a future phrase of the current track after a
transition. For example, while a Breakdown is active, an Outro can already be
scheduled with another Theme and AutoLoop. The active phrase may not be
retroactively mutated.

Waveforms can be available from a local Lumi library, a live deck provider such
as Beat Link, or deterministic simulator data. Their availability and detail
must not couple SwiftUI to any one provider.

## Decision

### Stable deck positions and moving master role

- The desktop Live workspace always presents Deck A left and Deck B right.
- Deck identity is stable and ordered by the normalized deck identifier.
- `leaderDeckId` moves the `MASTER · LIVE NOW` role between those fixed deck
  surfaces; the cards themselves never reorder.
- The master surface uses a strong Live treatment. A loaded non-master surface
  uses a quieter `PLAN READY` treatment.
- A master change promotes the already prepared plan for the new master to the
  rolling Live plan. The old master becomes outgoing/non-master and can later
  become the next planning surface after a new track load.

### Rolling Live plan

- The active phrase and every earlier phrase are immutable in the plan editor.
- Future phrases in the current Live track remain editable until their phrase
  boundary starts.
- AutoLoop selection applies to the selected future phrase.
- A Theme change can be scheduled from a selected future phrase onward.
- At the phrase boundary the scheduled change is executed and becomes locked.
- Revision and track-load-instance checks remain mandatory for every mutation.

### Provider-neutral waveform presentation

- The normalized deck snapshot may expose a whole-track waveform preview,
  waveform style, provenance and availability.
- Playback position, beatgrid and master state remain independent normalized
  observations and are overlaid by the client.
- Preferred source order is an exact local Lumi-library match, provider waveform
  data, then an explicit unavailable state. Simulator waveforms are marked as
  simulator data and are never presented as extracted audio analysis.
- Beat Link/JVM, Rekordbox and native PRO DJ LINK types never enter the domain or
  SwiftUI presentation model.

### iPhone hierarchy

- Portrait presents the Live deck followed by the planned deck.
- Landscape may present the two stable deck surfaces side by side.
- Both iPhone surfaces show the phrase band directly below the waveform.
- The iPhone client consumes the same deck, plan and waveform presentation
  contract; it does not independently infer the master.

## Consequences

- A user can always associate the left/right surface with physical Deck A/B.
- Master changes are a state transition, not a layout transition.
- Lumi needs full plan details for both the current and next track, not only a
  summary of the active plan.
- The engine must retain the promoted plan after a master switch so future Live
  phrases remain revision-safe and editable.
- Waveform rendering can be built and tested with the simulator before Beat Link
  is available while preserving explicit provenance.
- The later native iPhone app can reuse the same presentation decisions without
  copying planning logic.

## Rejected alternatives

### Sort cards as Live and Next

Rejected because the same physical deck moves between screen positions whenever
master changes, which is hard to follow during a transition.

### Make the complete Live plan read-only

Rejected because it prevents calm, advance corrections to later phrases in the
currently playing track.

### Render Beat Link UI output directly

Rejected because it couples Lumi presentation to a JVM/provider-specific view
and prevents local-library and future native-provider reuse.
