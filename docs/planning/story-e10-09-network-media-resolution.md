# E10-09 Automatic USB identification on live Players

Status: In progress on 2026-10-05. Phase 1 physical marker retrieval passed for
both independent USBs and after swapping Players. Phase 2 is implemented in
0.6.4-dev-1 with local regressions. Basic native identity acceptance passed after
local enrollment: GRAY and CHRM are simultaneously identified on their respective
real Players without restarting Lumi. Source-scoped live hydration is implemented
in 0.6.4-dev-4 with local regression coverage. Native cross-Player matching and
the separate track-origin display passed on 2026-10-06. Updated-track sync,
physical media replacement and end-to-end DMX acceptance remain open.
Depends on E10-08 and ADR 0045.

## Loaded track origin in Mac Live and Remote

Lumi 0.6.4-dev-7 and Remote 0.1.3-dev-4 distinguish the USB mounted in a Player
from the USB supplying its loaded track. The mounted USB remains in the Player
identity card. A separate compact Source row beside the track shows the verified
name and native media color, plus local or via Player N / LINK. Unidentified
origins remain explicit and never borrow the destination USB's name.

Regression coverage includes linked loads, local loads, unresolved identities,
source-only projection changes, Remote transport-anchor preservation, legacy
snapshots without the optional field, and unchanged output record counts during
origin projection. This is a display addition; the accepted media matching,
waveform renderer and output timing paths remain unchanged.

On 2026-10-06, real CDJ-1500X Players identified CHRM in Player 1 and GRAY in
Player 2 simultaneously. Loading 90s Bitch on Player 2 over LINK from Player 1
produced its prepared 17-phrase plan without a bridge restart. The explicit origin
row and performance check are validated separately below once the new build is
installed. Fresh local sync and physical media replacement remain acceptance
steps, not implied by successful linked loading.

The installed dev-5 check exposed an unavailable isolated reader after the
service upgrade, despite a standalone fixed-file read succeeding in 78 ms.
Dev-6 retains a bounded, control-character-free diagnostic for recognized
reader failures and the child exit status. A non-zero child is never treated
as a successful marker read. Failure detail stays on the Mac; Remote receives
only resolution state. Physical acceptance of the new build remains required.

Dev-7 preserves the underlying failure of the same read-only RPC GETPORT query
instead of the dependency's generic portmap error. Java regression coverage
confirms GETPORT-only behavior and retention of the original exception. The
installed background service reports `No route to host` for both real Players;
the identical packaged reader started directly read the Player 1 marker in
65 ms and the Player 2 marker in 56 ms. After the owner changed macOS
permissions, the same running Dev-7 service recovered without a restart or
database changes. Its readers identified CHRM in 60 ms and GRAY in 206 ms.
No permission was changed or bypassed by the automated test.

Before that blocker, the complete Dev-6 portable Rust and safe Apple gates
passed, including source projection, Remote anchors and native builds. With
compiler load removed, 250 snapshot samples measured full snapshot p95 9.112 ms
and live snapshot p95 2.477 ms. These are component measurements, not a claim
about MIDI-to-light timing or successful playback with the new resolver.

Native acceptance after permission recovery:

- Mac Live shows CHRM physically in Player 1, with track source CHRM local.
  Player 2 shows GRAY physically mounted but track source CHRM via Player 1 /
  LINK. Both players have the prepared 17-phrase 90s Bitch plan.
- Arm, Start, Pause and resumed Start preserve those identities. Navigating
  through Integrations and returning to Live retains playback, prepared plans
  and the correct origin. Pro DJ Link recorded zero automatic restarts.
- Remote 0.1.3-dev-4 in the iPhone 17 Pro simulator remains connected in View
  only without changing the physical iPhone's Controller assignment. Portrait
  and landscape both show the full linked-source name. Landscape gives the
  origin its own fixed-height row; rotation retains the fixed live playhead.
- During Start with the real Player 1 loop, Pro DJ Link ingress measured p95
  10.0 ms, zero critical saturation and an empty queue. AutoLoop output measured
  p95 4.7 ms, zero late dispatches and zero saturation. SoundSwitch displayed
  155 BPM with one Link peer and received AutoLoop selections. Its hardware
  interface was disconnected, so this is not measured DMX/light latency.
- Neither Player supplied native USB color information in this session.
  Names remain correct and icons stay neutral; no guessed Pink/Blue values
  are stored. Native color after a fresh media broadcast remains an acceptance
  step. The shared color mapping and source projection have regression tests.

The waveform renderer and integration scheduling code were not changed for
the origin display. Safe Apple regression tests and both native app builds
passed for the final Remote layout.

The final Dev-7 portable Rust gate also passed, including strict Clippy,
workspace regressions, process tests and release-mode planner/library budgets
(`build/track-source-dev7-rust-recheck.log`, local ignored evidence). An earlier
run failed local fake-Carabiner connection tests; an isolated rerun and the
complete repeat passed without code changes or relaxed assertions.

### Extended hardware test is not a show-readiness sign-off

The longer session exposed intermittent RPC timeouts during marker rechecks
and a recurring Link stale-clock hold. A bounded 24-snapshot read-only capture
showed both USB identities recovering with unchanged IDs and generations,
but temporary unavailable states remained. Exact-position readiness also
briefly dropped. The origin display correctly reflected those states; this
does not establish stable continuous show output.

Independent 20-packet reachability checks observed Player 1 at 0% loss with
3–151 ms round trips, and Player 2 at 30% loss with replies up to 2474 ms.
These checks demonstrate network instability during this session, not its
cause or proof that every timing issue is network-related. No network settings
were changed. The existing identical-prepared-context binding path was also
identified for follow-up regression review, not changed speculatively here.
Further continuous Start/Link and physical DMX acceptance is still required
before declaring this build ready for a show.

### Retain established bindings during failed background rechecks

The owner clarified on 2026-10-06 that a timeout, including repeated timeouts,
must never revoke an already verified playing track's USB binding or cause a
show error. Dev-8 retains the verified identity, color, name and epoch on failed
reads, records the delay as resolver detail and retries with bounded backoff.
It does not fabricate a fresh verification time. Newly loaded tracks still
wait for their own successful verification. Explicit unknown/conflicting media,
device loss and a confirmed different marker keep their invalidation behavior.
Cross-source matching fallback is deferred.

Thirteen resolver tests passed, including eight consecutive failed rechecks,
same-medium recovery, withheld authorization of new loads, and explicit media
change/conflict after a timeout. The complete portable Rust gate passed,
including strict Clippy, process/network regressions and release performance
budgets. The 250-sample release snapshot benchmark measured full projection
p95 at 1.804 ms and live projection p95 at 0.364 ms.

The installed Dev-8 app was tested through the native UI with the real Players
and DJM. Arm and Start succeeded; both 17-phrase plans were present. Player 1
displayed mounted CHRM and a local CHRM track, while Player 2 displayed mounted
GRAY and its CHRM track loaded through Player 1 / LINK. SoundSwitch showed
155 BPM, one Link peer and the selected AutoLoop. During the observed Start
interval, diagnostics recorded seven MIDI pulses, output p95 of 5 ms, no late
output, no queue saturation and no timing safety holds. These are software
dispatch measurements, not physical light latency: SoundSwitch's hardware
interface was disconnected.

A separate 46-second read-only background capture contained 24 samples. Both
USB identities and their epochs stayed trusted, both track preparations stayed
ready, and each Player completed four distinct successful verifications. No
read timeout occurred in that bounded capture; repeated failure preservation
is covered by the injected regressions rather than claimed as an observed
hardware failure. One sample lacked exact position authority before recovery.
The earlier network and Link observations therefore remain separate follow-up
items, not declared fixed by this narrowly scoped change.

The first local package launch encountered a macOS launch-constraint rejection;
the service subsequently started and reopening Lumi attached successfully.
Later reopening retained both verified plans. The ad-hoc-signed Dev update is
not evidence of a fully reliable first-launch installer. The Dev database was
backed up and was not reset; production data and USB files were unchanged.

## Event driven mounts and cached track origin

The owner requested this refinement on 2026-10-06. Lumi 0.6.4-dev-9 and Remote
0.1.3-dev-5 separate a Player's current USB mount from each loaded track's
verified source. Mount changes come from the native CDJ USB status. After a
successful identity read, stable status and track changes on the same mount
perform no further USB reads. Initial failures retry independently; insertion,
discovery and recovery after a status interruption request verification.

Eject or replacement clears only the current mount's authorization. Existing
verified tracks keep their origin, exact local beatgrid, Lumi phrases and plan,
including cached tracks loaded over LINK. A different new load uses the newly
verified mount. A load awaiting its first verification cannot adopt a USB
inserted after that load. No fallback matching, waveform change or tempo-relay
redesign is part of this refinement.

Acceptance requires regressions for no repeated reads, ordered mount events,
cancelled old reads, local and linked cached tracks surviving removal, and new
loads using the new source. Native Mac/Remote empty-mount presentation and real
CDJ status behavior remain required before completion.

### System crash during development

At approximately 22:14 on 2026-10-06, macOS panicked after repeated crashes of
AppleBCMWLAN. Separate driver reports earlier that evening identify the WLAN
deadlock watchdog; they do not attribute the trigger to a specific app. Dev-9
was not installed or running. This does not exclude existing Lumi network
traffic as a trigger and does not establish a fix for the operating-system
failure. Both databases passed SQLite quick-check after the reboot.

Production and Dev engine/gateway services restarted at login under their
existing RunAtLoad/KeepAlive configuration. Both were explicitly stopped with
owner approval before further local checks; no Java or Link helper remained.
Subsequent gates must not start the live network implicitly. Physical timing
and first-launch acceptance are separate from local compilation and regression
results.

### Local validation of the event driven refactor

Dev-9 passed the complete portable Rust gate with tests executed serially,
strict Clippy, Java bridge verification (20 tests), the full Apple gate,
25 native engine-client tests, both native app builds and documentation checks.
The new regressions cover stable mounts with 10,000 ticks and only one read,
ordered eject/insertion events, cancelled replies, local and LINK cached tracks
retaining their source and plan, and an unverified load refusing a later mount.
The native client gate no longer supplies the live bridge to standalone process
tests; joining the real DJ network remains an explicit acceptance step.

One parallel debug-suite run exceeded the existing 25 ms snapshot budget
(p95 57.922 ms). The complete serial repeat passed without changing that budget.
An isolated 250-sample release run measured full snapshot p95 1.441 ms and
Live projection p95 0.292 ms. These measurements do not establish physical
MIDI or DMX latency. The pre-install Dev library backup passed SQLite
quick-check. No library reset, USB write or production database change occurred.

### Native validation of Dev 9

The signed installer passed the fixed-path, payload and signature audit on
its second build attempt. The first stopped during the quiet DMG stage without
a diagnostic; no packaging assertion was bypassed. Build 368, source
`addbd85e2a1f`, was installed under `/Applications/Lumi/Dev` with the existing
Dev database. Only that channel's engine, bridge and gateway were running.

The real CDJ-1500X Players and DJM-V5 were discovered. Both 17-phrase plans
loaded in the Mac UI: Player 1 mounted CHRM and loaded locally; Player 2 mounted
GRAY but retained its CHRM track source via Player 1 / LINK. Arm, Start, Pause,
resumed Start and navigation through Integrations kept those plans and sources.
During the observed Start interval, output p95 was 5.1 ms with no late dispatch,
saturation, fail-closed hold or provider failure. Pro DJ Link ingress p95 was
10.0 ms with zero critical saturation and zero automatic restarts. These are
internal measurements; SoundSwitch's hardware interface was disconnected.

SoundSwitch showed 155 BPM, one Link peer and bank 3 selection after Lumi
started. Its MIDI mapping view remained usable without editing mappings.
Normal Lumi quit stopped Carabiner and removed the peer from SoundSwitch.
The stable CoreMIDI engine and Pro DJ Link bridge remained under the existing
service policy. Reopening Lumi retained the same loaded plans and sources;
Arm and Start succeeded again.

A separate 24-snapshot read-only capture over 46 seconds with the desktop closed
showed unchanged verified USB timestamps and generations (CHRM 2, GRAY 1),
both prepared load IDs unchanged, exact position READY in every sample and
bridge traffic advancing from 25,737 to 29,907 events with zero restarts.
The first attempt while the Mac UI was attached timed out because the desktop
endpoint serves one UI at a time; it is not a parallel monitor endpoint.

Remote 0.1.3-dev-5 was installed in the previously paired iPhone 17 Pro
simulator. Portrait and landscape retained the mounted USB and distinct LINK
track-source labels, fixed live playhead and live Start state. Its View only
role and the physical iPhone's Controller assignment were not changed. Neither
Player supplied a native media color in this session; icons remain neutral.

Physical eject/replacement while a cached track continues, refreshed-track
acceptance and actual DMX latency remain open. Local regressions do not
substitute for those tests. No second kernel panic was observed during this
bounded session, but Dev-9 is not established as a Wi-Fi-driver fix or a full
show-readiness sign-off.

### Physical eject exposed a linked track decoder defect

The owner safely ejected CHRM from Player 1 without changing Player 2.
Dev-9 correctly displayed Player 1's empty mount while its local cached track
kept CHRM as source, its 17-phrase plan and live playback. Player 2 instead
changed from CHRM / LINK to an inferred GRAY local source and lost preparation.
Returning CHRM restored the mounted identity but not Player 2's track origin.
The linked cached-track physical acceptance therefore failed in Dev-9.

The CDJ-1500X extended-ID decoder has a local-Player inference when Beat Link's
legacy identity is absent. Dev-10 retains the last confirmed loaded origin
when the same extended track ID remains and native identity is absent. Native
identity overrides that cache, including an explicit source change with a
colliding numeric ID. A changed ID, true unload, device loss or rediscovery
clears or replaces the old identity. This does not add source-agnostic matching
or network probes. The bridge suite passed 21 tests with a regression for these
cases; physical eject acceptance must be repeated with Dev-10.

The complete portable Rust and safe Apple gates, native builds and signed
installer audit passed for Dev-10 (build 370, source `c35d48c9832b`). Its
installed bridge JAR hash matched the tested artifact. Both Players again
had the correct CHRM source and 17-phrase plans before the second eject.
The physical repetition still failed: Player 1 retained its cached local plan,
but Player 2 again changed to inferred GRAY/local and lost preparation.
The owner changed only CHRM's mount. The narrow decoder regression is therefore
insufficient to cover the actual status transition; Dev-10 is not accepted for
cached LINK playback after eject.

The owner authorized and ran a passive capture restricted to eight Player 2
UDP status packets on destination port 50002. The post-eject capture contains
1152-byte UDP payloads, with their first 726 bytes retained by the 768-byte
frame capture limit. All eight captured status headers report Player 2 as the
track source, USB slot 3, Rekordbox track type 1 and ID 1031. The extended ID
at `0x194` also remains 1031; play state is cued and beat number is 64.
The field offsets agree with Beat Link 8.0's `CdjStatus` implementation and
[Deep Symmetry's status analysis](https://djl-analysis.deepsymmetry.org/djl-analysis/vcdj.html).

This disproves the narrower assumption that only a missing legacy identity
caused the failed acceptance: these post-eject packets contain a nonzero
legacy identity, which Dev-10 accepts as a source change. The capture covers
the stable state after removal, not the transition or the preceding LINK
state. A before/after comparison is required before interpreting additional
extended fields as a durable load identity. Retaining an origin solely because
the numeric track ID is unchanged could misidentify a real load from another
USB with colliding IDs. The raw capture remains local under ignored `build`;
no network addresses or packet dump are published with this result.

The second owner-run eight-packet capture, after restoring CHRM and reloading
Player 2 over LINK, confirms source Player 1, USB slot 3, track ID 1031 and
beat 64 in the same 1152-byte layout. Compared with the post-eject capture,
the bounded extended track block `0x170..0x1af` is byte-identical. Source at
`0x28` changes from one to two, `0xbb` from two to zero and `0x125` from zero
to four. Packet counter differences are not treated as track changes. These
are two stable-state measurements, not a capture of all intermediate packets.
The owner explicitly unloaded Player 2 before that fresh LINK load. The equal
extended block therefore also occurs across reloads of the same content; it
cannot be interpreted as a unique load-instance token.

Dev-11 implements the exact-layout cached ownership exception in ADR 0045.
It retains only an already observed LINK load, never matches a new track across
USBs. Tests exercise the captured fields through Beat Link's native decoder
and bridge publication, 10,000 repeated cached statuses, loading before coherent
tempo, unload/loss, changed ID/block, another source and unsupported firmware
or layouts. The unknown extended block is not claimed to be a durable unique
identity. Physical repeated eject and fresh-load acceptance remain pending.

## Autonomous hardening evidence from 2026-10-05

Phase 3 now prepares a source-scoped track on a read-only library worker rather
than querying SQLite in the realtime pump. Requests carry the track load, source
Player, verified USB identity and media generation. Tests reject stale replies
after a new load or media swap and retain the physical source when a different
Player loads over Link. An unresolved track is rechecked after a committed sync;
an active prepared track keeps its current analysis and plan until an explicit
reload. A deferred update is shown in the existing Mac USB status line.

Local evidence before physical acceptance:

- Full portable Rust validation passed for dev-3: strict Clippy, workspace tests,
  canonical transcript, process checks and release-mode planner/library budgets.
  The combined run reported 458 passing tests and 14 intentionally ignored tests;
  ignored hardware tests are not hardware acceptance.
- Full Apple validation passed with exclusive Dev MIDI ownership: 7 protocol,
  25 engine-client, 17 design-system, 58 Live, 63 Library, 35 Remote-client and
  14 Remote-feature tests, plus native Mac/iOS builds and signing checks.
- With a deliberately blocked library worker, 10,000 release-mode pump polls
  measured p95 250 ns, p99 292 ns and maximum 128,125 ns on this Mac. A separate
  80 ms injected reader delay did not block polling. These are component timings,
  not end-to-end MIDI or observed lighting latency.
- Schema 20 migration retained all 114 tracks, 264 timeline revisions and both
  media bindings. Every row in the 43 existing tables matched the pre-migration
  backup in both directions; `quick_check` returned `ok`. The only addition was
  68 retained full-audio identity records. Production data was not changed.
- Native desktop checks showed CHRM and GRAY as two separate offline sources,
  with their own remembered playlist subscriptions and the existing GRAY review.
  Live Decks remained selected with two waiting Players while hardware was off.
- The desktop check found an offline-audio contract regression: a verified USB
  without an available file returned an empty audio URI, which the editor rejected.
  Dev-4 represents this explicitly with a non-file `lumi-unavailable` URI. Rust
  regression covers editor, Local Playback and connected-player preparation;
  a Swift decoder regression preserves waveform and phrase access without demo
  audio or fallback to an unverified old path.
- Final dev-4 validation passed: 459 Rust tests in the full portable gate, with
  the same 14 deliberate ignores, and the complete safe Apple gate including
  64 Library tests and native Mac/iOS builds. The installer audit passed for the
  clean code revision `bb7f778a9b2d`, build 359, installed in the Dev channel only.
- The installed dev-4 desktop opened 90s Bitch at R38 with protection enabled
  and Ready for Show retained; My Favourite Regrets opened at R49. Search and
  Clear worked, stored waveform colors/phrases rendered, and neither selection
  produced the editor failure. Audio remained explicitly unavailable while the
  USBs were offline. The database still contained 114 tracks, 264 revisions and
  68 audio-identity records with `quick_check` returning `ok`. Dev-4 was left
  open on Live Decks in Off mode, waiting for the physical Players.

Still required: actual local and cross-Player loads on both USBs, native RB media
colors, cue/start/pause/master changes, updated-track adoption and lighting timing
with the real CDJs. The iPhone simulator could not be opened through desktop
automation during this check; package/client tests are not a substitute for that
native Remote acceptance. No general simulator NFS emulation is claimed.

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

Physical gate: retrieve the same marker read locally from CHRM in Player 1 and
GRAY in Player 2; repeat after swapping the sticks. Do not call this supported
until actual CDJ-1500X tests pass. If either export path or firmware rejects the
file, record that fact and discuss the next identity strategy before expanding.

## Phase 2 Isolated resolution service

Implement bounded asynchronous jobs, single-flight coalescing per source slot,
device/media generations, capped retries with backoff and cancellation. Keep all
file I/O, parsing and SQLite preparation outside the realtime pump. Resolve only
known trusted markers. Expose source name, resolution state, last verified time
and actionable failure in existing Pro DJ Link diagnostics.

The owner explicitly requires real CDJs first, then simulator behavior based on
those observations. Use fixture transports for automated fault tests. The current simulator provides
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

## Physical evidence from 2026 10 04

The opt-in `MediaIdentityProbeMain` entry point was added to the Java bridge
module for the Phase 1 POC. Neither BridgeMain nor the engine started it in that
phase. It uses pinned Crate Digger
0.2.1 RPC records and Remote Tea 1.1.4 to perform bounded UDP portmapper, mount,
LOOKUP and READ calls. It never starts a VirtualCdj or a metadata finder. The
root USB mount is `/C/`; the only filename is `.lumi-media.json`.

The reader rejects non-regular files and sizes outside 1..4096 before READ. Its
custom READ decoder rejects an oversized XDR payload before allocation, using
at most 1024 bytes per chunk. File handle and modification identity are checked
again after reading. RPCs have at most 1200 ms each within a five-second monotonic
budget; a separate parent kills and reaps a worker exceeding eight seconds.
An exact local SHA-256 reference is mandatory. This POC validates access only;
it does not register sources or trust network data as authenticated evidence.

Results:

- Local CHRM marker: valid schema 1, 108 bytes. Local GRAY marker: valid schema 1,
  125 bytes, distinct media and source IDs. Neither marker was changed.
- Owner moved CHRM to the physical CDJ-1500X Player 1 and left it in a playing
  loop. Lumi's native Pro DJ Link page identified both CDJ-1500X Players and the
  DJM-V5. Two isolated requests returned CHRM's exact local bytes via NFS:
  `verified_exact_bytes`, 94.092 ms and 75.716 ms worker elapsed time. These are
  two file-read samples, not a latency distribution or a lighting benchmark.
- Owner moved GRAY to physical Player 2, loaded a local track and left it paused.
  Two requests returned all 125 local marker bytes exactly: 280.498 ms and
  83.018 ms. A deliberate comparison of GRAY's remote marker with CHRM's local
  reference returned `reference_mismatch` (exit 1), not a successful identity.
- Owner paused both Players, safely ejected the sticks, swapped them and loaded
  local tracks while keeping both paused. Player 1 then returned GRAY's exact
  125 bytes in 445.038 ms; Player 2 returned CHRM's exact 108 bytes in 85.922 ms.
  The identity followed the medium rather than the device number. This passes
  the Phase 1 fixed-file gate on these CDJ-1500X units and media, not the full
  production-cache/media-generation or cross-Player acceptance gate.
- All 17 Java regressions pass: five existing bridge tests plus twelve new
  marker, chunk-limit, XDR-bound, media-change, schema, address, deadline and
  child-cleanup tests. Missing-file behavior uses an injected transport; no real
  missing-file or unreachable-server acceptance is claimed.
- The sandbox initially rejected networking with `Operation not permitted`.
  The same authorized read-only request outside that sandbox succeeded. This
  was an execution-permission failure, not CDJ rejection.
- Native UI after both reads still reported connection READY, three devices and
  continuing bridge traffic. Exact position authority remained WAITING, as it
  was before the probe. Lumi stayed Off. This does not prove live matching or
  output correctness; the approved source-scoped hydration fixes are still open.
- After GRAY was loaded on Player 2, native diagnostics also showed exact
  position authority READY. The existing installed app was not replaced.
- After the swap, native Overview still showed READY and all three devices.
  Live retained Player 1's prepared 90s Bitch plan; Player 2 showed external
  track 1012 with AUTO HELD. This records the current app's unresolved matching,
  not a failure of the fixed-file probe and not a completed recognition fix.
  Lumi was left open on Live in Off mode; no timing/control setting was changed.
- Read-only registry inspection found GRAY registered in the Production
  library, but no CHRM device-source row there. Dev contains GRAY and an older
  CHRM source key that differs from CHRM's current marker. This is relevant to
  migration/unknown-source handling, not proof of the earlier failed show's
  cause. Never auto-register a network marker or merge these keys by guesswork;
  preserve local authorization, aliases and edited phrases in migration tests.

## Phase 2 implementation and evidence from 2026 10 04

The engine now has a separate USB identity worker. The realtime pump only polls
bounded completion and job queues; it performs no NFS, process startup, JSON
parsing or SQLite work for this resolver. One read runs at a time, with four
Player slots, device generations, cancellation, capped retry backoff and a
15-second successful-identity recheck. A direct Java worker has a five-second
RPC budget and an eight-second Rust supervisor deadline. Its bounded stdout is
read and the process is killed/reaped on cancellation or expiry. It starts no
VirtualCdj, metadata finder or nested Java process.

Unlike the exact-reference Phase 1 POC, this worker reads a validated marker
without a supplied local SHA. That is not authorization: schema 19 separately
stores the marker-to-existing-source binding established by a local USB scan or
sync. The network reader opens SQLite read-only, cannot create a trusted source
and reports unknown/conflict instead of registering or guessing. Local binding
preserves legacy canonical source keys and their existing aliases and phrases;
conflicting local physical evidence is sticky rather than rewriting the binding.

Evidence so far:

- Rust engine tests: 123 library tests passed, four intentionally ignored;
  process and safe network acceptance tests passed. SQLite: 23 unit and 30
  integration tests passed, one performance test intentionally ignored. Pro DJ
  Link regression suites passed. Strict Clippy passed for all three crates.
- Seven resolver regressions cover delayed replies after reconnect, repeated
  discovery coalescing, deadline/shutdown cleanup, invalid addresses/replies,
  the real Java response contract, copied trusted markers on two Players and a
  slow worker while the pump continues. The debug component test completed
  100,000 non-blocking polls in 7,588 microseconds. This is not an end-to-end
  output latency measurement or the final performance gate.
- SQLite regressions prove that a network marker cannot authorize itself,
  legacy binding does not replace the canonical source, copied local identities
  conflict, older databases remain read-only and colliding track IDs resolve
  only within the specified source. The runtime does not yet use that new track
  lookup; Phase 3 is still required.
- All 63 Swift Library regressions passed, including the new marker/legacy
  registration test. Native Dev build, package audit and strict ad-hoc signature
  verification passed. This is a local test package, not a public release.
- Desktop testing found that the UI preferred CHRM's marker key over its existing
  legacy registration. The fix retains the legacy key only when the marker agrees
  with the current physical fingerprint and there is exactly one matching legacy
  registration. Modern sources, ambiguous legacy names and foreign markers are
  not rebound. The regression covers those cases.
- Real Player 1 returned GRAY's valid 125-byte marker through the packaged reader
  in 59.212 ms. The background worker recovered from unavailable to unknown,
  correctly asking for local registration. Player 2 without a mounted USB remained
  unavailable; neither condition restarted the healthy three-device bridge.
- The corrected native scan showed CHRM CONNECTED under the same legacy source,
  GRAY OFFLINE and the two previously selected playlists remembered. Schema 19
  now binds CHRM's existing marker to that canonical source with no conflict.
  Its marker SHA stayed identical to the Phase 1 reference. No USB write, Sync
  action or phrase/MIDI edit was performed.
- The Dev library retained 114 tracks and 264 timeline revisions. All 42 tables
  common to the pre-migration backup and current database had identical row
  fingerprints after the scan; SQLite integrity returned `ok`. Only the new
  authorization table was populated. Production data is unchanged.
- After the owner safely returned CHRM to real Player 2 and loaded a track,
  the running Dev UI changed its USB line from unavailable to green
  `DJ VIC CHRM · identified` without restarting Lumi or the bridge. Pro DJ Link
  and exact position authority remained READY, with both Players and the DJM
  detected and continuing bridge/position traffic. Player 1's GRAY remained
  unknown and separately requested local enrollment; it was not mislabeled CHRM.
  The CHRM binding remained non-conflicted and track/timeline counts remained
  114/264. No transport or output controls were used for this check.

- The owner then connected GRAY locally. The native read-only scan indexed 86
  playlists and 1,205 tracks, remembered its selected playlist and retained the
  existing canonical GRAY source. Its existing 125-byte marker SHA remained
  unchanged. The new authorization binding is distinct from CHRM's and both
  bindings are non-conflicted. No Sync action or review override was performed;
  the library still contained 114 tracks and 264 timeline revisions.
- After GRAY was safely returned to Player 1 with a track loaded, the native
  Pro DJ Link page simultaneously showed green `DJ VIC GRAY · identified` on
  Player 1 and `DJ VIC CHRM · identified` on Player 2. Both CDJ-1500X Players and
  the DJM-V5 remained detected, with Pro DJ Link and exact position authority
  READY and continuing event/position traffic. No app or bridge restart, transport
  change or output trigger was needed. This passes basic two-source identity
  acceptance, not source-scoped live track or physical lighting acceptance.

## Live USB presentation

The owner requested a compact mounted-USB label in Mac Live and Lumi Remote,
using the native rekordbox/CDJ media color (CHRM Pink, GRAY Blue), not track
color or a user-defined Lumi palette. The label sits below Player identity and
remains available without a loaded track. Its row reserves space so identity
updates do not move the waveform. A separate small status icon distinguishes
verified identity from resolving, unknown, conflict or unavailable.

The bridge listens passively for Beat Link `MediaDetails` updates and forwards
only Player number and native media color ID on the replaceable display lane.
It does not send media queries. Color is presentation evidence, never identity
or trackmatching authority. Players that do not broadcast it retain a neutral
swatch; Lumi does not infer a color from the USB name. Loss/reconnect and failed
identity resolution clear previous color evidence.

The Remote projection exposes a bounded optional `playerUsbs` list with Player
number, identity state, verified source name and native color ID. It excludes
marker IDs, canonical source IDs, network addresses and resolver error details.
Older snapshots remain readable. Live transport anchor updates preserve the USB
list. This addition does not change transport, planning or MIDI decisions.

Implementation: Lumi 0.6.4-dev-2 and Lumi Remote 0.1.3-dev-1. Native UI and
real-CDJ color acceptance must be recorded before declaring this increment done.

Validation on 2026-10-04:

- Local Rust, Java and Swift regression suites passed, including native color
  decoding, trusted-only labels, loss/reconnect invalidation, Remote projection
  privacy and preservation of USB information through transport-anchor updates.
  Strict Clippy, both native builds and the signed Mac installer audit passed.
- The packaged Dev app was installed in `/Applications/Lumi/Dev` and opened.
  Its service reported 0.6.4-dev-2, build 356. Only that Dev engine and bridge
  were running; Production and both databases were left unchanged.
- Native Mac Live showed the reserved USB rows for both empty Players. The
  connected iPhone Simulator showed the same neutral unidentified state in
  portrait and landscape, with the integration statuses remaining visible.
  The existing view-only role was preserved; no controller access was granted.
- Real hardware was not reachable during this UI check: the bridge discovered
  zero devices, and one ping to each previously observed Player address received
  no reply. This is not evidence of a new discovery defect or of correct native
  color reception. The final loaded-Player UI check with GRAY Blue and CHRM Pink
  remains pending. The physical iPhone was not available for installation.

Pending: robust mount/media epochs for live lookup, cross-Player loading,
physical failure cases and end-to-end timing comparison.
Periodic identity revalidation alone is not sufficient to authorize a newly
loaded track after a USB swap. Phase 3 must reject stale media and track-load
results before consuming a source binding. No full live recognition or lighting
acceptance is claimed by this phase. The simulator remains unchanged and has no
NFS/media server.
