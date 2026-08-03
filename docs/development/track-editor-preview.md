# Track Lighting Editor waveform and audio preview

E2A-04 turns the Library inspector action into a real CDJ-inspired Track
Lighting Editor. The editor is deliberately fixed-dark in Lumi's dark, light,
and system appearances, while key notation still follows the global
Camelot/Classic preference.

## Runtime boundary

Opening an editor sends a non-mutating `openLibraryTrackEditor` command to the
local Rust engine. The returned library snapshot contains one bounded,
provider-neutral analysis projection:

- canonical track metadata and a read-only audio URI;
- the complete beat grid with bar and beat indices;
- low, mid, and high colored waveform samples;
- source phrase observations for the initial read-only phrase lane.

Open and close commands never advance `stateRevision`, open the output gate, or
change a Live/Next plan. Swift decodes the projection into `TrackEditorAnalysis`.
The bar/beat grid, performance waveform, phrase lane, overview, scrubber, and
playhead all use the same beat-coordinate mapper. Every viewport starts and
ends on a complete bar; individual beats remain visible at every scale.

## Local read-only audio

`TrackAudioPreviewResolver` accepts only repository-owned `lumi-demo://` audio
or readable local file URLs/absolute paths. Network and unsupported schemes
fail closed. A missing, moved, empty, unsupported, or corrupt source leaves the
track analysis editable and shows an explicit preview-unavailable diagnostic.

Local files are opened for reading and are never copied, tagged, rewritten, or
deleted. Demo audio is generated deterministically in memory. AVAudioEngine and
its player node are created lazily only when Play is pressed, so inspecting a
track does not claim an audio device. Closing the sheet, switching engine state,
or disconnecting the local helper stops playback and releases the audio graph.

## Controls

- Play/Pause and `Space` share one deterministic transport action.
- Stop resets to the start of the track.
- Left/Right Arrow and the previous/next buttons seek to an exact bar boundary.
- Dragging the main waveform seeks and scrubs through the shared beat map.
- Dragging the overview moves the complete-bar viewport.
- Volume affects preview audio only.
- `Loop selected phrase` schedules the exact complete-bar phrase range.

Phrase editing is intentionally delivered by E2A-05. Its mutations will replace
the phrase projection without replacing the isolated audio transport, so valid
edits can update loop boundaries without interrupting ordinary preview.

## Verification

The feature package proves bounded decoding, invalid/incomplete grid rejection,
all complete-bar zoom scales, coordinate inversion, viewport clamping, safe
source resolution, and bar/phrase transport boundaries. Rust tests prove editor
open/close projection and unknown-track failure. The real Swift process test
opens and closes an editor against the bundled Rust helper and proves that show
state revision remains unchanged.

Two deterministic PNGs cover the editor under dark and light host appearances;
the editor remains fixed-dark in both. The canonical hands-on build is launched
only through:

```bash
open -n /Users/victor/Engineering/Repo/Lumi/build/DerivedData/Build/Products/Debug/Lumi.app
```
