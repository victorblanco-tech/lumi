# Live-show reliability recovery proposal

Status: implementation and autonomous testing authorized on 2026-10-08; not implemented by this document. Hardware acceptance failed. Dev-19 is not show-ready. No new release until the gates below pass. [Epic 11](epic-11-show-readiness-recovery.md) consolidates the findings, simulator-first execution order and the subsequently requested phrase-start run-in behavior.

## Evidence and limits

- User observes approximately three seconds of delay in Live lighting and delayed BPM display. First playback from standstill starts the AutoLoop late and leaves it off-beat. End-to-end delay has not been instrumentally measured.
- Native UI reproduced unavailable service attachment, an editor stuck loading and a playlist selection leaving the collection visible. A cold Dev restart restored a library response, but does not establish general recovery or timing correctness.
- A known independent USB can acquire a repeated physical-identity conflict; sync then imports nothing. Authorization is unnecessarily requested again. Preserve existing data and do not bypass copied-marker safeguards.
- Source inspection: the integration pump, desktop command handling and remote commands execute against the same mutable runtime in session.rs. Synchronous command application and snapshot/library payload construction can occupy that owner. Logical lane separation therefore does not prove scheduling isolation. The biased select can also starve lower-priority commands if pump work overruns its interval. These are risks to measure, not established causes of today's latency.
- Desktop Live snapshot polling is nominally 250 ms plus exchange/decode time. This may explain some display delay but not, by itself, three seconds of lighting delay.
- MIDI scheduling already uses its own lane; its internal dispatch statistics do not prove prompt upstream decisions or SoundSwitch playback alignment. The snapshot AutoLoop lateCount is currently a literal zero, not a measured lateness counter. Never present it as evidence that no late triggers occurred.
- Simultaneous production services observed later are not evidence of simultaneous production use during the failed hardware test; exclude that assumption from the root-cause conclusion.

## Phase 1: capture, baseline and service ownership

Collect bounded local diagnostics across source receipt, ingress queue residence, runtime pump start/end, plan readiness, intended cue deadline, MIDI enqueue/emission and UI projection receipt. Use monotonic clocks; validate clock domains before comparing bridge and engine timestamps. Correlate loaded-track generation, master generation and trigger identity. Measure SoundSwitch acceptance and visible lighting separately; do not substitute MIDI send completion for downstream acknowledgement. Do not publish raw credentials, media identifiers or personal diagnostics.

Reproduce against the current build and a previously accepted build using a database copy and identical mappings, tracks and network. Isolate BPM relay, manual-equivalent MIDI trigger and combined operation. Record what changed since the accepted build. A rollback is only a release candidate after compatibility and hardware validation, not an assumption of safety.

Make channel-specific service readiness and ownership deterministic: bounded attachment/authentication, stale record validation, executable/build verification, actionable failure, controlled reconnect and clean stop. A Ready UI must not conceal unusable engine/library state. Preserve the autonomous show during a desktop-only reconnect; explicit Stop/Quit policy must remain clear. Do not indiscriminately kill unrelated apps or conflate Dev and production databases.

## Phase 2: enforce timing isolation

Keep the realtime owner limited to bounded ingress reduction, transport/beat decisions and prepared cue dispatch. Move database reads, plan compilation, waveform expansion, snapshot serialization and filesystem/network enrichment off that owner. Publish immutable versioned results through bounded messages. Workers do not mutate show state; the owner accepts results only for the current load/master/library generation. Preload the current and next deck while stopped or armed. A verified existing track keeps its provenance and plan through USB timeout/eject; the inserted-media identity and loaded-track source remain separate.

The tempo relay consumes the newest authoritative master clock independently of planning, operation mode and UI. Preserve continuous BPM changes without seeking or repeatedly reanchoring Link. Coalesce superseded ordinary observations while preserving ordered start/stop/load/master/seek transitions. Handle overflow explicitly; never replay a seconds-old backlog to the lights. If a cue is genuinely late, apply a documented current-state policy rather than playing obsolete cues in sequence.

Retain existing waveform colors and fixed playhead. Project compact transport anchors without repeatedly transferring full waveform/library payloads. UI smoothing must not become an output clock or extrapolate through an actual seek.

## Phase 3: deterministic AutoLoop launch and optional start alignment

Prepare the bank and current phrase command before playback where possible. Transport Start in Start mode dispatches the current phrase once; enabling Start on an already-playing master uses the current authoritative phrase. Planned phrase boundaries use a calibrated signed offset; an unpredictable first Play press cannot be anticipated with a negative offset. Bank settling, MIDI transport latency and SoundSwitch launch semantics need separate measurement.

Research and hardware-test SoundSwitch Cue and beat/phase controls. Official documentation lists these controls but does not establish a machine-readable current AutoLoop phase or a safe one-shot correction contract. Do not infer phase feedback from a MIDI send or BPM equality. Repeated loop button presses must also be tested for toggle/retrigger behavior before any retry strategy.

Proposed optional SoundSwitch-profile policy: Off (default), then Start alignment only after validation. A launch creates an identity-bound alignment window of at most four beats, permits at most one alignment action and then closes permanently until the next genuine launch. Prefer aligning as part of launch, not a visible mid-loop jump. Keep tempo ownership in the Link relay; do not send competing BPM/timeline writers, MIDI Clock bursts or global Link resets. A separate SoundSwitch control may still alter a shared phase, so its effects on all active output and peers must be tested.

Automatic correction conditional on measured error requires reliable downstream phase feedback. If none exists, label any supported action honestly as one-shot launch alignment, not automatic detection/correction. Do not use screen scraping as a production feedback loop. A manual one-shot Align control is an alternative to propose to the user, not silently enable. Cancel pending alignment on a new load/master/seek, Off/Pause, disconnect or expired window. Do not repeatedly retrigger an already emitted loop. A new seek to another phrase is a new musical decision, not an excuse for ongoing correction.

## Phase 4: usable USB/library preparation workflow

Persist valid security-scoped USB access; investigate unstable physical fingerprints versus missing serial information without conflating independent same-model sticks. Provide explicit safe re-confirmation for genuine conflicts. Show a source-scoped stored sync selection and current USB tree, exact folder hierarchy and pre-sync new/changed/held/review tracks with playlist paths. Same versus changed versus proven freshness must remain distinct. Preserve protected Lumi phrases and existing aliases when keeping the local revision.

Library queries must atomically bind selected playlist/workflow/search to displayed rows, clear or identify stale results and expose failures/retry. Editor requests must finish or fail visibly, cancel obsolete selections and retry correctly. Existing local editing must not depend on a successful fresh USB scan. Test exact playlist membership and edited-track save/reopen, not only whether a tree renders.

## Phase 5: release gates

Proposed targets, to validate and agree before implementation: internal ingress-to-MIDI dispatch P95 <= 20 ms, P99 <= 50 ms under simultaneous UI/library stress; no unexplained multi-beat delay, no stale cue replay or missing/duplicate trigger. Define and report the downstream SoundSwitch/fixture timing budget separately using measurement resolution. Initial alignment target: <= 50 ms beat error, with no subsequent unsolicited jumps. Wi-Fi and wired runs are measured separately, not treated as interchangeable latency guarantees.

Test at least 30 stop/hotcue starts, phrase transitions, same-loop phrase choices, pause/resume, mode switches, master changes, pitch-slider changes and known seeks; startup/relaunch, UI backgrounding, editor/playlist activity, USB reconnect/eject and sync rejection. Include Mac and Remote controls. Compare exact source/master/track and plan identity, Link tempo, MIDI messages and observed SoundSwitch loop progress. Automated regression includes delayed workers, SQLite contention, slow clients, reordered/stale observations and bounded-queue overload.

Run a minimum two-hour real-hardware soak with lights/SoundSwitch and a longer simulator soak after the simulator models validated hardware behavior. Zero crashes, missing plans for known prepared tracks, hidden command failures or unsolicited loop phase corrections. Report untested scenarios and measured distributions. Publish main only after both preparation workflow and Live gates pass; a good Live test cannot waive an unusable editor/sync workflow.

## Sources

- [SoundSwitch standalone MIDI controls](https://support.soundswitch.com/en/support/solutions/articles/69000847411-soundswitch-using-soundswitch-in-standalone-mode): Cue, beat/phase shifting and AutoLoop launch controls; exact application behavior still requires validation.
- [SoundSwitch MIDI Sync](https://support.soundswitch.com/en/support/solutions/articles/69000847415-soundswitch-connecting-soundswitch-with-midi-sync-in-and-midi-output): MIDI Clock/MTC are timing inputs for AutoLoops, not evidence that mixing them with the existing Link relay is safe or necessary.

Scope of this change: proposal documentation only. No app, database, integration or SoundSwitch configuration modified.
