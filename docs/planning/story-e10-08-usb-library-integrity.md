# E10-08 — Predictable USB sync and Library identity

Status: In progress. Extends E10-03; no production release without acceptance.

## Dev-17 — sync failure visibility and confirmed local GRAY recovery

- A failed sync retains its source ID and shows an actionable explanation in the
  existing fixed-height progress area of that USB. It is no longer tooltip-only;
  other source lanes do not inherit the error and the page does not grow/shrink.
- Native GRAY sync reproduced a sticky identity conflict before import. Its
  marker and filesystem UUID matched the known GRAY, but the current hardware
  fingerprint differed from its earlier binding. After explicit user approval,
  backed up the database and re-confirmed only that exact local binding. No
  automatic weakening of copied-marker rejection is introduced. The cause of
  the historical fingerprint difference is not yet established.
- The actual selected six-playlist sync then showed track/audio progress, but
  failed on a non-increasing Rekordbox beatgrid. Add the failing track name and
  USB track ID to the error; retain strict validation rather than inventing beats.
  Diagnostic replay against a separate database copy identified `Our Origin
  (Extended Mix)` (USB track 1283). Independent read-only PQTZ decoding confirmed
  source beat index 890 goes from 404463 ms back to 404082 ms. This is not a
  rendering or invented-grid issue. User decision requested: retain/exclude the
  invalid track and sync valid tracks, or first repair its Rekordbox analysis.
- Evidence: 69 Swift Testing tests plus 2 XCTest feedback tests passed; 143 engine
  tests passed (4 ignored), a separate duplicate-time regression passed, strict
  engine Clippy passed. Package/installer checks passed, build 389. Actual native
  six-playlist sync showed 17/149 progress, then rejected the malformed source;
  this is explicitly not a successful full-sync acceptance. Live database remains
  120 tracks, 278 timeline revisions, 139 playlist memberships; quick_check is OK.
- No waveform, live timing, phrase ownership, MIDI or Remote behavior changes.

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

## 2026-10-04 Reproduced gaps and approved next work

Review used actual USB workers with temporary encrypted OneLibrary fixtures and
temporary Lumi databases. 199 existing local tests passed; four engine tests
were intentionally ignored. Ten repeated syncs retained the same row counts. A
50 ms grid shift was imported exactly and a manually edited Lumi phrase role
survived. No physical USB was mounted, so this is not hardware acceptance and
does not establish the cause of the owner's previous failed show test.

The owner approved repairing the following four boundaries:

1. Source-blind live lookup can be ambiguous or wrong for colliding USB track IDs.
   The owner chose automatic identification, not manual Player assignments.
   [E10-09](story-e10-09-network-media-resolution.md) prepares an isolated NFS POC;
   [ADR 0045](../architecture/adr/0045-automatic-network-media-resolution.md)
   defines the hardware gate before production integration.
2. A track without source phrases can sync with no timeline, then fail live
   hydration and trigger global bridge recovery. Introduce per-track preparation
   outcomes; an unavailable plan must not reset healthy Players or Link.
3. A Player loaded before sync receives no new load event when the same identity
   remains loaded. Notify/revalidate after committed library changes off the show
   pump and adopt prepared changes safely without replaying a cue.
4. Replacing the same source/track alias overwrites its old audio fingerprint.
   A later import of the original prepared audio can become a new canonical row.
   Preserve fingerprints independently of aliases and choose only compatible
   audio locations. Existing edited tracks and user configuration must survive.

These are prepared requirements, not completed fixes. Additional review findings
about missing unrelated files, missing key metadata, incomplete metadata updates,
duplicate editions and durable sync reports remain tracked separately from the
four approved repairs. The production application has not been changed by this
preparation task.

## 2026 10 05 implementation

The four approved repairs are implemented in 0.6.4-dev-4: exact trusted-source
lookup, per-track preparation outcomes, read-only revalidation after committed
sync, and schema 20 fingerprint history. Pro DJ Link source Player/slot and load
identity are retained through preparation. An unresolved loaded track can become
ready without a reload; already prepared tracks retain their plan and defer new
data until a real reload. Compatible audio selection rejects stale aliases rather
than using an old path that now belongs to another edit.

Regression coverage includes colliding IDs, alias replacement, fingerprint
migration, concurrent snapshot reads, commit after load, preserved playback/BPM,
stale completion rejection, reader faults, source-specific invalidation and a
deferred timeline update without an additional lighting dispatch. Local fault
fixtures do not replace real USB/CDJ or light-output acceptance. Physical testing
is scheduled with the owner after the autonomous implementation.

Native testing caught an offline-USB editor contract failure in dev-3 before
handoff. Dev-4 uses an explicit non-file unavailable audio URI instead of an
empty value; stored phrases/waveform remain editable without a mounted stick.
Playback must report unavailable, never synthesize music or fall back to a stale
unverified path. Rust and Swift regressions cover this boundary. Detailed local
test evidence and the remaining physical gates are recorded in E10-09.

## Playlist hierarchy release blocker from 2026 10 07

The owner requires an expandable playlist tree in Tracks/Editor before 0.6.4.
The old browser rendered the correct full stored paths as one flat list. The
same presentation defect existed in the Local Playback library browser.

Dev-13 adds one shared renderer, separate native folder metadata in schema 21,
and source/ID/path-scoped enrichment during a scan. A legacy USB needs one scan
to recover its exact native folders; no track sync is needed for enrichment.
Unknown legacy names are not split speculatively, avoiding synthetic folders
when a name itself contains `/`. Active selection reveals its parents; collapsing
a folder does not change the track query or start playback.

Local tests cover nested/collapsed folders, retained leaf IDs, duplicate leaf
names, literal slashes, invalid/unknown legacy metadata, exact-source enrichment
and schema-20 migration preserving tracks and playlists. Native desktop
acceptance and final gate results are recorded after installation, not inferred
from these tests. Network/timing and waveform code are unchanged.

## Follow up for the next release

On 2026-10-07, the owner deferred the misleading USB source `CURRENT` badge
to the next release. It is not a blocker for the planned 0.6.4 release; no
runtime or sync behavior is changed by this follow-up.

Reproduction on 0.6.4-dev-12: connected CHRM showed green `CURRENT` from its
previous sync before a fresh inspection. A read-only scan then reported 71
unique tracks in the two selected playlists: 9 new, 29 possible updates,
27 current and 6 for review. No Sync action was performed, but the source badge
still showed `CURRENT` and the inspector mixed previous-sync counters with
the current inspection.

Acceptance for the follow-up:

- Before a fresh comparison, show that the connected USB has not yet been
  checked rather than claiming it is current.
- After inspection, summarize the current differences in the source row;
  use wording such as `Updated tracks` when updates are present, with distinct
  new-track and review counts where relevant. Do not imply they were imported.
- Mark retained sync counters and their date as results of the last sync,
  separate from the current scan and selected-playlist impact.
- Before Sync, expose a browsable list of every new or potentially updated
  track, with its title, artist, change status and full USB playlist path
  including parent folders. Expand the aggregate New/Update counts into these
  records; provide a changes-only filter so the owner need not search every
  folder to discover what Sync will affect.
- For an update, explain which compared components differ (audio/file identity,
  beatgrid, cues, source phrases or metadata, including track color) using the
  available evidence. Distinguish unknown or unverified differences from a
  confirmed newer version; do not label every difference as newer.
- Show all relevant playlist memberships when a changed track occurs in more
  than one playlist, but retain unique-track totals and one import per track.
  Make the selected sync scope explicit. Pre-sync inspection must not perform
  an import or discard existing Lumi phrases.
- Show a current/in-sync state only for the scope actually compared. Preserve
  per-source identity, remembered selections, fixed layout and isolated sync.
- Test initial state, changed and unchanged scans, source switching and sync
  completion without altering live timing or lighting output.
# Dev-14 — source and destination sync workspace

The USB playlist picker now presents two independent, fixed-height trees:
USB source and saved selection on the left; actual synchronized playlists for
that trusted USB in the Lumi Library on the right. Folder checkboxes support
none/partial/all selection. Saved user selection takes precedence over the
last completed sync, including an intentionally empty selection. Deselecting
never deletes library data. Destination membership comes from persisted
source-specific playlist relations, not from the current checkboxes.

Scope: presentation and selection restoration only. No live scheduling,
waveform rendering, MIDI, USB writes, track import or schema changes.
Automated and desktop validation are recorded separately; do not treat a
successful build as completed hardware acceptance.

Desktop evidence (CHRM, 2026-10-07): expanded destination shows the exact two
stored playlists (15 and 56 tracks). Clearing source selection disables Sync
but leaves both destination playlists intact. Original two selections restored
without importing tracks. The UI check exposed an empty-selection height jump;
the impact metrics now remain present at zero instead of replacing the panel.
Folder selection respects visible search results. Local Swift regression suite:
69 tests. Waveform and integration lanes are unchanged; physical light output
is not covered by this UI test.
# Dev-15 — current, decision-oriented USB review

Completed source scans are authoritative for review visibility and review
counts. Historical conflict rows no longer appear when the fresh comparison
is absent or all imported components are equal. Stored records are preserved;
a scan does not resolve a conflict by overwriting tracks.

Review cards show an honest uncertainty headline, conservative Keep Lumi
recommendation and only changed component names. Export dates and revision
fingerprints are in collapsed Details; neither establishes audio-file age.
The current File Data comparison is metadata, not verified audio freshness.
Use USB Version requires a fresh component comparison and retains the existing
confirmation and revision guards. No live lane or waveform changes.

Validation 2026-10-07: 143 engine tests passed (4 ignored), 69 Swift tests
passed, strict engine Clippy passed, installer build 385 audited. In the actual
desktop app, the stored GRAY/Doo Pah card renders compactly with Details closed
and Use USB Version disabled before verification. Opening and closing Details
works. After scanning GRAY, both the stale card and its orange review count
disappear. No Sync or overwrite action taken. SQLite quick_check remains OK;
120 tracks, 278 timeline revisions and 139 playlist memberships preserved.
# Dev-16 — available width and compact playlist statuses

Import & Sources uses the available content width rather than a 980-point
maximum. Playlist title/count and status badges occupy separate lines.
Short Same/Update/Held/Review/New labels retain their existing meanings;
tooltips clarify that Update does not prove newer audio. Adaptive status rows
and single-line intrinsic-size badges prevent narrow columns from stretching
capsules vertically. Selection, synchronization and integration lanes unchanged.

Validation: 69 Swift regression tests passed; audited installer build 387.
Actual desktop GRAY scan and Trancendence 2 browsing confirmed readable
single-line badges, including a playlist with four statuses which wraps into
two compact badge rows. Six saved playlist selections retained. No Sync action
performed during this presentation test. Dev-16 left open on the tested set.
