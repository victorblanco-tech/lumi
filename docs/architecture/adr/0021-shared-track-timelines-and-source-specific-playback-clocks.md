# ADR-0021: Shared track timelines and source-specific playback clocks

- Status: **Accepted**
- Date: **2026-08-07**

## Context

The Track Editor, Local Playback and connected Live Decks all present the same
track analysis, but serve different workflows. The editor needs a freely
movable playhead and detailed editing. Local Playback owns native audio and must
remain visually smooth. A connected deck remains the external transport master.

Early Live iterations coupled large authoritative snapshots, UI animation and
multiple independently derived timelines too closely. Under real imported
tracks this caused slow deck loads, delayed leader changes and visible drift
between waveform, phrase and AutoLoop-plan playheads.

## Decision

Lumi separates immutable track-time data from source-specific playback clocks.

Shared track-time data contains:

- stable track-load identity and duration;
- the exact imported beat grid, including valid trailing markers;
- the Lumi-owned phrase timeline and materialized lighting plan;
- cached RGB waveform detail and bounded preview data;
- explicit analysis provenance and revision.

Each playback source publishes provider-neutral position anchors containing at
least track-load identity, position or beat, effective BPM, playing state,
observation time and discontinuity/revision information. The source remains
authoritative:

- Track Editor uses its editor audio clock and a freely movable playhead;
- Local Playback uses the native audio clock and the accepted fixed-live-
  playhead policy while playing;
- connected Live Decks use observations from the configured deck adapter and
  never let Lumi become transport master.

The engine owns plan state, phrase-boundary decisions and output scheduling.
Clients may interpolate a visual position between authoritative anchors for
smooth rendering, but that interpolation cannot emit MIDI, advance engine state
or become an alternative lighting clock. Waveform, beatgrid, phrase band and
AutoLoop Plan on one deck use the same visual clock and beat conversion.

Static analysis is sent when a track loads or reconciliation is required.
High-frequency transport changes use bounded incremental messages or lightweight
acknowledgements. Full snapshots remain the recovery mechanism, not the display
frame rate. Renderer primitives may be shared across Editor and Live, while
their interaction policies remain separate.

## Consequences

- Local Playback can be smooth without weakening engine-authoritative output.
- A future Pro DJ Link adapter can reuse the timeline and rendering foundation
  while retaining external-master and stale-observation semantics.
- Editor improvements can reuse waveform/beatgrid primitives without inheriting
  the Live fixed-playhead or phrase-locking policy.
- Plan and light output stay synchronized to one engine timeline even when the
  UI renders at a higher frame rate.
- Provider adapters must expose discontinuities explicitly after seek, load,
  reconnect or master change.
- Connected transport snapshots carry a monotone discontinuity revision. Live
  renderers use a revision change to discard stale Core Animation motion and
  resume Master follow at the new authoritative position; the revision never
  participates in lighting-output timing.

## Rejected alternatives

### Drive lighting from the SwiftUI animation clock

Rejected because rendering cadence is neither realtime-safe nor authoritative
and may pause under window or system load.

### Poll full dual-deck snapshots at animation rate

Rejected because immutable waveform and plan payloads dominate transport
traffic and caused visible interaction latency.

### Force Editor, Local Playback and connected decks into one monolithic view

Rejected because their transport ownership, editing rules and failure modes are
different even though their analysis and rendering primitives are reusable.
