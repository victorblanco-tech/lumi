# E10-09 Automatic USB identification on live Players

Status: Prepared on 2026-10-04. No NFS POC or production implementation completed.
Depends on E10-08 and ADR 0045.

Lumi must automatically associate a live track with the correct trusted USB,
including two independent equal-model sticks and tracks loaded over Link. The
owner does not want manual USB-to-Player assignments. Media access must not delay
the accepted BPM relay, transport processing or one-shot AutoLoop output.

## Phase 1 Physical file access proof

Build a standalone, read-only probe using the pinned Deep Symmetry components.
It receives a discovered Player address and USB slot and tries only the existing
`.lumi-media.json`. It must not start a second virtual Player, query waveforms,
download music, write to media or change CDJ transport. Record mount negotiation,
file outcome, exact-byte comparison, elapsed time and bounded failure outcomes.

Prepare validation and supervisor tests before network testing: valid and invalid
markers, missing file, oversized response, unreachable server, truncated data,
deadline expiry and child-process cleanup. No admin rights or system NFS server
configuration should be required. FileFetcher needs a verified download/deadline
bound, not only an after-download size check.

Physical gate: retrieve the same marker read locally from GRAY in Player 1 and
CHRM in Player 2; repeat after swapping the sticks. Do not call this supported
until actual CDJ-1500X tests pass. If either export path or firmware rejects the
file, record that fact and discuss the next identity strategy before expanding.

## Phase 2 Isolated resolution service

Implement bounded asynchronous jobs, single-flight coalescing per source slot,
device/media generations, capped retries with backoff and cancellation. Keep all
file I/O, parsing and SQLite preparation outside the realtime pump. Resolve only
known trusted markers. Expose source name, resolution state, last verified time
and actionable failure in existing Pro DJ Link diagnostics.

Use fixture transports for automated fault tests. The current simulator provides
status, tempo and position but no NFS/media server. A fixture or HTTP simulator
response is not evidence that a real NFS request works. Add only the simulator
support necessary to exercise media swaps and cross-Player sources; do not build
a general CDJ emulator or install a system NFS daemon for this story.

## Phase 3 Source scoped library hydration

Pass source Player, slot, verified source ID, media generation and track-load ID
through the lookup. Tests must prove that the same numeric ID on two USBs cannot
select the wrong canonical track. Player 2 loading from Player 1 must resolve the
USB in Player 1. Reject stale results after a swap, reconnect or new track load.

Integrate E10-08 fixes: missing phrase preparation is non-fatal; a committed sync
causes a fresh lookup for an unresolved loaded track; audio versions keep their
historical fingerprints and cannot be redefined by replaced paths. Do not force
an unload/reload or restart a healthy bridge to refresh library data.

For already active prepared tracks, define and test safe adoption of changed
analysis/phrases. Preserve the playing plan until a safe explicit transition;
never silently change active beat coordinates or replay the current cue. Show a
pending-library-update status when adoption is deferred.

## Phase 4 Acceptance and timing evidence

- Local regression: two sources with colliding IDs; identical audio on both;
  different editions; missing phrases; commit after Player load; retained edited
  timelines; replaced-path detection; stale resolver completions.
- Process faults: slow/unreachable NFS, server loss, malformed/large marker,
  copied marker, media swap without a captured empty status, shutdown during a
  request. No unbounded retry, leak or blocking of the show pump.
- Native UI: source name and resolution status are correct on macOS and iPhone;
  a missing/unprepared track affects only that Player; app switching and sync do
  not clear the other deck or shift the layout.
- Physical: load both locally and across Link; start from cue, pause/resume,
  hotcue/beatjump, master handover, eject/reinsert after playback stops, restart
  Lumi while paused and playing. Never eject a playing source USB.
- Soak: representative two-Player simulator run with resolver faults, then a
  physical run across multiple tracks. Record latency distributions, queue age,
  starvation, cue count/order and recovery outcomes, comparing the same setup
  before/after. Physical light-output observation is separate from dispatch
  latency and simulator acceptance.

Success requires all known reproduced failures to have durable regression tests,
no source-blind automatic match, no resolver-triggered bridge/Link restart, no
extra AutoLoop trigger from a library refresh, and no measurable regression
outside the baseline timing budget. Agree numeric limits from measured baseline
before claiming the performance gate passes.

## Owner setup for autonomous testing

1. Connect GRAY and CHRM to the MacBook so each marker and source registration can
   be verified. Preserve all music, Rekordbox files, phrases and MIDI mappings.
2. Move one stick to each CDJ-1500X, keep the Players and DJM on the same LAN as
   the MacBook and leave one locally known track loaded on each, initially paused.
3. Keep the desktop unlocked and approve any requested local-network permission.
   Confirm that test builds may run and that no performance/show is in progress.
4. For output regression, leave SoundSwitch open with Ableton Link enabled. Lumi
   Off/Arm is sufficient for the first file POC; Start and actual light output
   require a separately confirmed safe test window.
5. Provide the current simulator control URL/token when it is running. Expired
   historical URLs must not be reused or committed to the repository.

Codex can prepare builds, run local/process tests, inspect discovery and control
the simulator. Physical insertion, swapping/ejecting USBs and CDJ Play/hotcue/pitch
actions still require the owner unless a documented hardware control interface
is explicitly approved. Physical compatibility cannot be declared solely from
headless or headed simulation. Stop and report permission failures rather than
working around them.

## Current evidence and limits

- Source audit and test plan prepared; no app-code/runtime changes in this task.
- The current simulator deliberately does not serve files over NFS.
- Pinned Crate Digger 0.2.1 FileFetcher is available in the local dependency
  cache; a local Java 21 packaging toolchain is available. Hardware reachability
  and arbitrary marker access have not been tested.
- At preparation time no removable USB was mounted on the MacBook. Hardware
  setup, physical POC, headed acceptance and timing evidence remain pending.
