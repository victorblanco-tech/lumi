# Story E4-02: Direct Pro DJ Link input and SoundSwitch timing

## 2026-08-10 — Trusted USB and integration UI refinement

- `Library > Import & Sources` now presents USB sources only, with trusted,
  connected/offline, synchronization and version-health status.
- Trusted source rows remain as the compact permanent overview. Expanding a
  connected row refreshes its read-only playlist index after mount/remount;
  the 71 playlists on `DJ VIC CHRM` are selectable before Sync and the complete
  disk is never selected implicitly.
- Lumi standardizes new USB ingestion on Rekordbox OneLibrary. A source that
  only contains the legacy device database is rejected with an upgrade/export
  instruction instead of activating a long-lived compatibility path.
- Expanding a playlist before Sync exposes each track with `Current`, `USB
  newer`, `USB outdated`, `New` or `Review` state. Playlist rows aggregate the
  same states, so a DJ can confirm the expected changed tracks before applying
  anything.
- Selected playlists are durable subscriptions. They are restored by complete
  folder/playlist path as well as source-local ID, so a OneLibrary refresh or a
  backup USB with different internal IDs does not silently lose the selection.
- Track tables expose active `USB Sources` relations per canonical track; the
  relation is queried in one bounded batch with the visible library page.
- Equal full-path playlists on primary and backup USB media render once in the
  Library with a deduplicated canonical-track union while both USB relations
  remain independently visible and synchronizable.
- USB alias matching uses strict metadata first and a bounded audio-content
  signature fallback so renamed backup tracks can resolve safely.
- Analysis promotion is monotone: newer analysis may promote, older analysis
  is protected, and same-date/unknown conflicts are held without overwrite.
- `Integrations > Pro DJ Link` replaces the legacy Beat Link Trigger product
  surface and shows discovery, equipment, addresses, traffic and capability
  status from Lumi's own bridge.
- Physical CDJ-1500X and DJM-V5 compatibility remains pending hardware
  acceptance; the simulator path is verified.

## User outcome

As a DJ, I can run Lumi directly beside supported Pro DJ Link decks without
configuring Beat Link Trigger expressions. Loaded tracks are recognized using
their actual Rekordbox analysis identity and SoundSwitch follows the master
deck's effective tempo and musical phase.

## Functional slice

This story is delivered in visible, independently testable increments:

1. **Bridge health** — Deck Inputs shows the bundled bridge version, pinned
   Beat Link version, process health and discovered players.
2. **Live transport** — Live Decks uses direct play, pause, master, BPM, beat
   and position observations through the existing deck-source contract.
3. **Track recognition** — loaded media identity and Beat Link signature
   reconcile against the read-only Rekordbox Device Library mirror.
4. **Rich deck data** — Lumi hydrates waveform, beat grid and cue information
   when available without blocking transport.
5. **Lighting sync** — the engine publishes master timing through an Ableton
   Link output provider while existing MIDI notes select SoundSwitch Autoloops.
6. **Fallback and diagnostics** — source loss holds automatic output, reports a
   single actionable diagnostic and permits Local Playback without requiring
   Beat Link Trigger.

## Build order

### E4-02A — Bridge foundation

- **Implemented:** Java 21 helper module with pinned `beat-link` dependency.
- **Implemented:** versioned NDJSON envelopes on stdout and commands on stdin.
- **Implemented:** structured stderr diagnostics and deterministic protocol fixtures.
- **Implemented:** Rust bridge decoder, process supervisor and contract tests.
- **Implemented:** app-owned development launch; no Beat Link Trigger process,
  expression or runtime configuration is part of the direct production path.
- **Implemented and app-bundled:** the bridge JAR plus a jdeps-derived Java 21
  runtime ship inside Dev, RC and release app bundles; no host Java install is
  required.
- **Verified on two hosts:** Lumi discovered `LUMI-SIM` player 1 at
  `192.168.1.61`, received a loaded live track and reached `ready` without BLT
  or a manually started bridge.
- Local build and verification scripts remain the development path; no paid
  GitHub Actions minutes are consumed.

### E4-02B — Direct deck observations

- **Implemented:** device discovery and lifecycle through the supervised bridge.
- **Implemented:** Virtual CDJ read-only session.
- **Implemented and contract-tested:** player status, master, effective BPM,
  beat, seek and play/pause translation into provider-neutral observations.
- **Implemented:** exact USB/Rekordbox ID hydration through the existing local
  Library context, reusing Live waveform, beat-grid, phrase and plan rendering.
- **Implemented and tested:** loaded pre-roll and empty-deck status accept beat
  zero without terminating the bridge source; track replacement unloads the
  previous observation atomically.
- **Verified across two hosts:** play/pause, exact seek, pitch/effective BPM,
  master, beat position and live track replacement remain connected in Lumi.
- **Implemented and UI-tested:** a loaded connected deck may have a ready next
  plan before any deck is elected Master; the strict macOS decoder preserves
  this normal DJ preparation state instead of rejecting the snapshot.
- Freshness lease and safe disconnect behavior.
- Replay fixtures captured without copyrighted audio.

### E4-02C — Identity and analysis

- **Implemented:** mounted-media identity uses the stable filesystem UUID;
  legacy mount-specific records for the same physical USB are reconciled
  atomically on the next successful sync.
- **Implemented:** playlist-scoped OneLibrary inspection and sync; the complete
  device is never implicitly selected.
- **Implemented:** pre-sync playlist and track browsing with aggregate and
  track-level freshness/conflict state.
- **Implemented:** playlist subscriptions persist by full path across remounts,
  database revisions and trusted backup media.
- **Implemented:** one visible operation card reports scanning/syncing progress
  and finishes with exact processed, matched, unmatched, protected and held
  counts.
- **Implemented:** active USB alias relations are visible as a Library column;
  obsolete Rekordbox-provider footer copy was removed.
- **Verified against `DJ VIC CHRM`:** a read-only mounted-device sync against a
  temporary Dev database copy matched canonical tracks, preserved direct
  and simulator identity, and consolidated two legacy mount records to one
  stable source record without touching the normal Dev database.
- **Implemented in dev-24:** metadata, beat grid, waveform, hot-cue list and
  signature retrieval. Hot-cue letter, name, loop and source RGB color persist
  once and render consistently in Editor, Local Playback and Live Decks.
- **Completed and physically verified in dev-27:** current OneLibrary point
  encoding is parsed from DAT/EXT, and hot cues use independent provenance so
  they can enrich a held canonical analysis safely. The unchanged GRAY
  `MainStage 140+` sync added the two expected `90s Bitch` cues while preserving
  analysis identity, timeline revision 35 and all 17 authored phrase points.
- Exact identity/signature reconciliation and persisted aliases.
- Explicit ambiguous/unknown result; no realtime fuzzy auto-activation.

### E4-02D — SoundSwitch timing output

**Implementation status for `0.4.0-dev-21`:** D1 and D2 are implemented and
locally verified. D3 status, fail-closed source handling, bridge/helper
recovery and an Off-only side-effect-free helper self-test are implemented.
The managed Link path is verified against SoundSwitch as a real peer for
130 → 140 BPM plus hold/start-stop. D4 remains a complete-show, soak and
physical-hardware acceptance gate.

- **Architecture accepted in ADR-0030:** provider-neutral engine timing
  authority with a managed Ableton Link output adapter.
- Lumi owns master selection, effective BPM, beat/bar phase, transport
  generation, freshness and correction policy. No BLT state or expression is
  part of the production path.
- The first adapter supervises a pinned Carabiner helper as a separate process
  and uses its loopback-only protocol. Carabiner owns only the Link session;
  Lumi owns every source and lighting decision.
- Local Playback and Pro DJ Link publish identical `TimingAnchor` values into
  the same output port.
- Normal beat jitter is filtered. Tempo changes preserve phase; cue, seek,
  track replacement and master handoff establish one new anchor on the first
  reliable beat.
- Link publishes effective BPM, beat, four-beat bar phase and start/stop.
  `Lumi Clock` remains an independently diagnosed MIDI Clock fallback and only
  one timing output may be authoritative.
- Lighting Output Offset remains a phrase-boundary MIDI preroll and never
  falsifies Link tempo or phase. A pending change becomes active on the next
  safe phrase boundary.
- SoundSwitch receives Ableton Link, Lumi Virtual MIDI and optional Control One
  input in parallel. SoundSwitch alone owns the selected downstream DMX
  interface, which may itself be Control One.
- Timing work runs outside SwiftUI, waveform rendering and SQLite. Bounded
  queues and latest-state diagnostics prevent UI load from delaying beat
  publication.
- **Robustness implemented in dev-21:** direct input is pumped by the engine at
  20 ms independent of UI snapshot polling; only exact Pro DJ Link beat packets
  steer playing phase; three missing beats hold Link fail-closed; and a failed
  bridge restarts automatically before fresh timing re-arms the session.
- **Performance evidence implemented in dev-21:** bounded diagnostics count
  received/applied/coalesced anchors, phase corrections, maximum phase error,
  fail-closed/provider failures and realtime pump starvation/lateness without
  adding unbounded logging to the show path.
- **Network acceptance implemented and passed in dev-21:** an opt-in production
  engine test waits for the USB-backed LAN simulator's first precise beat,
  leaves the authenticated client completely idle for three seconds, then
  proves that engine pumps, bridge frames and applied Link anchors all advanced
  without SwiftUI polling.
- **Release-blocking safety implemented in dev-18:** the direct Pro DJ Link
  helper has no Local Playback/app-start lifecycle. Selecting Live Decks first
  verifies that UDP 50000–50002 are available; a same-host Rekordbox conflict
  is rejected before any bridge process or network traffic starts, without
  clearing the Local Playback session.

#### E4-02D5 — Realtime AutoLoop execution and discontinuities — implemented;
physical acceptance pending

AutoLoop selection is the primary realtime output of Lumi. Ableton Link keeps
the selected SoundSwitch loop on tempo and phase, but does not decide which
loop must start. D5 completes that decision and execution route independently
from SwiftUI.

- normalize exact Pro DJ Link beat packets and absolute status position onto
  the same phrase timeline used by Local Playback;
- derive `PhraseChanged` from the exact Lumi-owned beat grid, not UI polling;
- detect forward and backward discontinuities, including hotcue and beatjump,
  and invalidate any older output generation;
- schedule the next cue's Bank for the 50 ms SoundSwitch settle interval plus
  one engine-tick safety margin before a predictable phrase boundary;
- replace the blocking 50 ms Bank sleep with bounded non-blocking Bank and
  AutoLoop stages;
- trigger a safely pre-armed AutoLoop on the landing/boundary beat;
- when Lumi enters Start while its Master is already playing, execute the
  current planned phrase once instead of waiting for another phrase event;
- when a discontinuous landing needs another Bank, arm immediately and trigger
  on the first following exact beat rather than starting off-beat;
- interpret output offset as a real signed time delta: negative is early and
  positive is late; predict negative direct-deck offsets from exact future beat
  packets and delay positive offsets with the non-blocking scheduler;
- retain bounded metrics for requested, pre-armed, emitted, cancelled, late and
  one-beat-fallback cues;
- keep Control One parallel: a manual override remains possible, while Lumi
  reasserts its plan at the next phrase or discontinuous landing.

Acceptance scenarios cover normal sequential phrases, forward hotcue to Drop,
backward hotcue, beatjump across multiple phrases, pause/cue/play, rapid second
seek cancelling the first target, track replacement and master handoff. Each
scenario asserts the exact Bank/AutoLoop mapping and proves no stale command is
emitted. Simulator network acceptance follows deterministic offline tests;
physical SoundSwitch/DMX timing remains a D4 release gate.

Implementation evidence for `0.4.0-dev-23`:

- exact Pro DJ Link Beat packets activate the hydrated Lumi phrase timeline;
- forward and backward position discontinuities are explicit seeks;
- Bank selection and AutoLoop emission are non-blocking scheduler stages;
- normal boundaries use a short settle-window pre-arm and exact beat output;
- unprepared discontinuities use a next-exact-beat fallback and stale context
  cancellation;
- bounded scheduler counters are available in Integrations diagnostics;
- the full local repository suite and opt-in two-host Pro DJ Link LAN timing
  acceptance pass without GitHub Actions.

Additional implementation evidence for `0.4.0-dev-31`:

- deterministic domain coverage proves Start catches up the current playing
  phrase once and retains deduplication across pause/resume;
- direct Pro DJ Link offset scheduling is calculated from effective BPM and an
  exact future beat, including the maximum supported early offset at high BPM;
- Local Playback and direct decks share the same negative-early/positive-late
  convention, while the engine keeps MIDI scheduling independent from SwiftUI;
- existing Dev preferences migrate once from the former inverse sign.

#### E4-02D1 — Timing contract and deterministic authority — implemented

- add provider-neutral timing source, anchor, generation and health types;
- select only the fresh Lumi lighting leader as authority;
- cover effective-pitch BPM, beat/bar mapping, pause, resume, seek, track
  replacement and atomic master handoff with deterministic tests;
- fail closed when the selected source is stale or ambiguous.

#### E4-02D2 — Managed Ableton Link adapter — implemented and locally verified

- add an asynchronous latest-anchor adapter for Carabiner's local protocol;
- automatically launch, supervise and stop the pinned helper;
- validate the helper version and monotonic clock mapping before declaring
  readiness;
- apply tempo only on change, re-anchor only on discontinuity or measured phase
  error, and expose peer count plus current session state;
- retain exact GPL license, source and build provenance in the release bundle.

#### E4-02D3 — Product status and recovery — implemented

- provide a dedicated `Integrations > Ableton Link` workspace with an explicit
  session switch, peer/source/tempo state and an optional saved app-start
  preference;
- expose a compact Link switch and authoritative BPM in Live;
- expose helper health, peers, source deck, effective BPM, beat/bar lock, last
  beat age, phase error, last re-anchor and actionable error;
- consolidate Live system status to Pro DJ Link, Light Output and Ableton Link
  without treating an empty deck or intentionally disabled integration as a
  fault;
- provide an explicit helper test in Diagnostics, disabled while a Live show
  could be disturbed and unable to join or change the shared Link session.
- expose bounded realtime counters in Diagnostics and recover automatically
  from stale input, helper failure and direct bridge exit without an app
  restart.

Implemented status includes independent Link and MIDI readiness, helper
version, peers, source, deck, effective BPM, beat age, phase error, last
re-anchor and actionable failure. Link is off by default and joins only after
an explicit user action or saved auto-start preference; fresh timing anchors
then drive the managed helper and reconnect it when needed.

#### E4-02D4 — Physical acceptance — pending

- run a complete Local Playback song into SoundSwitch without BLT;
- prove pitch, pause/cue/resume, seek and master handoff with the USB-backed
  network simulator;
- perform a one-hour no-drift soak and retain timing metrics;
- physically verify CDJ-1500X, DJM-V5, SoundSwitch, Control One and fixture DMX;
- prove the complete path with no BLT process running.

### E4-02E — Packaging and physical acceptance

- **Implemented locally:** dependency-derived bundled Java runtime, bridge JAR
  validation and helper signing hooks in all macOS channels.
- **Implemented locally:** pinned universal Carabiner 1.2.0 helper, checksum,
  architecture validation, signing hook and GPL/source provenance.
- Third-party license/source inventory and reproducible dependency lock.
- No separately installed Java, BLT or internet dependency.
- Physical player, mixer, SoundSwitch, Control One and DMX acceptance run.

### E4-02S — USB-backed network simulator — implemented

- Development-only Mac mini player; excluded from production packaging.
- Read-only OneLibrary and ANLZ track catalog from the same show USB.
- Real Pro DJ Link discovery, status and beat packet generation for one player.
- Rekordbox ID, exact beat grid, play/pause, seek, pitch, master and on-air.
- Token-protected browser controls and stable HTTP API for remote agent tests.
- `prolink-simulatorctl.sh` wrapper and local packet/config verification.
- Verified on `DJ VIC GRAY`: 1,138 tracks and 64 playlists indexed read-only;
  track ID 1256 resolved as `90s Bitch - Extended Mix` with an exact beat grid.
- Generated announcement, status and beat packets parse successfully through
  beat-link 8.0.0 with USB identity and effective tempo intact.
- **Verified across two hosts:** the Mac mini simulator discovers Lumi as a Pro
  DJ Link peer and unicasts CDJ status to the MacBook's leased player endpoint.
- **Verified safe mismatch behavior:** `DJ VIC CHRM` track IDs 1022 and 1031
  survive load-at-beat-zero, play/pause, seek and +5% pitch changes while Lumi
  displays an external track and holds automatic lighting.
- **Verified exact USB hydration end to end:** with `DJ VIC GRAY` mounted on the
  simulator and mirrored by Lumi, Rekordbox ID 1256 resolves to canonical track
  202 (`90s Bitch - Extended Mix`). The product UI renders the exact RGB
  waveform, beat grid, 17 Lumi phrases and a ready 17-cue Bank 1 Autoloop plan.
- **Verified live transition:** electing the simulated player Master and On Air
  advances the fixed waveform playhead, active phrase and active/next Autoloop
  statuses from the same authoritative deck clock.

## Acceptance criteria

- Lumi starts and stops the bridge automatically with the engine.
- A bridge crash cannot stop local playback or corrupt a lighting plan.
- Two fixed Lumi deck identities follow two physical player identities.
- Pitch changes update effective BPM without double application.
- Master changes preserve the correct deck and do not select an unrelated
  Autoloop.
- Hotcue and beatjump landings activate the destination phrase rather than
  replaying an intermediate or previously scheduled Autoloop.
- A safely pre-armed normal phrase boundary reaches CoreMIDI with p95 <= 20 ms;
  a late, unarmed discontinuity uses an explicit next-beat fallback.
- The same track exported to different media resolves by content signature.
- An unknown or ambiguous track remains visible but automatic lighting is held.
- SoundSwitch Autoloops follow Lumi's Ableton Link tempo and bar phase while
  Lumi selects them over the independent virtual MIDI output.
- Control One continues to operate alongside Lumi.
- Link timing, Lumi command MIDI and Control One are parallel SoundSwitch
  inputs; Control One may separately remain its downstream DMX interface.
- No cumulative phase drift occurs during the one-hour soak; measured
  phrase-boundary output targets p95 <= 20 ms and is retained as release
  evidence.
- All bridge and Rust contract tests run locally without paid CI minutes.
- Simulator packet tests and API acceptance checks run locally without paid CI
  minutes.
- The opt-in `prolink_network_acceptance` test runs only against the dedicated
  LAN simulator and remains ignored in deterministic offline verification.

## Out of scope

- Sending load, play, sync or master commands to physical players.
- Replacing SoundSwitch fixture programming or DMX output.
- A native Rust implementation of the Pro DJ Link protocol.
- Full CDJ audio, display, media-server or remote-command emulation.
- Reimplementing the Ableton Link network protocol.
- Making Ableton Link or SoundSwitch the transport authority for physical
  decks.

## Planned Pro DJ Link media presence

When physical-player media-slot metadata is available, the direct Pro DJ Link
adapter will associate a player slot with an already trusted USB identity and
show `Connected to Player N` on that source. This is presence information only:
it never starts a sync or changes the canonical Library implicitly.
