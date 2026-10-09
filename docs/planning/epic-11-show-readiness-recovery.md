# Epic 11 — reliable preparation and Live performance

Status: implementation and autonomous testing authorized on 2026-10-08.
Baseline: public Lumi v0.6.3; development candidate 0.6.4-dev-25.
Products: Lumi, Lumi Remote and the independently versioned Pro DJ Link Simulator.
Execution order: simulator fidelity first, then recovery and measured timing, preparation workflow, launch policy, integrated acceptance.

The earlier [recovery proposal](live-show-reliability-recovery-2026-10-08.md)
defines the timing investigation. This epic is the consolidated acceptance ledger.
Existing story completion and previous component measurements do not close a
newly reproduced regression. Every row needs a failing reproduction, repair,
automated regression and actual application acceptance where applicable.

Dev-25 / Remote dev-7 follow-up: desktop acceptance found that the launch
setting save status and countdown were omitted from the immediate presentation
refresh comparison. Include launch state changes and reserve stable header
space on both platforms. Mac presentation tests (63) and Remote feature tests
(14) pass. The new ignored simulator/MIDI acceptance exercises initial launch
at -250/0/+250 ms and rejects duplicate boundary sends. SoundSwitch downstream
acceptance remains open due to the independently sampled
[Control One lifecycle hang](soundswitch-control-one-hang-2026-10-09.md).

Actual simulator/MIDI run: all three signed-offset cases passed in 156.67 s.
Each selected phrase index 1, completed exactly one AutoLoop, sent two pulses
(bank + AutoLoop), reported zero failures and zero late sends, and remained at
one completion after the normal phrase boundary. Evidence:
`build/phrase-launch-network-acceptance.log`. This measured the real MIDI lane,
not SoundSwitch's visible response or physical lights. The harness initially
reused a command ID and was correctly deduplicated; the successful run uses
unique IDs. Clippy passes after adding a bounded disconnect shutdown check.

Packaged Dev-25 acceptance: build 413 / d79a9cc, installer signature and channel
destination audit passed. The installed Mac app recognized both prepared tracks
and CHRM local / CHRM via Player 1 sources. The actual Timing popover changed
Immediate / On phrase start, cleared Saving without an unrelated action, and
Remote dev-7 reflected the saved choice. Arm → Start while paused displayed
Waiting for playback. Simulator playback changed the Mac countdown from 95 to
79 beats; Remote also counted down. Returned to Off before the target boundary.
Remote retained all three status indicators while disconnected and reconnected
as its existing view-only role; no controller permission was changed.

Upgrade follow-up remains open: macOS recorded a first gateway launch constraint
violation at 12:48:29 before the normal service registration/retry recovered and
Remote connected. Do not count eventual recovery as clean upgrade acceptance.
Investigate registration ordering and installed service identity without
weakening macOS security checks. SoundSwitch restart approval is still needed
for downstream playback acceptance; Lumi is left open in Off.

## Findings and ownership

Additional Dev-25 desktop checkpoint (2026-10-09): while the installed app was
Live with the Mac mini simulator, Arm/Start from a paused position showed
Waiting for playback, then Show running after Play. The status popover reported
26 MIDI pulses, p95 5.0 ms, last 4.5 ms and zero late sends. SoundSwitch reported
one Link peer at 155 BPM and Control One connected. These observations do not
prove downstream AutoLoop playback: the automation view continued to expose
the mapping screen / loading logo despite the owner's Performance-mode setup.
No mappings were intentionally edited.

Concurrent Library browsing verified Sets / Trancendence / Trancendence 2 /
Part 1 - 138+ Trance: 15 rows; typing Shiver without Return narrowed to one;
Clear restored 15. The subsequent Part 2 selection and two further desktop
observations timed out, including screenshot-only observation. Do not count
Part 2 or continued UI responsiveness as passed. A three-second process sample
(`build/dev25-library-ui-sample.txt`) showed substantial SwiftUI transaction /
view update work rather than a single blocked main-thread stack. A point-in-time
process check showed the app, engine, gateway, bridge and Carabiner still alive;
process liveness alone does not establish healthy lighting output. Investigate
UI workload and automation responsiveness separately before assigning cause.

Follow-up: the owner confirmed Lumi remained responsive. Resetting only the
computer-use session and selecting the exact Dev-25 app path restored immediate
UI access; no Lumi/service restart was needed. Do not label that incident a
confirmed application hang. The restored status showed continuing Pro DJ Link
positions and 77 MIDI pulses / zero late sends, but a stale Link warning.
`LinkRelay::synchronize` cleared its stale flag before suppressing an unchanged
tempo/master observation, leaving the provider degraded. A failing-before /
passing-after regression now requires one recovery observation, then resumes
normal duplicate suppression. This source fix is not installed yet.

Release gate: inspection of the existing Carabiner provider found actual
`force-beat-at-time`, `start-playing`, `stop-playing` and start/stop-sync commands.
Consequently earlier descriptions of the complete provider as tempo-only were
too strong. Restoring a stale provider currently enters this alignment path;
do not deploy the recovery fix alone or claim timeline isolation. Complete the
owner's tempo-only contract and add wire-command assertions for first receipt,
pause/play, gaps, master changes and recovery before packaged acceptance.

| Finding since the public release / development acceptance | Story | Required evidence |
| --- | --- | --- |
| Approximate three-second lighting delay and slow displayed BPM changes | E11-03 | Correlated source-to-dispatch and downstream measurements under load |
| First Play/hotcue start is late and remains off-beat | E11-05 | Repeated immediate and phrase-boundary starts; exactly one launch |
| Start/stop/reattach services unclear; Ready despite unavailable engine | E11-02 | Cold/warm start, reconnect, upgrade and shutdown UI scenarios |
| Independent same-model USBs conflict again after local reconnection | E11-04 | Stable identity observations; copied-marker rejection retained |
| Authorized USB asks for authorization again; scan/selection view missing | E11-04 | Bookmark lifecycle and restored selectable source tree |
| Current/green status before a new comparison; new/changed tracks hidden | E11-04 | Scan impact by full playlist path, separate from last-sync result |
| Sync does nothing visibly or gives configuration/init wording for identity errors | E11-04 | Immediate source-scoped progress/failure, meaningful retry |
| Nested folders flattened or selecting a playlist leaves collection rows | E11-04 | Exact leaf membership, hierarchy, query cancellation and failures |
| Editor stays loading after sync failure, including imported local tracks | E11-04 | Failed-sync-to-editor sequence and visible bounded failure/retry |
| Review uncertainty and old/new semantics, bulk decisions, invalid-grid exclusion | E11-04 | Preserve accepted behavior and phrases; fresh scan and restart regressions |
| New mashup versions and changed grid/cues/metadata need safe adoption | E11-04 | Compatible audio, alias/fingerprint history, protected phrase retention |
| Mounted USB differs from a loaded LINK track's origin | E11-01/06 | Mac and Remote show both facts with evidence-based media colors |
| Eject cached local/LINK tracks, timeouts and delayed replies after swaps | E11-01/06 | Hardware-derived packet sequence and generation tests |
| UI stutter and presentation delays | E11-03/06 | No feedback into outputs; preserve waveform rendering and fixed playhead |
| Remote control ownership, persisted offset and Prod/Dev pairing | E11-06 | Retain earlier fixes through controller, reconnect and channel regression |

Detailed prior evidence remains in E10-07, E10-08 and E10-09. Cross-source matching
without verified identity remains deferred by the owner; do not silently add it
as a way to make simulator matching pass. Unknown track freshness remains honest;
no export/analysis date is invented as a cue/grid revision timestamp.

## E11-01 — hardware-derived simulator and repeatable scenarios

2026-10-09 update, simulator 0.4.1-dev-5: real destination-aware, unprivileged
RPC/MOUNT/NFS marker reads implemented (ADR 0046). Automatic one/two-interface
selection, independent source addresses, USB-status packet fields, bounded
media faults and visible network mode added. Native acceptance includes exact
SHA-256 through the unchanged production reader on port 111, source-address
verification, stale handles, eject, missing/oversize/symlink protection and
timeout recovery. Local browser acceptance uses explicitly synthetic sources.
Mac mini installation, real exports across its two physical interfaces and Lumi
show acceptance are still required; do not mark the complete epic done.

2026-10-09 cross-Mac checkpoint: dev-5 is running on the owner's Mac mini.
Primary Ethernet and secondary Wi-Fi addresses were discovered automatically.
The production reader on the MacBook read the existing CHRM marker over actual
RPC/NFS in approximately 62 ms. Player 2 has no independent USB; therefore two
independent physical media are not yet validated in this setup.

Historical checkpoint (superseded by the update above): simulator 0.4.1-dev-3 was built,
checksum-verified, and the packaged desktop UI was opened and inspected. No
Rekordbox USB was attached to this Mac, so no local simulator session was
started. The owner-supplied Mac mini still runs 0.4.0-dev-56; its control page is
reachable, but that older installation cannot verify the new media-slot and
eject behavior. No Mac-mini controls or production Lumi state were changed.

Implemented in 0.4.1-dev-3: independent USB libraries per Player, retained
source-Player identity for LINK-loaded tracks, mounted-versus-loaded source
state, cached source retention after eject, OneLibrary import, remote insert/eject
controls, and source-aware playlist Auto Mix. Unit and package verification pass.
Still open: test the new build on the Mac mini with two distinct USB exports,
verify its packets against new captures, and resolve production NFS source
addressing. The simulator does not yet model the CDJ's real USB identity RPC or
serve NFS media, so it is not full USB-source acceptance evidence.

Acceptance and implementation tasks:

- Independent mounted-media and loaded-track models per Player; source Player
  retained across LINK loads, cached playback, eject and replacement. Two media
  may contain the same numeric track ID with different audio/analysis.
- Use OneLibrary data through a shared, verified importer or equivalent validated
  reader. Reuse existing parsing where practical. Never synthesize an allegedly
  exact beatgrid when analysis is unavailable; show that limitation explicitly.
- Read only the real versioned identity marker, with the same size/schema bounds.
  Network marker retrieval must exercise the production reader and its limits.
  An HTTP identity shortcut or injected trusted result is not NFS acceptance.
- Resolve multi-player network addressing before shipping media emulation: the
  current two Players share one host address, whereas real NFS identifies a
  source by host/slot. Two independent media must not collapse to a single marker.
  Keep the simulator non-admin where possible; document any transport-specific
  test seam and its limits instead of weakening production source verification.
- Generate the observed CDJ-1500X status layout, mount transitions, stable
  post-eject ownership fields, extended content witness and declared wire size.
  Test through the actual 512-byte Beat Link receive boundary. Stable captures
  do not prove unobserved intermediate transitions; label those synthetic cases.
- Remote and visible controls for insert/eject, local/LINK load, unload, Play,
  cue/seek, pitch, master handover, loop and playlist Auto Mix.
- Deterministic faults: delayed/missing/malformed marker, stale reply after a
  replacement, status/position gaps, reordered ordinary observations and bursts.
  Preserve critical transitions; make all faults opt-in and bounded.
- Scenario runner records seed, operations, expected sources and event times,
  with clear progress/results. Include cached eject, ID collision, fresh load,
  reconnect, first Play, phrase run-in and long mixed-playlist soak.
- Package and validate a self-contained simulator DMG with its own incremented
  dev version. Do not claim Mac mini installation until verified remotely.

## E11-02 — usable service lifecycle

2026-10-09 Control One checkpoint, installed dev-23: SoundSwitch visibly reports
CONTROL ONE Connected. Quit through the desktop UI removed all matching Lumi
engine/gateway/bridge/Carabiner processes and removed the Lumi Link peer from
SoundSwitch, while Control One remained connected. Normal app restart recovered
both CHRM tracks (local Player 1 and LINK Player 2), one Link peer, and Arm → Start.
After restart the Live status reported eight MIDI pulses, zero late dispatches
and p95 4.6 ms; both applications followed the simulator pitch reset to 155 BPM.
These are software status observations, not a physical-light or first-beat
alignment measurement. No mappings or macOS privacy settings were changed.

Dev-22's earlier packaged USB failure recovered before installing dev-23; it is
not evidence that the new usage-description declaration fixed that failure.
The successful dev-23 warm restart does not close the complete startup/fault
matrix. Keep the intermittent initial marker-read failure under investigation.

Dev-23 preparation: the engine's embedded service Info.plist now includes the
local-network usage description, as the main app and Remote gateway already do.
Packaging rejects its absence. This is a declaration, not a consent grant and
not evidence that the packaged network problem is resolved. No process launch
or permissions workaround is introduced.

The Library editor pane now scrolls vertically when the saved editor height
cannot fit the window; the browser reserves 220 points. The editor keeps its
existing minimum content height, waveform renderer and saved height preference.
All 75 Library Swift tests pass (plus four XCTest feedback tests). Packaged
small-window/divider acceptance is still required.

Post-dev-22 checkpoint (source changes, not yet packaged): Live's aggregate
status now explicitly represents Stopped, Starting, Reconnecting and Unavailable
instead of declaring Ready while the engine is starting. Empty optional providers
remain informational. All 62 Live workspace tests pass. Remote process details
now require the executable to belong to the expected installation, not merely
a live PID from a private service record; all nine engine safety tests pass,
including wrong-installation/PID rejection and real child termination.
The complete macOS build also passed. The full client suite passed all 30 tests
after quitting the installed app. An initial run alongside that app failed MIDI
publication; it is not valid acceptance evidence. Focused reruns now use
`bash scripts/verify-engine-client.sh`, which applies the existing exclusive
MIDI ownership guard and serial execution from full Apple verification. The
test reports a publication command failure directly rather than cascading
through unrelated expectations.

Packaged dev-22 remains blocked for show acceptance: Player status/BPM and a
SoundSwitch Link peer are present, but the launch-agent USB marker reader reports
`No route to host`. The simulator reports its RPC/NFS service ready. The earlier
headless soak does not establish packaged network access. No privacy/network
settings were changed; the owner manages consent. Do not mark this resolved or
attribute the cause solely to permissions without evidence.

Packaged Library desktop acceptance passed: selecting Mashup ToDo immediately
showed seven tracks, typing Shiver without Return narrowed to two, clearing
restored seven, and selecting Part 1 showed fourteen. Small-window layout still
needs attention: a persisted tall editor can leave too little browser space;
maximizing reveals it, but is not a product fix. Waveform rendering is unchanged.

Dev-21 desktop checkpoint: Restart replaced engine PID 81158 with 81270 and
gateway PID 81176 with 81278, both from the expected Dev-21 bundle. Both Players
and their local/LINK plans returned; Arm → Start remained Live with the simulator.
Explicit Quit left no engine/gateway/Carabiner/bridge and SoundSwitch lost its
Lumi Link peer. SoundSwitch reports no hardware interface, so physical lighting
and Control One lifecycle acceptance remain untested.

Owner requirements confirmed 2026-10-09: explicit Quit must stop every owned
channel service, not detach and leave the engine/gateway running. The app stays
open in a visible Stopping state until termination is verified. Bounded graceful
shutdown may escalate only against verified owned processes; failure remains
visible and must not report success. Repeated Quit must not bypass shutdown.
Provide compact Settings service rows with real response-based state and
Start/Stop/Restart, expandable process/version details, and a warning before
interrupting an active show. Remote-enabled preference survives a normal Quit.
Test rapid relaunch, frozen/crashed services, stale records, and channel isolation.

Implementation checkpoint (not completion): dev-20 provides Settings controls,
process details, explicit unregister/verified engine termination, a Quit wait
state and shutdown error reporting. Two actual desktop Quit/reopen cycles left
no engine/gateway/Carabiner processes. SoundSwitch remained interactive and its
Link peer disappeared. Its hardware interface is currently disconnected, so
this is not the physical Control One reset-deadlock acceptance. Real-process
client suite: 26 tests passed. Additional TERM-ignoring child shutdown regression
passed. Startup cancellation and full managed-service fault cases still need
desktop acceptance.

Cross-Mac UI testing also exposed a concrete regression: LibrarySnapshotDecoder
rejected valid USB `empty` and `unloading` states, invalidating otherwise usable
snapshots. Decoder repaired with all seven mount-state cases passing. Repaired
desktop build now shows both tracks with 17-phrase plans, CHRM in Player 1 and
CHRM via Player 1/LINK as Player 2's loaded source while its own slot is empty.
The launchd-owned reader initially reported `No route to host`; after the owner
granted normal macOS consent it resolved CHRM without a code or permission reset.
Permission settings remain owner-managed, not agent-managed.

Live desktop checkpoint: Off → Arm → Start while paused, followed by simulator
Play on Player 1, showed SoundSwitch at 155 BPM with Intro Blue Red 2 active.
This is selection/connectivity evidence, not a measured first-beat latency pass.
The subsequent Settings Stop confirmation test was interrupted by macOS
ScreenCaptureKit error -3812; stop/start/restart UI acceptance is still open.
Normal and TERM-ignoring child shutdown/relaunch regression cases pass, including
an ownership recheck before forced termination. A Library query failure that
silently retained the old playlist's rows now has a preserving-navigation error
state and a passing regression; desktop error/recovery acceptance remains open.
The full Library package suite passed (70 Swift Testing tests and four XCTest
feedback tests), as did 15 media-resolver regressions and the macOS app build.
These automated passes do not replace the interrupted desktop acceptance.
Engine unit regression also passed: 145 passed, four explicitly ignored; the
canonical scenario differs only in its expected product version (dev-20).

Follow-up desktop checkpoint: resuming the existing Stop confirmation succeeded.
Settings showed Stopped and engine/gateway/Carabiner processes were absent.
Start and confirmed Restart both returned to Engine responding / Remote ready;
Restart replaced engine PID 34820 with 35145. A subsequent explicit Quit again
removed owned services. The transient “No verified Remote process” detail after
startup is repaired in dev-21 by refreshing details immediately after enablement.
Remote startup/recovery now carries a generation through suspension points so
an in-flight recovery cannot re-register after a concurrent stop. Shutdown reads
the previous-version record too; version mismatch still rejects normal commands.
Seven safety tests pass, including cancellation and old-version record validation.
These new race guards still require packaged desktop acceptance.

Library desktop checkpoint: expanding Sets → Trancendence 2 and selecting Mashup
ToDo eventually returned the expected seven tracks, but initially retained all
154 collection rows. Dev-21 adds an explicit pending-query state (including
Local Playback), clears stale rows while retaining navigation/editor data, skips
superseded queued queries and prevents an older monitor snapshot from replacing
the pending selection. Full Library suite: 71 Swift Testing + four XCTest passes.
The combined simulator soak now also alternates Library searches while changing
pitch, seeking and cycling lighting modes; query round-trip and maximum engine
command duration are recorded separately from MIDI-lane dispatch latency.

Measured E11-03 checkpoint on the Mac mini simulator and real SoundSwitch peer:
two 30-second combined runs each performed 14 pitch changes, four seeks, two
Pause/Start cycles, 28 Library searches and seven AutoLoop executions. Query
round trips peaked at 18.9 ms; source-age p95 was 20 ms; no MIDI failures or
queue saturation. The diagnostic repeat attributed the worst command (92.7 ms;
earlier unclassified run 259.9 ms) to Link enablement, not Library queries.
The relay now queues startup on its existing timing worker instead of waiting
for the helper. Starting/Degraded/Ready remain actual worker states; accepting
the request does not claim readiness. Engine unit suite still 145 passed / four
ignored; timing-output suite eight passed, including asynchronous startup failure.
Post-change 120-second run passed with 59 pitch changes, 17 seeks, ten operation
cycles, 115 Library queries and 27 AutoLoop executions: no Link/MIDI failures or
queue saturation. Library maximum round trip 21.0 ms; source-age p95/p99 20 ms,
maximum 51.8 ms; MIDI dispatch p95 52 µs, maximum 2.67 ms. Worst engine command
was now source-mode selection (88.5 ms), not Link enablement. Pump lateness still
peaked at 53.4 ms under aggressive full snapshots: E11-03 scheduling isolation
is therefore not closed. These are distinct segment measurements, not a claim
of end-to-end light latency or first-beat alignment. SoundSwitch UI remained
responsive and showed one Link peer. Physical hardware output is untested.

Reproduce the unavailable Dev attachment before repair. Verify service record,
process/build identity, authentication and command responsiveness separately.
Bound retries; show Starting/Recovering/Ready based on real responses. Keep the
existing explicit show/quit policy while separating UI reconnection from output
ownership. Test old helper/new app, stale record, rejected connection, failed
worker, sequential relaunch and clean helper cleanup. Separate channel databases
and configured mappings must survive. No automatic kill of unrelated apps.

## E11-03 — measured realtime boundaries

Dev-22 implementation (ADR 0048): full Library reads for browsing and snapshots
use an independent read-only transaction while the integration pump and Remote
commands continue. Real-database regressions cover captured query/editor state
and failure without recreating a missing database. The SQLite read-only lane now
registers the same pure version-family query function as the writer.

Matched 120-second simulator/SoundSwitch workload, before → after this change:
source-age p95 20 → 5 ms, p99 20 → 10 ms; pump starvation counter 2767 → 26.
After: 59 pitch changes, 17 seeks, 10 operation cycles, 114 Library queries;
no critical saturation or Link errors. Maximum query response grew from 21 to
36.5 ms (now off-owner). Worst pump lateness was 60.4 ms and source-mode startup
command 100.5 ms, so this does not close the scheduling/latency story. Evidence:
`build/Evidence/live-integration-projection-120s.json` (tested before the version
bump, embedded version dev-21). These are software-segment measurements, not
physical lighting latency. Packaged Dev-22 desktop acceptance remains required.

Instrument and reproduce before scheduling changes. Enforce ADR 0042's existing
requirement that heavy DB/plan/projection work cannot hold the show owner.
Compare a previously accepted build with current behavior using isolated data.
Preserve the tempo-only Link relay, sparse AutoLoop lane and existing visual
quality. Publish measured rather than hard-coded lateness. A source-to-MIDI
measurement and a SoundSwitch/light observation remain separate evidence.
Acceptance uses the proposed budgets and overload cases in the recovery plan.

## E11-04 — complete USB-to-editor workflow

Dev-21 implementation checkpoint: the UI and isolated USB worker now share
bookmark restoration. A stale bookmark resolving to the exact requested root
is renewed only while its existing security scope remains accessible. Failed
resolution no longer destroys the saved grant. Different or broader roots are
rejected; this never authorizes a different device or replaces physical marker
validation. Normal macOS selection remains the fallback when the grant cannot
be restored. Physical unplug/replug acceptance remains open.
The Library suite now includes four bookmark restoration-policy tests (75 Swift
Testing + four XCTest passes). Actual macOS stale-bookmark renewal has not been
forced in acceptance. USB-worker ownership is retained by the app; explicit
Stop/Quit cancels and waits for that worker too. Its cancellable process waiter
uses authoritative waitUntilExit completion, bounded TERM/KILL cleanup, and an
accurate operation-specific timeout message rather than always reporting 75 s.

Fix identity stability and durable authorization without merging independent
sticks. Scan, pre-sync impact, selection, progress, review and completion must
refer to the same source and revision. Keep stored selection and actual imported
state distinct. Validate new/changed/held tracks by complete playlist paths.
Use temporary DBs for destructive/fault tests; preserve user phrases/mappings.
Test nested leaf filtering, search, switching, editor/audio availability, phrase
edit/save/reopen and protected phrases after successful and rejected sync, with
the actual desktop app. Validate offline stored data as well as connected media.

## E11-05 — Arm preparation and phrase-start run-in

Dev-24 / Remote dev-6 candidate: versioned optional launch projection and guarded
compare-and-set commands are wired through both clients. The setting uses a
separate per-channel atomic JSON file with a bounded background writer; invalid
storage and save failures remain visible. Off/Arm only, Immediate default, explicit
no-upcoming-phrase state. Initial run-in pre-roll uses the existing scheduler for
negative, zero and positive offsets. Remote countdown reuses transport anchors;
it does not request a full waveform projection per beat. No Link phase commands.

Pre-install verification: 159 engine tests, 32 gateway tests and 18 Remote protocol
tests passed serially; 63 Mac Live tests, 36 Remote client tests, 14 Remote UI tests
and 30 exclusive real-process/MIDI client tests passed. Both app builds passed.
One earlier parallel run exceeded the snapshot performance budget while both apps
were compiling; the isolated rerun passed without changing the threshold. Packaged
UI/network launch acceptance remains required before closing this story.

2026-10-09 implementation checkpoint: ADR 0049 and a pure initial-launch gate
are implemented in source, with seven state-machine tests and a runtime test
which suppresses mid-phrase output and admits the next exact-grid boundary.
The installed dev-23 app still uses Immediate; no selectable policy or persisted
Mac/Remote controls are shipped yet. Negative-offset deduplication, MIDI failure,
late planning, UI projection and representative network acceptance remain open.
Do not present this groundwork as completed user functionality.

Arm preloads current/next phrase plans and prepares commands; preselect a bank
only if its documented/tested behavior cannot disturb existing output.
Immediate remains the default. Optional On phrase start selects the first
upcoming phrase boundary when playback begins after arming. A cue several beats
before that boundary supplies run-in. Start exactly on the boundary triggers
immediately; absence of a usable future boundary is explicit and never silently
waits indefinitely. Show the target phrase and beats remaining on Mac and Remote.

Schedule using the authoritative beatgrid, pitch and signed output offset.
Recalculate an unsent deadline on BPM changes, cancel stale targets on load,
seek/master changes and deduplicate execution. An already missed deadline is an
explicit late-start outcome; do not defer to a different phrase without intent.
This setting controls the initial launch, not a repeated delay at every phrase.

Investigate optional one-shot SoundSwitch alignment only after timing is fixed.
Maximum one action during a maximum four-beat launch window, then no corrective
actions until a new genuine trigger. No reliable phase feedback means no claim
of conditional automatic correction. Keep global Link resets and continuous
phase chasing excluded. A disruptive/unsupported proof of concept is omitted
from the release rather than silently enabled.

## E11-06 — integrated acceptance and release

Run automated component, real-process, fault and sustained-load suites, then
actual desktop Mac, iOS Simulator and simulator-control UI scenarios. Use
90s Bitch on Player 1 and My Favourite Regrets on Player 2 where available;
also vary tracks with playlist Auto Mix. Use read-only observations of SoundSwitch
and controlled MIDI tests to verify visible selection/progress. No UI screenshot
or internal latency percentile is proof of physical light timing.

Publish a coverage ledger with each scenario marked reproduced, repaired,
automated-pass, UI-pass, hardware-evidence or still-open. Simulator tests can
replace repetitive setup; real CDJ behavior is anchored to existing captures
and a short final physical acceptance, not claimed for every firmware/model.
Retain actual USB permissions/filesystem testing and downstream light acceptance
as distinct gates that synthetic fixtures cannot prove.

Version and push reviewed increments to dev. Package Lumi, Remote and Simulator
independently, document tested combinations, update user docs and HQ screenshots
where UI changed, and prepare main only after release blockers pass. Do not bump
production or publish an accepted release merely because a build succeeded.

## Owner involvement

Current control URL has been supplied and verified; never store its token here.
Keep Mac mini simulator available. At the package gate the owner may need to
install one new simulator DMG, since the existing HTTP controls cannot install
software. SoundSwitch running and an unlocked Mac permit native acceptance.
Use controlled fixture media locally while real USBs are unavailable; ask for
one bounded physical USB check and final light acceptance only when necessary.
No repeated CDJ play/eject/reload requests during routine regression development.
