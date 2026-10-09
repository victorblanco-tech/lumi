# Epic 11 — reliable preparation and Live performance

Status: implementation and autonomous testing authorized on 2026-10-08.
Baseline: public Lumi v0.6.3; current development build 0.6.4-dev-19.
Products: Lumi, Lumi Remote and the independently versioned Pro DJ Link Simulator.
Execution order: simulator fidelity first, then recovery and measured timing, preparation workflow, launch policy, integrated acceptance.

The earlier [recovery proposal](live-show-reliability-recovery-2026-10-08.md)
defines the timing investigation. This epic is the consolidated acceptance ledger.
Existing story completion and previous component measurements do not close a
newly reproduced regression. Every row needs a failing reproduction, repair,
automated regression and actual application acceptance where applicable.

## Findings and ownership

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

Reproduce the unavailable Dev attachment before repair. Verify service record,
process/build identity, authentication and command responsiveness separately.
Bound retries; show Starting/Recovering/Ready based on real responses. Keep the
existing explicit show/quit policy while separating UI reconnection from output
ownership. Test old helper/new app, stale record, rejected connection, failed
worker, sequential relaunch and clean helper cleanup. Separate channel databases
and configured mappings must survive. No automatic kill of unrelated apps.

## E11-03 — measured realtime boundaries

Instrument and reproduce before scheduling changes. Enforce ADR 0042's existing
requirement that heavy DB/plan/projection work cannot hold the show owner.
Compare a previously accepted build with current behavior using isolated data.
Preserve the tempo-only Link relay, sparse AutoLoop lane and existing visual
quality. Publish measured rather than hard-coded lateness. A source-to-MIDI
measurement and a SoundSwitch/light observation remain separate evidence.
Acceptance uses the proposed budgets and overload cases in the recovery plan.

## E11-04 — complete USB-to-editor workflow

Fix identity stability and durable authorization without merging independent
sticks. Scan, pre-sync impact, selection, progress, review and completion must
refer to the same source and revision. Keep stored selection and actual imported
state distinct. Validate new/changed/held tracks by complete playlist paths.
Use temporary DBs for destructive/fault tests; preserve user phrases/mappings.
Test nested leaf filtering, search, switching, editor/audio availability, phrase
edit/save/reopen and protected phrases after successful and rejected sync, with
the actual desktop app. Validate offline stored data as well as connected media.

## E11-05 — Arm preparation and phrase-start run-in

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
