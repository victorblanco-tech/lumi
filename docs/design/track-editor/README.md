# Track Editor – RGB waveform and phrase-point UX

- Status: **Implemented in E2A-13**
- Accepted: **2026-08-04**
- Product language: **English**
- Reference interaction: **Rekordbox/CDJ waveform editing**
- Delivery story: [E2A-13 – Track Editor phrase-point workflow](https://github.com/victorblanco-tech/lumi/issues/65)

## Accepted outcome

The Track Lighting Editor uses the familiar Rekordbox/CDJ editing model rather
than a block-based waveform or form-first timeline editor. Its primary canvas is
a continuously rendered waveform with unrestricted horizontal pan and zoom.
The beatgrid stays aligned to the audio at every zoom level and gives every bar
a stronger visual boundary and number.

The default waveform color mode is **RGB**. Lumi does not default to Rekordbox
Blue or 3Band rendering, and E2A-13 does not add a mode selector. A future
preference may add alternative renderers without changing the analysis or
phrase model.

## Phrase-point model

A Lumi phrase is authored by placing one start point, analogous to placing a
Memory Cue:

1. Move the playhead freely to the intended musical position.
2. Choose the Phrase Role.
3. Add a Phrase Point.
4. Lumi quantizes the new point to the nearest whole beat.
5. The phrase runs automatically until the next Phrase Point.
6. The final phrase runs automatically to the end of the track.

There is no separate end marker. Adding, moving, or deleting a Phrase Point
recomputes the two adjacent derived ranges atomically. The canonical timeline
remains contiguous, cannot overlap, and cannot contain a zero-length phrase.

Quantize applies to phrase mutations, not navigation. The playhead, audio
scrubbing, waveform pan, and zoom remain continuous. Phrase boundaries are
stored as beatgrid positions, never as absolute milliseconds.

## Layout

From top to bottom, the accepted editor contains:

- track title, artist, BPM, key, duration, and beatgrid readiness;
- play/pause/stop, precise time and bar/beat position;
- Phrase Role and `Add Phrase Point` action with `Quantize · 1 beat` status;
- a large detailed RGB waveform;
- beat ticks, stronger bar lines, bar numbers, phrase markers, and playhead;
- a derived colored phrase-range lane immediately below the detailed waveform;
- unrestricted horizontal position control;
- a compact full-track RGB waveform below the editor;
- a visible viewport window, full-track playhead, and all phrase ranges;
- a selected-phrase inspector with role, derived start/end, length, source,
  optional Autoloop override, and late-bound Theme status.

The full-track overview is navigation, not a second editor. Clicking it moves
and centers the detailed viewport. Clicking a phrase range selects its start
point and opens the same inspector state in both waveform views.

## Interaction requirements

- Click or drag in the detailed waveform to move the playhead freely.
- Zoom from a useful multi-phrase view down to individual beats without changing
  waveform geometry or turning audio into block tiles.
- Scroll horizontally at every zoom level.
- Click the overview to jump and center the detailed viewport.
- `Add Phrase Point` and marker drag/move snap to one whole beat.
- Left/Right moves one beat; Shift+Left/Right moves one bar.
- Space toggles play/pause; `P` adds a Phrase Point; Delete removes the selected
  point when timeline validity permits it.
- Selecting or editing a point never interrupts ordinary audio preview.
- Every accepted mutation creates a recoverable Lumi timeline revision.

## Design artifacts

- [Interactive RGB Track Editor proposal](rgb-phrase-editor.html)
- [Accepted desktop preview](rgb-phrase-editor.png)

The interactive proposal is a UX reference, not production code. Product code
must use the native Lumi Design System and the provider-neutral waveform,
beatgrid, and phrase-domain contracts.

## Implementation evidence

The native implementation is in `TrackLightingEditorView.swift`. Automated
evidence is rendered to `build/VisualEvidence/track-editor-dark-camelot.png`
and `track-editor-light-host-classic.png`. Domain, SQLite migration, Swift wire,
audio-loop, simulator, planner, undo/redo, and reconciliation tests all use the
same canonical beat positions.
