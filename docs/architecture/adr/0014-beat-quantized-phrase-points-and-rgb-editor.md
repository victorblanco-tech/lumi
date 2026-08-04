# ADR-0014: Beat-quantized Phrase Points and RGB Track Editor

- Status: **Accepted**
- Date: **2026-08-04**
- Supersedes: the bar-only phrase-boundary decision in ADR-0012

## Context

ADR-0012 deliberately constrained Lumi phrase boundaries to complete bars. The
first Epic 2A implementation followed that decision, but hands-on review showed
that it conflicts with the established Rekordbox/CDJ preparation workflow. A
DJ needs to navigate and zoom the real waveform freely, place the playhead on an
exact beat, and create the next musical marker with the same mental model as a
Memory Cue. A block-based waveform and bar-only mutations obscure the audio and
prevent valid phrase changes on a beat inside a bar.

Phrase points differ from cues only in their derived duration: a point starts a
phrase and the next point ends it. This makes a separate end marker redundant
and creates a simpler contiguous timeline.

## Decision

Lumi stores a phrase timeline as ordered Phrase Points on canonical beatgrid
positions. Each point contains a `PhraseRoleId` and starts one derived phrase
range. Its end is the next point, or the track end for the final point.

Phrase mutations are quantized to one whole beat. They cannot create a boundary
between beats, a duplicate point, a zero-length phrase, a gap, or an overlap.
Time in milliseconds remains derived from the active beatgrid and is not the
canonical phrase identity.

Quantization does not constrain navigation. Audio playback, playhead movement,
scrubbing, horizontal pan, and zoom are continuous. Bars remain prominent
visual and keyboard-navigation units, but they are no longer the only legal
phrase boundary.

The native editor adopts a Rekordbox/CDJ-inspired hierarchy:

- continuously rendered detailed waveform rather than block tiles;
- RGB as the default waveform color mode;
- beat ticks, stronger bar lines, and bar numbers above the waveform;
- Phrase Point markers and derived phrase ranges;
- a compact full-track overview below the detailed editor;
- viewport, playhead, and phrase ranges shared by both scales.

RGB is a presentation default. Provider adapters deliver neutral waveform band
or sample data and do not emit UI colors. Alternative Blue or 3Band renderers
may be added later behind a preference without changing persistence or adapter
contracts.

## Consequences

- The E2A-05 bar-based aggregate and SQLite representation require a versioned,
  backward-compatible migration to beatgrid positions.
- Existing bar-aligned timelines migrate losslessly because every stored bar
  boundary is also a legal beat boundary.
- Split, create, move, delete/absorb, undo/redo, revision restore, source rebase,
  and phrase-loop logic must operate on beat positions.
- Planner and execution contracts remain beat-based and therefore become more
  direct; they no longer need to infer an intra-track beat from a bar-only edit.
- Visual tests must prove waveform continuity at multiple zoom levels and must
  reject the previous block-tile presentation.
- Accessibility exposes point role, bar, beat, derived end, and duration without
  relying on color alone.

## Rejected alternatives

### Keep bar-only boundaries and improve only the drawing

Rejected because it preserves the interaction mismatch: the waveform would
look familiar while valid beat-level phrase placement remained impossible.

### Store arbitrary time or sample offsets

Rejected because Lumi planning and execution are beatgrid-driven. Absolute time
boundaries become fragile when source analysis or beatgrid alignment changes.

### Store separate start and end handles

Rejected because independent handles can create gaps and overlaps and do not
match the desired cue-point workflow. Ordered start points derive every end
unambiguously.

### Default to Blue or 3Band waveform rendering

Rejected for the Track Editor default. RGB is the accepted, familiar visual
mode; alternatives may remain future preferences.
