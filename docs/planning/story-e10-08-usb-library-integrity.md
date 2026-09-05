# E10-08 — Predictable USB sync and Library identity

Status: In progress. Extends E10-03; no production release without acceptance.

## User contract

Independent USB media retain independent identities and subscriptions, including
equal-model sticks with overlapping exports. Preserve existing Lumi phrases,
protection, MIDI mappings and history. A last-known suitable plan for the same
track is preferable to stopping lighting because physical-source evidence is
incomplete. This does not authorize treating an unrelated track with a colliding
Rekordbox ID as that track. Keep USB and library work outside the realtime lanes.

## Ordered implementation and evidence

1. **Revision and matching integrity:** order OneLibrary analysis/cue counters
   only within the same master identity; hashes prove difference, not freshness.
   Compare complete audio content before automatically merging track identities.
   Preserve source links across metadata-only changes; distinguish changed edits.
2. **Sync boundary:** validate the incoming source snapshot again before commit;
   reject stale analysis/selection, keep the old database on failure, and provide
   actionable per-source completion/held/conflict feedback without layout jumps.
3. **Media identity:** small versioned identity marker, independent of label and
   export contents. Migrate existing links without reset. Marker I/O must be
   bounded and isolated; missing, copied, malformed or unwritable markers cannot
   silently merge sources. No writes to PIONEER or music. Sync time is not proof
   of track freshness. Owner authorizes only identity metadata on removable media.
4. **Local/live use:** select compatible audio/analysis versions; preserve source
   context and explain last-known-plan fallback. Do not introduce blocking remote
   file fetches, hashing or new network traffic into live processing.
5. **Acceptance:** temporary-fixture fault tests plus native UI on connected GRAY;
   test repeated unchanged sync, metadata-only update, old/new counters, conflicting
   editions, removal/reconnect, remembered selections and stable operation status.
   Two-physical-stick and CDJ marker lookup require their own hardware acceptance.

## Baseline review

Source identity included the volume label. Analysis promotion assumed every
changed same-source revision was newer. Live lookup discarded source player and
slot. Metadata+size matching preceded audio verification. Audio signatures sampled
only the file ends; playback selected the first existing path. Sync transactions
and isolated workers already exist and should be retained, not replaced wholesale.

No existing user data may be cleaned up to make a test pass. Record actual test
results and remaining phases; a successful scan is not successful synchronization.

## 2026-09-05 — dev-9 implementation and acceptance

Implemented the first three boundaries: versioned optional identity marker in a
separate 3-second worker; full audio-container hashing; track-scoped monotone
OneLibrary counters; database/selected-analysis revalidation before the existing
atomic commit; per-source progress with reserved height and compact header status.
Whole backup hydration is now prepared in staging before activation as well.

Exact equality of parsed beatgrid, hot cues, source phrases and waveform clears a
hash-only conflict without promoting or replacing the active provenance. A scan's
initial impact is explicitly an initial comparison: sync verifies complete audio,
so its final matching counts may differ. Moved playlists are selected again by
the user; matching an old numeric playlist ID is no longer accepted as a stored
subscription.

Evidence:

- 137 Rust library tests passed (4 intentionally ignored), strict Clippy passed;
  59 Swift Library tests passed. Native Dev build and signed local DMG checks pass.
- Mounted GRAY was first synchronized against a disposable SQLite backup. The
  old stored playlist ID was rejected without commit; the current playlist (68
  tracks) synchronized successfully. Debug fixture run took 189 seconds; this is
  not a release-build latency benchmark.
- Native desktop: GRAY scan, persisted identity, map expansion, impact selection,
  actual sync, restart, remembered playlist and two repeat syncs exercised. Visible
  `SYNC 18/68` and determinate progress confirmed; collapse during sync retained
  a compact status and did not open a separate completion panel.
- GRAY stayed separate and connected; CHRM stayed separate and offline. Only GRAY
  was physically connected, so this does not replace two-stick hardware testing.
- Actual Dev library: 109 → 114 tracks; GRAY 68 active matched aliases, 67 current,
  one genuine component conflict (Doo Pah). Nine initially hash-only conflicts
  were eliminated by parsed equality. No existing phrase head changed; prepared
  tracks retained revisions 38 and 49. AutoLoop variant rows unchanged; SQLite
  integrity check `ok`. Backups/evidence are local ignored build artifacts.
- `.lumi-media.json` contains only schema, media UUID and source ID. GRAY's
  Rekordbox database SHA-256 was unchanged before/after registration.

### Still open — do not call the epic/release complete

- Native Tracks/Editor acceptance hit sustained AppKit/SwiftUI layout work and
  Computer Use timeouts after selecting a table row in the small window. A stack
  sample was retained; engine remained alive and the sync/data checks succeeded.
  This does not prove a new USB-code regression, nor prove that editor playback
  is acceptable. Diagnose and retest before production release. The window
  process was stopped for recovery; no show was running, no database was reset.
- Phase 4: version-compatible local audio choice and retaining source Player/slot
  context in live lookup. No new live NFS/USB reads were added here.
- Test simultaneous equal-model sticks, physical remove/reconnect and real CDJs;
  malformed/duplicate/read-only-marker fixtures are not equivalent to hardware.
- Legacy tracks without stored complete audio fingerprints require migration
  evidence; exact pre-sync audio matching needs an explicit preflight stage if
  final counts must be known before the Sync action.
