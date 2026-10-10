# E10-08 — Predictable USB sync and Library identity

Status: In progress. Extends E10-03; no production release without acceptance.

## Dev-31 — stable identity with duplicate FAT volume UUIDs (2026-10-10)

Root cause reproduced with both independent same-model sticks attached: their
on-disk FAT volume UUIDs are equal. Foundation's `volumeUUIDString` returned a
different, session-specific UUID for the second mounted volume, while Disk
Arbitration and diskutil still returned its real filesystem UUID. The old
fingerprint persisted that session value, so the same stick conflicted on a
later mount. A previous manual reconfirmation persisted the transient value
instead of fixing the reader. This was not evidence of a copied marker.

- Read mounted-volume UUID/name through Disk Arbitration; retain the existing
  serial/UUID/name fingerprint format, so separate equal-model sources remain
  separate without using the session UUID.
- Require the actual mount root, complete filesystem evidence, and unchanged
  disk/description during observation. Do not turn a partial read into a new
  identity. Open the saved security scope before reading worker identity.
- Use the same reader for UI registration/presence and isolated USB workers.
- Keep copied-marker conflict protection intact. Do not automatically clear an
  old conflict just because a marker or label matches.
- User approved a narrowly scoped recovery of the affected GRAY registration
  using its original backup fingerprint, after the code fix and a fresh backup;
  no track, phrase, mapping or sync-selection recovery/rollback is authorized.

Verification so far: 78 Library tests passed, including ten read-only GRAY
observations. With both sticks attached, a further 15 identity tests passed,
including ten CHRM observations matching its original stable fingerprint.
The actual macOS application compiled successfully; five Rust network-media
tests passed, including copied-marker conflict and source-isolated matching.
Dev-31 (build 431, ba3ba52) was packaged, signature/installer audited and installed.

Actual desktop acceptance: both sources appeared separately as connected. GRAY
scanned without renewed authorization, restored six selections, registered its
stable identity and completed Sync 6 Playlists with visible 18/149 progress and
a completion report: 148 current, zero updated, one invalid-grid skip (existing
version retained). No identity conflict recurred. The user-approved single-row
recovery was made after a fresh backup; the entire database outside the binding
table had an identical dump hash before/after recovery. After sync, all phrase,
timeline, protection and source-phrase-mapping table hashes still matched the
pre-test backup. Database quick_check passed and both binding conflict flags
remained zero. CHRM's UI scan currently awaits the user's normal folder grant;
its physical reader acceptance above is not a claim of completed CHRM UI sync.

## Dev-18 — isolate invalid beatgrids within a sync

- Explicit user-approved policy: an invalid beatgrid does not abort otherwise
  valid selected playlist tracks. Never synthesize or silently reorder beats.
- Validate parsed incoming grids before promotion/import. Exclude invalid tracks
  from analysis, cue and metadata promotion; retain a verified existing canonical
  match and its timeline when available. A new invalid track is not imported.
- Persist an exact source/track warning on the alias (`held-invalid:`). Display
  names/reasons under that USB and a skipped count, including after app restart.
  Retrying a valid repaired revision clears the warning through normal sync.
- Playlist structure and valid memberships commit together in the existing
  transaction. Unchanged-file verification and rollback protections remain.
- Scope: non-increasing/inconsistent/incomplete beatgrids detected after parsing;
  unreadable media, changed source snapshots and unparseable containers remain
  safe operation failures rather than being silently accepted.
- Acceptance: replay against a database copy succeeded; native Dev-18 build 391
  repeated the exact six user-selected GRAY playlists successfully with visible
  32/149 progress and completion. Result: 149 selected, 148 available, only
  `Our Origin (Extended Mix)` excluded; 8 incomparable revisions retained for
  review. All six stored playlist paths were present in the replay result.
- Post-sync live database quick_check OK, 154 tracks. Compared every previously
  active phrase point (beat, role and loop strategy) and all AutoLoop variants
  with the pre-sync backup: zero differences. A source-reconcile revision on
  one existing mashup retained identical phrase points.
- Actual quit/reopen preserved GRAY's `1 SKIPPED` and `148/149 matched` without
  requiring a fresh scan. Newest Dev-18 remains open in Import & Sources.
- Tests: 145 engine tests passed, 4 ignored; 69 Swift Testing + 2 XCTest passed;
  strict Clippy and package/installer checks passed. No physical light-output
  acceptance is claimed for this USB-only test.

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
## Dev-19 — bulk review

- Per-track selection and source-scoped Select all/Clear in Tracks to review.
- Keep Lumi persists the existing exact-revision decision; Use USB Version requires a count/component confirmation.
- Sequential processing reuses the isolated worker and its incoming/active revision validation, stops on the first failure, and does not roll back earlier successful choices.
- Existing Lumi-authored phrases and AutoLoop choices remain protected. No changes to USB age ordering, waveform rendering, or realtime integration lanes.
- GRAY investigation: eight reviews in the Dev-18 sync differed only in hot cues; beatgrid, RGB waveform, metadata and raw RB phrases were identical. Actual GRAY cueUpdateCount was NULL (the importer currently represents this as zero), analysisDataUpdateCount was 2. dateCreated/dateAdded exist but there is no per-cue modification date in the exported schema; do not infer cue freshness from those dates.
- Regression: queue preserves order and exact revisions; failure clears pending requests. 69 Swift Testing tests and 4 XCTest tests passed.
- Native desktop acceptance on a separate database copy: recreated all eight GRAY cue-only reviews; Select all showed 8 selected, Clear restored 0, bulk Use USB confirmation showed 8 tracks and Cancel preserved the reviews. Bulk Keep Lumi sequentially resolved all eight; SQL confirmed all eight exact aliases as kept-active and quick_check OK. Actual Dev alias/disposition data remained identical to the pre-test backup. Bulk overwrite execution was not performed on the user's library.
## Follow-up — service lifecycle and user-visible status (2026-10-08)

User-reported priority for the next release: service start/stop, lifecycle status and connection recovery remain unclear and can produce unstable startup behavior. During the Dev-19 launch the UI reported an unknown local engine error and an unavailable library; a graceful stop of the identified Dev engine followed by relaunch restored the 154-track library. Root cause is not established.

Investigate service ownership and stale connections across UI relaunch, deterministic readiness/recovery, clean shutdown, and consistent non-technical status (starting, ready, recovering, stopped, actionable failure). A Ready indicator must not imply a usable engine while the library is unavailable. Keep service recovery separate from USB sync and realtime output, with regression tests for existing service attachment, failed connection, restart, and shutdown. No lifecycle implementation change made during the user's current hardware acceptance test.
## Next-release follow-up — persistent USB authorization (2026-10-08)

Known GRAY was connected after app startup, but Choose Playlists requested authorization again before scanning. User requests investigation and correction in the first next release, not during the current hardware acceptance flow. Root cause is not established: inspect security-scoped bookmark persistence in the stored JSON, source-ID association, stale-bookmark resolution and renewals across relaunch/reconnect. Reuse valid authorization for the same verified USB; request authorization only when missing or genuinely invalid, with an actionable explanation. Never relax physical USB identity checks or silently grant access to a different stick.

Also make the initial connected-source state clear: distinguish stored synchronized playlists from the current selectable USB tree and offer an obvious scan/load action without suggesting a sync has already occurred. Regression coverage must include app relaunch, eject/reconnect, both independent same-model sticks and stale bookmarks. No app or sync changes performed for this follow-up.
## Next-release blocker — GRAY identity conflict after reconnect (2026-10-08)

Actual Dev-19 desktop failure on Sync 6 Playlists after successful read of GRAY (86 playlists, 1208 tracks): worker reports `USB identity conflicts with another physical source; no automatic identity was accepted`, wrapped as an engine service configuration failure. Source lane correctly states that no tracks were imported. This is not evidence of an uninitialized library.

Read-only evidence: the connected stick's marker still matches its known source registration, but the local binding is marked conflicted again. The other trusted stick is offline and its binding remains unconflicted. A similar fingerprint conflict had previously been explicitly re-confirmed; recurrence is a regression/reliability concern. Current observed fingerprint and why it differs are not established in this investigation. Do not automatically clear the conflict or assume sticks were cloned. Specific local identifiers and observation timestamps are omitted from public documentation.

For the first next release: trace identity observations across scan versus sync and reconnect, validate stability of physical fingerprint inputs (including hardware serial availability), distinguish genuine copied-marker collisions from missing/changing observations, provide a safe in-app confirmation/recovery path, and prevent an enabled sync action from concealing an already-known identity blocker. Replace technical configuration-error wording with the actual USB identity problem. Test with both independent same-model sticks and reconnect/relaunch after a real RB export. No identity binding, track data or app code modified for this report.
## Next-release blocker — editor remains loading after failed USB sync (2026-10-08)

User reports Track Editor fails with either USB connected individually. Actual desktop observation: collection still lists 154 tracks, but editor remains `Loading the selected track…`; a selected row retains its loading indicator and selecting the known prepared test track did not produce an editor. Read-only database quick_check returned OK. This proves an editor-load failure, not an empty collection, and does not yet establish its root cause or causal relationship to the preceding USB identity failure.

Code inspection identifies a feedback gap: openLibraryTrackEditor failures are presented as timelineEditFeedback, while the no-editor placeholder displays only library condition/diagnostic or a loading message; timeline feedback is passed to the loaded editor. Investigate request completion/cancellation and command-lane recovery after USB worker failure; ensure failed editor loading clears the pending spinner, displays the actual error and supports retry. Add end-to-end regression for relaunch → known USB authorization/scan → rejected sync → open existing editor and switch tracks, with each same-model USB individually. Do not require USB identity confirmation merely to display already imported local beatgrid/phrases. No app, library or USB data modified during this investigation.
## Next-release blocker — playlist selection leaves stale collection (2026-10-08)

User reports expanding Library folders and choosing a leaf playlist does not filter the track table. Reproduced in the actual Dev-19 UI: the 154-track collection remained after clicking a specific stored playlist. This is not a successful playlist-browser acceptance test; earlier tree-only tests did not establish filtering correctness.

Code inspection: queryLibrary returns silently on an EngineCommandFailure when the prior library condition is ready; the catch path likewise only presents an error when condition is importing. Therefore a failed query can leave an apparently valid stale collection with no user feedback. The runtime request failure/root cause remains unproven. Investigate request delivery, command-lane health and response application, and make selection/query/table state atomic. Show actionable query failure and retry rather than presenting old rows as the selected playlist. Acceptance must compare exact membership against stored playlist_tracks for two different leaf playlists and Collection, including after USB sync failure and service reconnect. Preserve all library data and realtime integration lanes. No app code changed during this investigation.

## Next-release timing blocker — first AutoLoop after transport starts (2026-10-08)

Hardware acceptance report: starting a newly loaded track from complete standstill starts the lighting late; the AutoLoop then remains off-beat rather than recovering. This affects the opening of a set. Measured latency and the responsible lane are not established. Record separately from playlist/editor failures; no runtime changes made during this acceptance session.

Proposed implementation: while armed, prepare the confirmed master track's current phrase, plan and MIDI command before playback. In Start mode, use authoritative Pro DJ Link transport/position/beat information to dispatch the current phrase's AutoLoop once at transport start, without waiting for a phrase boundary, UI update, plan generation or USB read. Deduplicate by loaded-track generation and transport-start event; discard stale pending commands after track/master changes. Keep Ableton Link tempo relay independent, with no continuous phase correction or repeated loop retriggering.

Account for an important boundary: an incoming play/beat event necessarily arrives after its source timestamp. Preloading cannot guarantee zero end-to-end delay or compensate an unknown physical button press in advance. Measure source event arrival, start detection, MIDI dispatch and observed SoundSwitch activation separately; verify SoundSwitch launch/quantization behavior and whether a one-time beat-aligned launch is supported. Do not add speculative MIDI clock, phase resets or periodic timeline corrections. If immediate start is already late, do not silently pretend it was beat-perfect; select an explicitly validated one-time alignment strategy, subject to user agreement if it delays the opening to a later beat.

Acceptance: repeated starts from a paused hotcue (including the first load), Off → Arm → Start before playback, Start while already playing, pause/resume, hotcue jumps and master transitions, with real CDJs and SoundSwitch. Test configured signed offsets separately: a negative offset can anticipate a known future phrase boundary, but cannot anticipate an unpredictable first Play press. Capture measurable trigger latency and beat alignment, prove exactly one trigger per required start and stable AutoLoop playback afterward, and verify foreground/background UI activity does not affect the integration lanes. Do not mark fixed based only on UI movement or mocked MIDI delivery.
