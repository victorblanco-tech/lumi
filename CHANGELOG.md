# Changelog

## 0.6.1 - 2026-09-04

### Fixed

- Keeps the macOS client connected across long-running shows by reusing one
  authenticated Remote Gateway health connection and retrying transient local
  engine socket replacement without retrying invalid credentials or approvals.
- Parks operation, Ableton Link and lighting output safely when an authenticated
  client connection fails unexpectedly, while allowing the persistent engine
  and an open Lumi window to reconnect cleanly.
- Prevents a long-running Remote Gateway session from exhausting local control
  connections and causing Live Decks to disappear.

### Verified

- Completes an extended headed playlist soak with two simulated CDJ Players,
  repeated Master handoffs, changing BPM, locally resolved phrase plans,
  SoundSwitch AutoLoop output and one stable Ableton Link peer.
- Keeps the realtime Pro DJ Link input, exactly-once SoundSwitch MIDI output and
  Ableton Link relay lanes unchanged by the control-plane repair.

### Compatible products

- Lumi Remote `0.1.0` remains the compatible iPhone companion; its independently
  versioned source and validation release do not change in this patch.
- Pro DJ Link Simulator `0.4.0-dev-56` is the independently released development
  test tool used for the playlist soak and is not bundled with Lumi.

## 0.6.0 / Lumi Remote 0.1.0 - 2026-09-03

### Added

- Introduces Lumi Remote, a native iPhone companion for the authoritative Live
  Decks state, operation controls and future phrase-aware Light Plan changes.
- Adds an independently supervised, opt-in Remote Gateway with local Bonjour
  discovery, one-use pairing, pinned TLS and revocable Controller access.
- Establishes independent product versions and release tags for Lumi Remote and
  the Pro DJ Link Simulator within the Lumi repository.

### Changed

- Presents the live Master and prepared next Player in compact portrait and
  landscape layouts with vivid RGB waveforms, Hot Cues, phrases, Light Plans,
  hardware identity and persistent integration health.
- Moves Remote transport rendering into the receiving iPhone clock domain and
  uses bounded native animation so phone presentation cannot pressure the Mac
  show lanes.

### Distribution

- Free physical-iPhone testing uses the immutable `lumi-remote-v0.1.0` source
  tag, Xcode and the tester's Apple Account until TestFlight is enabled.
- Adds controlled draft-release automation for the macOS DMG/checksum/SBOM and
  the independently versioned iOS Simulator validation artifact.

## 0.6.0-dev-12 / Lumi Remote 0.1.0-dev-12 - 2026-09-03

### Fixed

- Translates every Mac transport anchor into the receiving iPhone's local
  clock domain before visual interpolation. Mac and iPhone wall-clock drift can
  therefore no longer exhaust the 750 ms stale-data guard and pause the Live
  waveform between otherwise healthy updates.
- Retains source-side observation age and raw source ordering separately, so
  delayed or reordered anchors still cannot rewind playback.

### Tests

- Adds a deterministic mismatched-clock regression covering both the initial
  snapshot and subsequent transport anchors.
- A 20.951-second physical aiVoon Animation Hitches trace during live LAN
  simulator playback recorded no hitches and no interaction delay above 33 ms;
  after trace startup, displayed surfaces remained between 53 and 60 per
  second instead of periodically dropping to 20–49.
- Leaves the accepted high-resolution RGB raster, fixed 22% playhead, 40-bar
  Live zoom and macOS waveform renderer unchanged.

## 0.6.0-dev-11 / Lumi Remote 0.1.0-dev-11 - 2026-09-03

### Fixed

- Preserves the actual monotonic observation time of each canonical Pro DJ
  Link beat in Remote transport anchors. The iPhone no longer re-anchors a
  beat to the later gateway publication time and therefore does not step back
  slightly at each incoming beat.
- Keeps tempo-only updates phase-continuous while explicit play, pause, seek,
  Hot Cue and track-load discontinuities still re-anchor immediately.

### Performance

- Moves only the live Master waveform at display rate; a prepared Player keeps
  its fixed overview without running a second display clock.
- Uses the physical iPhone's native refresh rate and moves only the waveform
  layer per frame. Beatgrid, Hot Cue and playhead geometry are recalculated
  only for layout, track or zoom changes.
- Applies continuous socket anchors on the next display VSync instead of
  inserting extra off-cycle layer updates.

### Tests

- Regression-locks canonical beat timestamps across the Rust Remote boundary
  and proves that a tempo-only status update cannot reset playback phase.
- Builds and tests the actual iOS/UIKit renderer with warnings as errors.

### Known limitation

- Physical-device playback exposed a remaining clock-domain error: Remote
  interpolation still compared a Mac wall-clock timestamp with the iPhone
  wall clock. When those clocks differ, the 750 ms stale-data guard can pause
  the visual timeline before the next anchor. This is addressed in dev-12.

## Lumi Remote 0.1.0-dev-10 - 2026-09-03

### Fixed

- Replaces the per-frame SwiftUI waveform repaint on iPhone with one stable,
  high-resolution RGB track raster that moves through Core Animation.
- Keeps the live Master playhead fixed at 22% while pinch zoom changes only the
  musical viewport; a pinch can no longer accidentally enter inspection mode.
- Moves phrase and Light Plan animation onto a separate bounded 30 Hz visual
  clock so waveform movement does not rebuild the complete Player surface.

### Tests

- Builds the actual iOS/UIKit renderer for the iPhone Simulator.
- Regression-locks the fixed playhead across all supported zoom levels and
  retains bounded inspection for a prepared non-Master Player.

## 0.6.0-dev-10 / Lumi Remote 0.1.0-dev-9 - 2026-09-02

### Fixed

- Prevents unchanged Bonjour result callbacks from tearing down and rebuilding
  a healthy pinned-TLS Remote session.
- Makes reconnect, service replacement and app suspension generation-safe so a
  cancelled connection or command can no longer overwrite the state of its
  replacement.
- Disconnects a LAN client that stops reading and bounds a stalled
  gateway-to-engine command response, preserving Remote capacity without
  affecting the autonomous show lanes.
- Keeps compact landscape controls visually small while giving every show-mode
  and timing control a full 44-point touch target.
- Improves VoiceOver status, value and action descriptions for Pro DJ Link,
  Light Output, Ableton Link, show mode and timing offset.

### Tests

- Adds connection-generation regression coverage for duplicate and replaced
  Bonjour results.
- Stress-tests four clients with 20,000 two-Player transport anchors while
  proving bounded latest-value state and contiguous delivery sequences.
- Adds deterministic slow-writer and stalled-engine-response deadlines.
- Covers Live/Next Player selection on a four-Player network and retains the
  same 40-bar fixed Live viewport across iPhone orientations.

## 0.6.0-dev-9 / Lumi Remote 0.1.0-dev-8 - 2026-09-02

### Fixed

- Keeps the Master playhead at the same 22% Live position for the complete
  track on macOS and iPhone, including the first and last bars.
- Renders empty pre-roll and post-roll as black instead of moving the playhead
  or stretching the first/last waveform sample.
- Makes 40 visible bars one tested iPhone Live contract in both portrait and
  landscape; rotation changes the layout, not the musical zoom level.
- Keeps the PDL, LIGHT and LINK health indicators in the Remote header during
  discovery, reconnect and unavailable states, using neutral unavailable
  status instead of removing the indicators.
- Sends the complete 16,384-point RGB waveform losslessly in a compact static
  track projection and renders it with the same normalized line treatment as
  macOS; frequent transport anchors stay small and independent.

### Tests

- Covers fixed-playhead behavior at track start, normal playback and track end
  in both Live clients, plus out-of-track waveform sampling.
- Verifies two complete lossless Player waveforms fit the 512 KiB Remote frame
  and do not enlarge desktop polling snapshots or realtime transport updates.

## Lumi Remote 0.1.0-dev-7 - 2026-09-02

### Changed

- Mirrors Lumi's live-plan hierarchy in the booth UI: the running phrase and
  AutoLoop receive a red live glow while exactly one upcoming phrase and
  AutoLoop receive the blue `NEXT` treatment.
- Keeps the configured Phrase colors visible underneath the status treatment
  and preserves the entire upcoming block as its touch target for adjustment.
- Advances `ACTIVE` and `NEXT` from the same interpolated transport position as
  the waveform, so the status cannot visibly lag behind the fixed playhead.

### Tests

- Proves completed, active, next and later planned classification, including
  the exact phrase-boundary handoff.
- Completes headed portrait and landscape acceptance against the running Lumi
  Remote Gateway and LAN Pro DJ Link simulator.

## Lumi Remote 0.1.0-dev-6 - 2026-09-02

### Changed

- Makes both the coloured Phrase band and proportional Light Plan blocks
  touch targets that open one compact phrase editor.
- Adds an in-sheet Phrase selector followed by the current Theme/Bank,
  AutoLoop, Static Look and lock state.
- Keeps running and completed phrases inspectable while only allowing the
  Controller to mutate an upcoming phrase.
- Shows a subtle adjustment affordance on editable future Light Plan blocks.

### Tests

- Covers unavailable, completed, live and planned phrase states and proves
  that only an upcoming phrase owned by the Controller is editable.

## 0.6.0-dev-8 - 2026-09-02

### Fixed

- Downsamples bounded Live and Remote waveform previews by selecting the
  loudest real RGB sample instead of independently combining channel peaks.
- Preserves the source hue and prevents artificial white/pastel waveform
  columns while retaining peak height and bounded visual payloads.

### Tests

- Proves bounded preview downsampling retains a real source hue and never
  invents a mixed colour from neighbouring samples.

## Lumi Remote 0.1.0-dev-5 - 2026-09-02

### Changed

- Uses Lumi's shared Rekordbox-compatible RGB channel mapping, amplitude curve
  and normalized hue instead of the former flat direct-channel rendering.
- Keeps two relevant numbered Player surfaces visible, including a stable empty
  slot while the second Player has no loaded track.
- Orders numbered Players left-to-right in landscape and keeps the Master first
  in the scrollable portrait composition.
- Compresses the landscape status and operation controls into one toolbar row.
- Places Player identity, track metadata, transport metadata and role on one
  compact landscape row so phrases and the complete Light Plan remain visible.
- Adds readable Phrase Type labels to sufficiently wide phrase segments.

### Tests

- Covers the shared waveform color curve, single-Player placeholder behavior,
  portrait Master-first ordering and landscape physical-number ordering.

## 0.6.0-dev-7 - 2026-09-02

### Fixed

- Rejects a stale Remote Gateway service record from an older Lumi build and
  re-registers the bundled helper when the user enables the updated service.
- Publishes the gateway's ephemeral TLS port in release-scoped Bonjour discovery
  metadata so the iOS Simulator can avoid its synthetic host-service route;
  the existing certificate pin remains the trust boundary.
- Gives the user an actionable update message instead of reporting an obsolete
  helper as Ready.

### Tests

- Completes headed Mac-to-iPhone Simulator acceptance through QR pairing,
  explicit approval, Controller transfer, Live projection, `ARM`, `START`,
  confirmed `OFF`, moving transport and credential-backed app relaunch.

## Lumi Remote 0.1.0-dev-4 - 2026-09-02

### Fixed

- Gives an explicitly scanned pairing invitation precedence over a stale
  Keychain credential, allowing a trusted Mac to be paired again after its
  local Remote trust state was reset.
- Replaces the stale credential only after the new invitation is approved and
  the pinned-TLS pairing succeeds.
- Bypasses unreadable legacy Keychain data while a deliberate new pairing is
  in progress, so recovery cannot stall before the TLS connection begins.
- Keeps certificate evaluation off the Network.framework connection queue and
  uses an explicit loopback endpoint only in CoreSimulator, while physical
  iPhones retain normal multi-interface Bonjour routing.
- Declares the Keychain access group required by signed iPhone and headed
  Simulator builds.

## 0.6.0-dev-6 - 2026-09-02

### Fixed

- Aligns the macOS Remote Gateway supervisor with the Rust admin wire contract
  for `Id` and `Sha256` fields, so an enabled gateway reaches Ready instead of
  remaining on Starting.
- Aligns invitation approval, device revocation and Controller-transfer request
  keys with the same lower-camel-case contract.

### Tests

- Adds matching Rust and Swift contract coverage plus the protected loopback
  invitation-and-approval integration test.

## 0.6.0-dev-5 - 2026-09-02

### Added

- Completes the opt-in, separately supervised local-network Remote Gateway with
  channel-scoped Bonjour discovery, pinned TLS and explicit QR approval.
- Connects the native Lumi Remote iPhone client to the authoritative Live
  projection and revision-safe booth command allowlist.
- Adds paired-device management, one explicit Controller lease, revoke and
  Controller transfer in `Integrations > iPhone Remote`.

### Changed

- Uses the same dark booth presentation, RGB waveforms, Hot Cues, beatgrid,
  current user-configured Phrase colors and proportional Light Plans on iPhone.
- Keeps full deck/plan state change-driven and bounds visual transport anchors
  to a coalescible 20 Hz presentation stream.

### Safety

- Keeps the Remote Gateway outside Pro DJ Link, SoundSwitch MIDI and Ableton
  Link execution, with bounded clients, queues, frames and authentication.
- Disables controls across reconnect, sequence gaps and revision conflicts
  until a new authoritative snapshot and Controller lease arrive.
- Stores only credential verifiers on Mac and credentials in iPhone Keychain;
  backgrounding or disconnecting queues no command.

## 0.6.0-dev-3 - 2026-09-02

### Added

- Starts the native Lumi Remote product with an independent `0.1.0-dev-1`
  version, iOS app target and controlled draft-release path.
- Adds the bounded Remote v1 Live projection and command contract with shared
  Rust/Swift fixtures, contiguous client delivery sequencing and fail-closed
  gateway policy.
- Adds the native portrait and landscape Live presentation foundation,
  release-scoped Bonjour discovery, QR invitation validation and channel-bound
  Keychain credential storage.

### Safety

- Keeps the Remote Gateway LAN listener and every remote mutation disabled
  until pinned TLS, persistent Mac trust, explicit pairing approval and the
  isolated engine command path are complete.
- Adds architecture checks that keep iPhone presentation and gateway work out
  of Pro DJ Link, Ableton Link, SoundSwitch output, Library and Local Playback.

## 0.6.0-dev-2 - 2026-08-31

### Changed

- Starts the post-0.5.2 development line with a measured physical-master tempo
  latency baseline as its first priority; the stable 0.5.2 behavior remains the
  comparison point and Production data is not migrated automatically.

## 0.5.2 - 2026-08-31

### Changed

- Bounds local control-plane operations and validates process ownership before
  lifecycle actions, without moving UI or data work into realtime lanes.
- Makes direct Pro DJ Link and trusted OneLibrary USB media the only supported
  production providers; retired BLT, XML and direct Rekordbox-database product
  paths are no longer exposed silently.
- Isolates USB synchronization in a supervised worker and makes SQLite
  durability, contention and recovery behavior explicit and regression-tested.
- Shows the actual Pro DJ Link Player number and announced hardware model in
  Live instead of invented Deck A/B labels.
- Expands local and GitHub quality gates while keeping heavyweight dependency
  and licence audits away from ordinary development pushes.

### Fixed

- Prevents equal-model trusted USB disks from appearing connected merely
  because another disk can resolve a retained security bookmark.
- Prevents the complete Live layout from being invalidated by metadata refresh
  work several times per second.
- Uses one full 8-bit RGB contract for bounded and detailed Library waveforms,
  and rejects cancelled or stale raster results during track and zoom changes.

### Compatibility

- Preserves the existing 0.5.1 Production database and configuration; Dev data
  remains isolated and is never packaged or copied automatically.
- Apple Silicon and macOS 15 or newer.
- Public Beta, ad-hoc signed and not notarized; macOS may require `Open Anyway`
  on first launch.

## 0.5.2-dev-9 - 2026-08-31

### Fixed

- Derives trusted USB connection state exclusively from the current mounted
  volume inventory; a retained security bookmark can no longer make an absent
  equal-model backup disk appear connected.
- Uses one 8-bit RGB colour contract for bounded and detailed Library
  waveforms, removing the muted-colour transition during every Live load.
- Retries Local Playback's visual-only detail request on bounded UI-lane
  contention and rejects a late result after the deck has loaded another
  track, so a temporary busy client cannot leave muted fallback colours.
- Cancels superseded waveform raster jobs and rejects stale results so a
  previous track or zoom render cannot flash back into the Live surface.

## 0.5.2-dev-8 - 2026-08-31

### Fixed

- Stops the Live metadata strip from invalidating the complete two-Player and
  Library layout four times per second. Waveform and Light Plan motion remain
  on their independent Core Animation clocks.
- Prevents a loaded Local Playback Player from saturating the macOS main thread
  and blocking a subsequent Player load.

## 0.5.2-dev-7 - 2026-08-31

### Changed

- Replaces Deck A/B labels with the actual Pro DJ Link Player number throughout
  Live and Local Playback.
- Shows the exact detected hardware model beneath a connected Player number
  without allowing presentation metadata to affect timing or planning.

## 0.5.2-dev-6 - 2026-08-31

### Changed

- Adds Engine Client coverage to the fast native `dev` gate.
- Adds an independent weekly/manual dependency and release-license audit while
  keeping heavyweight security work away from ordinary pushes and pull
  requests.
- Validates the SPDX inventory and notices for the separately packaged Pro DJ
  Link, Ableton Link and USB database runtimes.

## 0.5.2-dev-5 - 2026-08-31

### Changed

- Removes the unreachable Rekordbox XML discovery, mirror and direct-analysis
  presentation from the macOS product; trusted OneLibrary USB sources remain
  the single supported ingestion workflow.
- Deletes the permanently disabled predictive AutoLoop scheduler so the tested
  exactly-once execution lane is the only runtime implementation.
- Moves the large session, library and SQLite fault-test modules into focused
  source files without weakening their access to private implementation seams.
- Adds architecture guards against restoring retired UI and timing paths.

## 0.5.2-dev-4 - 2026-08-31

### Changed

- Removes USB inspection, synchronization and conflict resolution from the
  realtime engine protocol; these operations now have one isolated worker path.
- Makes SQLite WAL, NORMAL durability, checkpoint cadence and the two-second
  contention deadline explicit and regression-tested.
- Ensures ordinary USB commit overlap can recover while a stalled data process
  remains bounded and cannot occupy Pro DJ Link, Ableton Link or MIDI output.

## 0.5.2-dev-3 - 2026-08-31

### Changed

- Retires the Rekordbox XML and direct local-database commands from the
  authenticated engine protocol and macOS product command surface.
- Keeps mounted Rekordbox OneLibrary USB media as the only supported product
  ingestion path while preserving existing Production library data.
- Adds an architecture gate that prevents the retired commands from returning.

## 0.5.2-dev-2 - 2026-08-31

### Changed

- Makes direct Pro DJ Link the only production Connected Deck provider.
- Removes the retired Beat Link Trigger MIDI input, virtual destination,
  diagnostics UI and silent runtime fallback from the macOS product.
- Adds an architecture gate that prevents the product engine or Swift UI from
  reintroducing the BLT runtime path accidentally.

## 0.5.2-dev-1 - 2026-08-31

### Changed

- Starts the 0.5.2 runtime and codebase hardening cycle from the public 0.5.1
  product baseline. The work is deliberately limited to reliability,
  maintainability, security and regression protection; it does not add a new
  show workflow or alter Production data automatically.

## 0.6.0-dev-1 - 2026-08-30

### Changed

- Starts the next isolated development cycle after the 0.5.0 production
  release. No production data or configuration is migrated automatically.
- Positions Lumi explicitly as a Public Beta and adds structured field-test
  guidance and a GitHub issue form for different hardware combinations.
- Corrects the production third-party inventory and makes future DMGs include
  installed legal notices, complete Carabiner/Ableton Link source with nested
  submodules and all pinned Java runtime source artifacts.
- Expands the SPDX SBOM beyond Cargo to cover the Java bridge, OpenJDK,
  SQLCipher/OpenSSL and the separately executed Ableton Link helper.

## 0.5.1 - 2026-08-30

### Changed

- Labels the GitHub-distributed build explicitly as Public Beta and adds a
  concise field-testing guide plus a structured hardware feedback form.
- Completes the public distribution package with installed legal notices,
  complete corresponding source for Carabiner/Ableton Link and the pinned Java
  bridge dependencies, and an expanded SPDX SBOM.
- Corrects the third-party inventory for the production Pro DJ Link runtime.

### Compatibility

- Functionally identical to 0.5.0; existing Production data remains compatible.
- Apple Silicon and macOS 15 or newer.
- Ad-hoc signed and not notarized; macOS may require `Open Anyway` on first
  launch.

## 0.5.0 - 2026-08-30

### Added

- Adds a complete USB-first preparation workflow for Rekordbox OneLibrary
  media, including trusted-source identity, selected-playlist synchronization,
  change review, beatgrid/waveform/hot-cue import and creative phrase relink.
- Adds configurable Track Preparation queues, phrase protection and a
  CDJ-style RGB Track Editor with local audio playback.
- Adds isolated Pro DJ Link, Ableton Link and SoundSwitch output lanes with
  Live Decks, Local Playback, exactly-once AutoLoop changes and Static Looks.
- Adds configurable Light Plans with Track Color rules, Theme eligibility and
  repeat protection.

### Changed

- Production and RC installations now start with an empty, isolated library.
  Demo tracks remain a Dev-only aid; no personal tracks, USB identities,
  mappings, Themes or Light Plan configuration ship in the production app.
- Delivers the user-facing GitHub guide, Retina product screenshots and the
  approved Lumi branding throughout the app and distribution.

### Compatibility

- Apple Silicon and macOS 15 or newer.
- The first public DMG is ad-hoc signed and not notarized; macOS may require
  `Open Anyway` on first launch.
- Existing Dev data remains isolated and is not migrated into Production
  automatically.

## 0.5.0-dev-40 - 2026-08-30

### Changed

- Makes the accepted tall Track Editor waveform layout the first-launch
  default and persists later divider adjustments through AppKit's native split
  view autosave support.
- Makes exact Library matches on physical Live Decks load the same detailed
  8-bit RGB waveform as Track Editor instead of remaining on the bounded
  realtime preview. The visual fetch stays outside the Pro DJ Link and lighting
  timing lanes.
- Replaces the separate README icon and title with one professional widescreen
  Lumi header built from the approved wordmark.
- Expands the user-facing integration documentation with Retina screenshots and
  dedicated explanations for Pro DJ Link, Ableton Link and SoundSwitch.

### Verification

- Adds a Library layout contract for the default editor height and stable
  autosave identity.
- Adds a Live snapshot contract for canonical Library track identity and runs
  the complete 53-test Live Workspace and 62-test Library Workspace suites.
- Builds the complete macOS Dev configuration with warnings treated as errors.
- Verifies the saved divider position across a real app restart and captures
  every documentation image directly from the installed macOS app at
  3600×2260.

## 0.5.0-dev-39 - 2026-08-30

### Fixed

- Restores the accepted vivid red, pink, cyan and blue RGB waveform palette in
  Track Editor, Local Playback and Live Decks without changing waveform shape,
  resolution or timing.
- Centralizes the Rekordbox PWV5 display-channel mapping so every waveform view
  uses the same color interpretation.

### Verification

- Adds Design System regression coverage for RGB channel order, amplitude and
  silent samples.
- Compares the real macOS Track Editor and Local Playback render against the
  previously accepted waveform appearance before packaging the Dev app.

## 0.5.0-dev-38 - 2026-08-30

### Changed

- Replaces hover-driven navigation auto-hide with two explicit, stable states:
  the full navigation and a fixed compact icon rail.
- Keeps every compact destination directly clickable without expanding or
  resizing the navigation, so moving the pointer across it never shifts the
  active workspace.
- Renames the visible action to `Hide navigation` while preserving the existing
  saved preference and its compact/full state across upgrades.

### Verification

- Builds the complete macOS app with Swift concurrency warnings treated as
  errors.
- Verifies on the desktop that hiding the navigation, selecting destinations
  from the compact rail, and leaving the pointer over those controls never
  expands the rail or moves the workspace.

## 0.5.0-dev-37 - 2026-08-30

### Fixed

- Keeps the selected workflow queue and its track page atomic while rapid
  Library queries, engine monitoring and track mutations overlap.
- Uses the authoritative engine query for workflow highlighting, so a zero-count
  queue can never visually claim rows from `Not Started`.
- Keeps `Protect Phrases` at one fixed size and removes its conditional banner;
  toggling protection no longer shifts the waveform or Track Editor layout.
- Keeps the browser mode control fixed and puts workflow queues in their own
  bounded scroll area, so the split divider can no longer cover or compress the
  first `Changed after USB sync` action.
- Corrects the `Changed after USB sync` wire identifier (`Usb`, not synthesized
  `USB`), so the engine accepts that queue instead of retaining the prior page.

### Verification

- Adds a regression proving that toggling phrase protection preserves an active
  empty `Changed after USB sync` query and page.
- Pins every Swift workflow-filter raw value to its exact Rust protocol value.
- Replays the exact desktop sequence: select the empty queue, protect the open
  track, switch between workflow steps, and verify count, highlight and rows
  remain consistent.

## 0.5.0-dev-36 - 2026-08-30

### Added

- Adds per-track `Protect Phrases`, with a persisted lock that covers phrase
  points, roles, boundaries, history restores and Autoloop choices.
- Gives `Ready for Show` its expected green check presentation.
- Presents empty workflow queues as `Nothing to review` instead of a Library
  failure.

### Safety and verification

- Enforces phrase protection inside the Library engine, independent of the UI.
- Keeps trusted USB beatgrid, waveform and cue updates active while protected;
  source changes still create an explicit workflow review.
- Adds schema 18, optimistic lock revisions and migration/persistence/regression
  coverage without adding work to any live timing lane.

## 0.5.0-dev-35 - 2026-08-29

### Added

- Completes the configurable Track Preparation Workflow with ordered custom
  steps, visual identity and automatic quality gates.
- Adds fixed `Changed After USB Sync` and `New Track Versions` safety queues,
  including live counts in the Library.
- Adds guided successor review: reuse Lumi phrases only when the predecessor
  and successor have an exact compatible beat timeline, or keep both versions
  separate.

### Safety and verification

- Keeps all workflow evaluation and version review inside the Library worker,
  isolated from Pro DJ Link, Ableton Link and lighting output lanes.
- Adds schema 17 migrations, optimistic revisions and revision-scoped review
  decisions so later USB changes cannot inherit stale choices.
- Covers the Rust domain, SQLite repository, engine protocol, Swift decoder and
  real macOS app. Desktop acceptance found and fixed a lightweight-refresh
  regression that could temporarily hide the workflow catalog.

## 0.5.0-dev-34 - 2026-08-29

### Added

- Adds the first Track Preparation Workflow with `Not Started`, `In Progress`
  and `Ready for Show` statuses.
- Adds a paged `Changed after USB sync` inbox with exact metadata, waveform,
  beatgrid, hot-cue and source-phrase reasons.
- Adds workflow navigation, counts, a Library table status and editor controls
  to complete source-change reviews explicitly.

### Safety and verification

- Keeps workflow state separate from technical readiness and all Pro DJ Link,
  Ableton Link, planning and MIDI realtime lanes.
- Preserves Lumi-authored phrase points on their existing beat indices after a
  trusted USB analysis update.
- Adds schema 16 migration, optimistic revision conflicts, USB-promotion,
  filtering, engine protocol and Swift snapshot regressions.

## 0.5.0-dev-33 - 2026-08-28

### Fixed

- Keeps a live future-phrase Theme change inside the existing engine session;
  the app no longer loses its snapshot connection after accepting the edit.
- Materializes every changed phrase against its own selected Theme/Bank instead
  of reusing the original track Theme's AutoLoop address.
- Preserves all earlier phrase selections atomically and recompiles only the
  selected phrase and its successors.
- Treats a missing mapping in a deliberately sparse Theme as a safe hold, while
  retaining exact bank, AutoLoop and planning evidence for mapped phrases.

### Verification

- Adds an exact regression where a future phrase moves to another Theme whose
  only valid mapping is AutoLoop 32; action and `libraryResolution` must both
  report the selected bank and button 32.
- Replays future-Theme, first-Play, sparse-mapping and live-output regressions.

## 0.5.0-dev-32 - 2026-08-28

### Fixed

- Prevents the first successful physical AutoLoop trigger from terminating the
  engine when the selected Theme has an unmapped Phrase Role later in the
  track.
- Makes output-history enrichment strictly observational: it resolves only
  the executed phrase/address and returns an empty optional diagnostic when
  that metadata is unavailable.
- Adds an exact regression with a mapped Intro, an unmapped later Drop,
  Arm, Start and first Play; the opening output is emitted and the subsequent
  Live snapshot remains valid.

### Verification

- Reproduced the physical failure and captured the durable fatal cause:
  `the selected Theme has no entry for this Phrase Role`.
- Confirmed the engine exit occurred while serializing UI output diagnostics,
  after the realtime MIDI action, rather than in Pro DJ Link or SoundSwitch.
- Repeated the installed-app test on physical CDJ-1500X hardware: Arm, Start
  and first Play remained Live through two observed Bank 3 AutoLoop commands,
  with an unchanged engine PID and zero reported late outputs.

## 0.5.0-dev-31 - 2026-08-28

### Fixed

- Restores the user's saved Live Decks selection before the recovered engine
  becomes Ready, so a launchd restart cannot visibly or functionally fall back
  to Local Playback during Arm/Start.
- Defaults a new performance installation to Live Decks; Local Playback only
  becomes sticky after the user explicitly selects it.
- Isolates Ableton Link helper failures from Pro DJ Link ingestion and the
  sparse SoundSwitch AutoLoop command lane. Link can degrade without
  terminating the engine or discarding prepared deck plans.
- Converts direct Pro DJ Link adapter/reducer failures into a bounded adapter
  restart instead of terminating the complete engine service.
- Persists the fatal cause beside the Dev database if the launchd engine ever
  does terminate, so service recovery no longer erases the diagnosis.

### Verification

- Reproduced the reported fallback and identified a macOS launch-constraint
  termination against a stale Dev service registration.
- Added deterministic source-mode recovery and integration-isolation checks.

## 0.5.0-dev-30 - 2026-08-28

### Fixed

- Recognizes a track that was already loaded and paused/cued on a physical
  CDJ-1500X before Lumi Live Decks starts. Lumi now hydrates its exact trusted
  USB track, phrases and Light Plan before the first Play press.
- Decodes the Rekordbox track ID from the observed 512-byte CDJ-1500X extended
  status layout when Beat Link 8.0 exposes only its legacy `NO_TRACK` fields.
- Drops precise-position callbacks safely while their matching deck status is
  not yet cached instead of allowing a Beat Link null-status exception.

### Verification

- Added isolated regressions for the CDJ-1500X extended loaded-track ID, true
  unloaded state, unsupported packet lengths and other player models.
- Verified read-only against two physical paused/cued CDJ-1500X players: IDs
  1256 and 1237 were published immediately and matched the Dev USB library.
- Verified exact position continues for both paused decks without null-status
  callback failures.

## 0.5.0-dev-29 - 2026-08-28

### Fixed

- Made Rekordbox Track Color an optional planning influence again: an absent
  color no longer blocks an otherwise usable automatic Light Plan.
- Kept Theme and AutoLoop `Only` rules hard without falling back to an
  excluded Theme or variant.
- Changed partial Theme coverage from a whole-track failure to an exact,
  provider-safe phrase hold. Lumi sends no command for the unmapped Phrase
  Role and the existing SoundSwitch AutoLoop continues until the next mapped
  cue.
- Limited automatic Theme rotation to the enabled, color-eligible Themes with
  the best exact phrase coverage and a mapped opening phrase, preventing a
  sparsely configured Bank from winning merely because it is eligible.

### Verification

- Added regressions for no-color `Only` exclusion and partial Phrase Role
  coverage.
- Reproduced the original failure with the real Player 1 loop and the Dev
  Library before implementing the fix.
- Verified the installed macOS build through Live Decks: `90s Bitch` remains
  ready without Track Color, selects BLUE RED GREEN, and arms on the first
  user action.

## 0.5.0-dev-28 - 2026-08-28

### Fixed

- Treats Theme Strategy `Only` as a hard eligibility rule. A track without the
  required Rekordbox Track Color can no longer fall through to that Theme just
  because it is the only fully mapped Bank.
- Detects metadata-only USB changes after a previous `Keep Lumi` review, so a
  later Rekordbox Track Color update becomes synchronizable even when beatgrid,
  cues and analysis are unchanged.
- Marks a Library deck as `AUTO HELD` when no plan exists instead of presenting
  its recognized track as plan-ready.

### Improved

- Explains exactly why a recognized track has no plan, including the missing
  Phrase Roles per Theme or the Track Color rule that excludes an otherwise
  complete Theme.
- Keeps every failure fail-closed: Lumi holds the current look and never uses a
  different Phrase Role or silently violates Theme Strategy.

### Verification

- Covers hard `Only` eligibility, matching-color eligibility and metadata-only
  changes with an unchanged kept-active analysis revision.
- Verifies the current Dev library diagnosis: `My Favourite Regrets` needs
  `BD CHORUS 2` in BLUE PINK or both Pre-Drop roles in GREEN PINK / BLUE RED
  GREEN before any one coherent Theme can cover its complete timeline.

## 0.5.0-dev-27 - 2026-08-28

### Fixed

- Keeps the Pro DJ Link, deck-state and Ableton Link lanes alive when a
  recognized Library track has no enabled Theme that can resolve every phrase.
- Treats an incomplete or deliberately disabled track-specific Light Plan as
  a safe no-output condition instead of terminating the engine and falling
  back to Local Playback.

### Verification

- Adds regressions for both an empty executable Theme set and a physically
  executable Theme disabled by the active Light Plans policy.
- Reproduces the cold-start failure with the current Dev database, then keeps
  both connected decks ready while processing thousands of Pro DJ Link frames.

## 0.5.0-dev-26 - 2026-08-27

### Fixed

- Keeps Live Decks selected when a library-matched CDJ track exposes only a
  subset of configured output Themes.
- Falls back to the track's first executable Theme when every available Theme
  is excluded by a non-matching `Color only` rule, instead of terminating and
  restarting the engine in Local Playback.

### Verification

- Reproduces the exact one-bank, non-matching `Color only` configuration in
  both planner-level and engine-level regression tests.
- Exercises the installed development build against live Pro DJ Link input and
  verifies that the engine remains connected without a launch-service restart.

## 0.5.0-dev-25 - 2026-08-24

### Fixed

- Resolves Rekordbox track colors from the fixed OneLibrary palette ID instead
  of its user-renamable label, so custom labels such as `Zweef` retain their
  authoritative blue RGB value.
- Backfills colors that earlier Lumi builds stored as empty on the next normal
  USB playlist synchronization without replacing newer analysis data.

### Improved

- Presents one consistent track-color swatch in Tracks, Local Playback and
  both Live Decks, including an explicit hollow state for uncolored tracks.

### Verification

- Covers fixed palette IDs, custom Rekordbox labels, missing-color backfill and
  the shared macOS presentation model.
- Confirms the reader against the connected GRAY OneLibrary USB source.

## 0.5.0-dev-24 - 2026-08-24

### Added

- Adds immediate, cancellation-safe Library search with a visible clear action.
- Makes every Tracks column server-sortable across the complete result and all
  pages, with native macOS sort indicators.
- Adds `Reuse Lumi Phrases` for copying an exact-beat-compatible authored
  timeline from an older edit or mashup into a new target revision.
- Registers equal-model USB disks independently with local trusted-source
  identities while leaving every file on removable media untouched.

### Improved

- Keeps Import & Sources status geometry stable and source-scoped while USB
  scanning, synchronization and review actions complete.
- Runs USB parsing in a bounded, short-lived worker so a stalled removable
  volume can never block Pro DJ Link, Ableton Link or MIDI output.
- Uses the worker's actual process-termination event instead of asynchronously
  polling Foundation state, preventing completed physical scans from being
  reported as 75-second timeouts.
- Packages USB inspection as a separate one-shot helper without the permanent
  engine's launch-service identity, so macOS volume consent follows Lumi.
- Persists one explicit security-scoped authorization per trusted USB source;
  subsequent scans reuse it and fail immediately with guidance when missing.
- Documents the short pre-show beatgrid refresh workflow and confirms that
  Rekordbox library, analysis and media data remains read-only.
- Ranks likely `v003` → `v004` successors without silently merging track
  identities or guessing across a changed arrangement.

### Verification

- Adds deterministic server-side sorting coverage across page boundaries.
- Adds revision, source-preservation and exact-beat safety coverage for creative
  timeline reuse.
- Re-runs the USB identity, inspection, selection-impact, source-review and
  bounded Library UI suites.
- Serializes native engine-client tests so every process test exclusively owns
  its fixed CoreMIDI endpoints.

## 0.5.0-dev-19 - 2026-08-23

### Fixed

- Treats `Do Not Sync to Lumi` as a complete exact-revision hold: analysis,
  beatgrid, waveform and hot cues remain on the active Lumi version.
- Shows that held revision as current in playlist impact instead of counting it
  as an update that would appear eligible for synchronization.
- A genuinely newer USB analysis revision automatically becomes reviewable
  again.

### Verification

- Adds regression coverage for exact-revision scoping across analysis and hot
  cue synchronization.
- Re-runs the persistent choice through the connected GRAY device in the real
  macOS UI.

## 0.5.0-dev-18 - 2026-08-23

### Fixed

- Persists `Do Not Sync to Lumi` for the exact reviewed USB revision and only
  asks again after that source changes.
- Keeps CHRM and GRAY attached to their own trusted source when equal-model USB
  disks report the same unreliable hardware serial.
- Shows GRAY's green/orange component comparison after every completed scan.
- Keeps review details and controls stationary while the larger playlist result
  loads below them, preventing scan-completion layout jumps.
- Shows saving progress and any failure beside the review action that caused it.

### Verification

- Adds regression coverage for revision-safe persistent review choices and
  colliding equal-model USB identities.
- Verifies the complete macOS Library workspace test suite with Swift warnings
  treated as errors.

## 0.5.0-dev-17 - 2026-08-23

### Fixed

- Keeps component-level review details available per trusted USB source instead
  of retaining them only for the most recently scanned disk.
- Shows the same green/orange Beatgrid, Cue Points, File Data, Rekordbox
  Phrases and Waveform comparison after switching between CHRM and GRAY.

### Verification

- Adds regression coverage for inspecting two independent USB sources in one
  session and verifies the fix read-only against the connected CHRM and GRAY
  OneLibrary disks.

## 0.5.0-dev-16 - 2026-08-23

- Adds component-level USB conflict review for Beatgrid, Cue Points, File Data, Rekordbox Phrases and waveform projection.
- Adds revision-safe `Ignore This Time`, `Do Not Sync to Lumi` and confirmed `Sync to Lumi & Overwrite` actions while preserving Lumi-authored phrases and AutoLoop choices.
- Shows USB and active-source dates, revision fingerprints and concrete first differences for beatgrid, cue points, metadata, Rekordbox phrases and waveform data.

## 0.5.0-dev-15 - 2026-08-23

### Fixed

- Keeps USB analysis conflicts visible after synchronization instead of
  presenting a previously held conflict as current.
- Shows the exact tracks and provenance reason behind every USB review state.
- Replaces the layout-shifting USB scan banner with a compact, fixed operation
  status in the Trusted USB Sources header.

### Safety

- Review conflicts remain non-destructive: Lumi keeps the active analysis and
  never writes to the connected Rekordbox media.

## 0.5.0-dev-14 - 2026-08-23

### Fixed

- Keeps independent USB media such as GRAY and CHRM as separate trusted sources
  even when FAT32 happens to expose the same filesystem UUID.
- Preserves the exact OneLibrary playlist hierarchy when playlist names contain
  `/`, instead of inventing synthetic folders such as `Psy` or `Tech`.
- Keeps inspection, selection and synchronization state bound to the selected
  trusted USB lane.

### Safety

- Migrates the previous UUID-only trusted source during its next successful
  sync without deleting canonical tracks, Lumi phrases or Light Plans.
- Continues to read Rekordbox media strictly read-only.

## 0.5.0-dev-13 - 2026-08-22

### Fixed

- Renders configured Phrase Colors at their full persisted sRGB value in the
  Track Editor detail and overview lanes.
- Keeps beat-range selection visible through its outline and handles without
  placing an accent tint over the user's Phrase Color.

## 0.5.0-dev-12 - 2026-08-22

### Added

- Adds a revisioned Phrase Color picker to `Settings > Phrase Model`.
- Uses the same authoritative role color in Track Editor, Live Decks, Light
  Plans, SoundSwitch Banks & AutoLoops and the Virtual Controller.

### Safety

- Migrates existing Phrase Roles to schema v15 without changing timelines,
  plans or lighting-output mappings.
- Keeps phrase colors out of the realtime planning and output paths.

## 0.5.0-dev-11 - 2026-08-22

### Fixed

- Treats Rekordbox DAT, EXT and 2EX as one versioned analysis set so beatgrid,
  RGB waveform, raw phrases, exact duration and cues refresh together.
- Draws waveform samples and phrase lanes against the exact Rekordbox time axis
  instead of stretching waveform data evenly over the number of beats.
- Corrects PWV5 RGB channel order and preserves its independent waveform height.
- Repairs the historic leading-partial-bar phrase projection while preserving
  every user-moved phrase boundary, role and AutoLoop strategy in a new timeline
  revision.

### Changed

- Shows USB playlists in their Rekordbox folder hierarchy, collapsed by default,
  with compact folder totals and searchable ancestor disclosure.

## 0.5.0-dev-8 - 2026-08-22

### Fixed

- Consolidates duplicate trusted USB identities only when their display name and
  complete active canonical track set agree; a matching name alone is never
  sufficient.
- Persists an audio location per synchronized USB and resolves playback against
  the currently mounted source, while preserving a valid local Mac audio file.
- Restores Track Editor and Local Playback audio when a canonical track was first
  imported from a different, currently disconnected backup USB.

### Safety

- Existing phrases, timelines, playlists, Light Plans and lighting mappings are
  preserved. Consolidation happens atomically inside the next successful sync.

## 0.5.0-dev-7 - 2026-08-22

- Adds an explicit `Light Plans → Theme Strategy` workspace where SoundSwitch
  Banks 1–4 are named and planned as coherent Lumi Themes.
- Selects one automatic Theme for the complete track, with configurable fallback,
  weight, Rekordbox Track Color affinity and next-track-aware cooldown.
- Adds an Automatic Plan Preview that exposes the selected Bank, Theme and reason;
  manual Theme selection remains an explicit preview override.
- Preserves pre-Theme-Strategy policy behavior and every existing Bank, AutoLoop,
  Static Look and MIDI mapping until the new strategy is deliberately saved.

## 0.5.0-dev-6 - 2026-08-22

- Compiles verified SoundSwitch Static Looks into deterministic phrase and
  whole-track Light Plans using Application Rate, weight, cooldown and color.
- Executes only sparse Static Look state transitions alongside the existing
  exactly-once AutoLoop lane; no continuous SoundSwitch timeline control.
- Shows `No Override` or the selected Static Look in Plan Preview and on Live
  plan segments, with an inspectable active-state assumption in diagnostics.
- Releases Lumi-managed Static Looks when the plan returns to `No Override`, on
  `Off`, or before the virtual MIDI source is stopped.

## 0.5.0-dev-5 - 2026-08-22

### Added

- A dedicated `Integrations → Lighting Outputs → Static Looks` workspace
  with the familiar SoundSwitch 32-slot, four-column layout.
- Named Static Look mappings, guided MIDI Learn, per-slot Toggle tests and
  separate activation/release verification.
- A fixed global MIDI surface on Channel 12, Notes 64–95, isolated from the 128
  bank-specific AutoLoop addresses.

### Changed

- Static Look provider mapping now belongs to Lighting Outputs; Light Plans keeps
  only eligibility and variation rules.
- Existing Static Look mappings and rule references are projected by their MIDI
  address, preserving the two mappings created during the physical POC.

### Safety

- Learning or testing a Static Look never enables automatic execution. A slot is
  eligible only after both activation and release have been confirmed; automatic
  runtime output remains a separate follow-up.

## 0.5.0-dev-4 - 2026-08-22

### Fixed

- SoundSwitch AutoLoop MIDI Learn uses 128 unique `(channel, note)` addresses;
  learning the same numbered button in another bank no longer overwrites an
  earlier bank.
- Runtime scheduling retains the bank identity through the final CoreMIDI
  pulse instead of collapsing back to a shared AutoLoop note.

### Changed

- `Virtual Controller` replaces the former Test Controller label and adds
  per-button Learn and Test actions.
- Guided MIDI Learn sends the selected address and automatically advances to
  the next AutoLoop and bank, while SoundSwitch remains responsible for arming
  and confirming each Map action.
- Banks & AutoLoops exposes a direct Test action for every one of its 128 slots.

## 0.4.0 - 2026-08-18

### Added

- Trusted Rekordbox One Library USB sources met stabiele device-identiteit,
  playlistgerichte synchronisatie, change-impact, trackprovenance en veilige
  library rebuild/back-up.
- Directe, read-only Pro DJ Link-integratie voor CDJ-decks en mixers, inclusief
  device discovery, live transport, masterwissels, BPM, exacte positie,
  Hot Cues en lokale USB-trackmatching.
- Geïsoleerde Ableton Link BPM-relay naar SoundSwitch en een afzonderlijke,
  bounded realtime MIDI-lane voor Bank- en AutoLoop-selectie.
- Persistent launchd-owned engine service, afzonderlijke Dev/RC/Production
  identities en databases, Lumi-branding en een installable Apple Silicon DMG.

### Changed

- Live Decks gebruikt één deterministische transporttijdlijn voor waveform,
  phraseplan en AutoLoop-output, terwijl de UI uitsluitend een read-only
  afgeleide presentatie blijft.
- SoundSwitch ontvangt één AutoLoop-trigger per bevestigde phrase-landing;
  continu afspelen, Link BPM en UI-rendering zijn volledig gescheiden lanes.
- RGB waveforms, Rekordbox beatgrids en cue-markers worden gedeeld door de
  Track Lighting Editor, Local Playback en Live Decks.

### Fixed

- Hot Cues, Beat Jumps, loops, pitchwijzigingen en masterwissels kunnen geen
  oude AutoLoop-deadline, verkeerde phrase of teruglopende Link-tijdlijn meer
  uitvoeren.
- Pro DJ Link status- en positie-jitter kan de waveform, Ableton Link of
  SoundSwitch AutoLoop niet meer terugsturen.
- Een CDJ die tijdens een show opstart kan de Pro DJ Link bridge niet meer
  omver trekken; een echte bridge-restart herstelt geladen decks automatisch.
- UI-appswitching, Library-navigatie en client reconnects beïnvloeden de
  show-kritische engine-, Link- en MIDI-lanes niet meer.

## 0.4.0-dev-48

- Promotes each release channel's engine to a non-privileged per-user
  `SMAppService` LaunchAgent owned and restarted by launchd.
- Preserves the existing channel database, stable CoreMIDI endpoints and
  authenticated loopback discovery while UI Quit/relaunch reuses the exact
  engine process.
- Automatically reconnects the open UI when launchd replaces a crashed engine
  and publishes a new endpoint.
- Replaces the shared show/timing anchor with a clock-only Ableton Link input;
  phrases, AutoLoops, Hot Cues, seeks, lighting operation and show generations
  can no longer request a Link phase correction.
- Keeps Ableton Link and realtime AutoLoop MIDI as parallel, separately owned
  SoundSwitch inputs.

## 0.4.0-dev-47

- Requires a coherent multi-frame `CdjStatus` timeline to show an independently
  detected transport discontinuity before any precise-position cluster can
  re-anchor Ableton Link or replace the active AutoLoop plan. One reordered
  status packet or a normal status beat that merely agrees with a jittering
  position is no longer treated as corroboration.
- Applies the same independent-status gate to imported Hot Cues. Known cue
  targets retain the two-precise-packet confirmation count, but correctness now
  takes precedence over accepting an unverified low-latency jump.
- Adds regressions separating precise-position consensus from independent
  status-jump consensus, including reordered status, age and landing-beat
  bounds.
- Records the invalidated dev-46 short-soak result: a later continuous physical
  run produced seven false Link re-anchors in 45 seconds despite zero late or
  saturated MIDI dispatches.
- Passes a packaged physical three-wrap soak: exactly one discontinuity and
  one hard Link re-anchor per observed CDJ loop wrap across 6,637 exact
  positions, with MIDI p95 `0.1 ms`, zero late dispatches and zero saturation.

## 0.4.0-dev-46

- Corrects precise-position continuity for pitched playback: track-time
  progress is now compared with elapsed time multiplied by the effective/original
  BPM ratio, so packet coalescing cannot turn normal pitched playback into a
  forward seek.
- Requires arbitrary seeks and loop wraps to agree with the independent
  `CdjStatus` absolute beat before advancing Lumi's transport generation.
  Coherent precise-position noise can therefore no longer scrub the waveform,
  Ableton Link or SoundSwitch AutoLoops.
- Keeps imported Hot Cues low latency with a dedicated two-packet confirmation
  path while preserving the generation barrier that cancels old output before
  the landing phrase is applied.
- Ignores position-time jumps that do not move more than two beats, preventing
  sub-phrase jitter from causing show-wide hard re-anchors.

## 0.4.0-dev-45

- Separates macOS presentation recovery from show-critical timing: returning
  to Lumi now restarts the waveform and AutoLoop-plan Core Animation directly
  from the current read-only visual clock without touching engine, Link or
  MIDI state.
- Requires three consecutive modern-player position frames to confirm a new
  transport timeline. Isolated or interleaved precise-position jitter can no
  longer create a seek generation, re-anchor Ableton Link, select a phrase or
  trigger an AutoLoop.
- Keeps real Hot Cues and loop wraps responsive: a stable new timeline is
  confirmed in roughly 60 ms at the CDJ-1500X position update rate, after
  which the old output generation is cancelled before the landing phrase is
  applied.
- Adds regressions for discontinuity consensus, stale-frame interleaving and
  the exact Hot Cue phrase/output barrier.

## 0.4.0-dev-44

- Makes modern-player `PrecisePosition` the only Pro DJ Link authority for
  playback position, phrase selection and automatic lighting output. Lumi maps
  playback milliseconds through the trusted local Rekordbox beat grid.
- Keeps bar-relative Beat packets timing-only. A Beat arriving before the
  matching status after a Hot Cue can no longer combine a new bar position
  with an old absolute track position and select the wrong phrase.
- Holds automatic output fail-closed when exact position authority is missing
  or older than 250 ms. Future Bank and AutoLoop MIDI remains represented as
  guarded deadlines and is released only against a fresh matching transport
  generation.
- Updates Ableton Link tempo directly from playing deck status without
  re-anchoring phase, improving pitch-slider response while preserving a
  monotonic Link timeline.
- Adds provider and scheduler regressions for the exact Hot Cue race and shows
  exact-position readiness in Live and Integration Diagnostics.
- Treats delayed-but-forward precise-position callbacks as receive jitter, not
  seeks. Physical acceptance processed 19,620 exact positions and emitted
  25/25 scheduled pulses with zero late dispatches, saturation or Link errors;
  a Hot Cue A landing emitted only Intro's configured AutoLoop.
- The corrected packaged build classified only the real CDJ loop wraps across
  a further 7,821 exact positions. Its final SoundSwitch soak emitted 26 MIDI
  events with zero late dispatches, cancellations, saturation or Link errors
  and a maximum measured dispatch latency of 153 microseconds.

## 0.4.0-dev-43

- Keeps the channel engine and its `Lumi Virtual MIDI`/`Lumi Clock`
  endpoints alive across ordinary UI Quit and relaunch, preventing CoreMIDI
  device-topology churn while SoundSwitch and Control One are running.
- Moves the show to Off and leaves Ableton Link on every authenticated UI
  disconnect, including an unexpected UI exit; the persistent engine may not
  continue lighting output without a client.
- Reattaches the next UI session to the exact running engine service and
  preserves build-exact replacement for upgrades.
- Bounds Carabiner command, shutdown and child-process waits so a blocked
  helper cannot make the macOS supervisor force-kill the engine and leave a
  ghost Link peer.
- Refreshes integration diagnostics from lightweight runtime snapshots even
  when the Library revision has not changed.
- Adds real-engine disconnect/reattach and bounded child-termination
  regressions. Physical CDJ/SoundSwitch evidence retained zero late or
  saturated AutoLoop dispatches while SoundSwitch remained responsive through
  UI Quit and reattach.

## 0.4.0-dev-42

- Kept the direct Pro DJ Link bridge passive for media content so occupied
  player numbers cannot cause an active metadata retry storm while realtime
  beat and lighting output remain enabled.
- Classifies playing deck-status position changes against elapsed time and
  effective BPM instead of a fixed two-beat jump. Delayed normal progress can
  no longer impersonate a seek, advance the transport generation or re-anchor
  SoundSwitch's running AutoLoop.
- Prevents a late asynchronous deck-status frame from rewinding the canonical
  beat established by a newer precise Beat packet.
- Adds provider-level delayed/out-of-order status regressions and a real-helper
  lifecycle regression proving that dropping Lumi's managed Link output closes
  its owned Carabiner peer without an explicit Stop command.

## 0.4.0-dev-41

- Keeps the running Ableton Link timeline monotonic during ordinary Pro DJ
  Link beat traffic. UDP receive-time jitter is measured but can no longer
  request or force phase corrections on every beat.
- Re-anchors Link exactly once for real musical discontinuities: initial
  start/resume, Hot Cue or seek, track load, and Master handoff. Continuous
  pitch/BPM changes update tempo while preserving phase.
- Adds a regression that injects deliberately conflicting continuous beat
  phases and proves that only the initial discontinuity can emit a Carabiner
  phase command.

## 0.4.0-dev-40

- Keeps a prepared AutoLoop deadline alive when Pro DJ Link reports entry into
  that phrase just before the CoreMIDI deadline; scheduled output is no longer
  mistaken for an already emitted pulse.
- Keeps successive phrase deadlines in one transport generation so preparing
  the next phrase cannot cancel the pulse that is due for the current phrase.
- Uses the isolated MIDI lane's 50 ms post-bank deadline after starts, Hot Cues
  and beatjumps instead of waiting for another Pro DJ Link beat.
- Reschedules prepared output only for an actual BPM change, not for ordinary
  network/bridge arrival jitter at a stable tempo.
- Adds action-specific scheduling and dispatch telemetry, including deadline
  lead, dispatch lateness and a count above the 20 ms show-critical budget.
- Makes the macOS app own its engine and helpers through termination. Closing
  the last window or Quit waits for graceful teardown and force-stops an
  unresponsive engine; an unexpected UI disconnect also ends the app-owned
  service.
- Prevents queued Link anchors from relaunching Carabiner while shutdown is in
  progress and fixes a queue-depth accounting race in realtime diagnostics.
- Gives every app-owned Link helper a free isolated control endpoint inside
  Carabiner's accepted port range, avoiding cross-version ownership and helper
  startup failures on larger macOS ephemeral ports.

## 0.4.0-dev-39

- Pre-schedules an exact Pro DJ Link phrase transition up to four bars ahead
  on the isolated realtime MIDI lane. A short beat-packet or UI interruption
  immediately before the boundary can no longer delay a prepared AutoLoop by
  seconds; seek, Hot Cue, plan and Master generations still cancel stale work.
- Keeps the connected-deck presentation clock monotonic across delayed or
  out-of-order transport polls, while explicit transport revisions still make
  real seeks and Hot Cues jump immediately.
- Disables continuous sizing on the actual root hosting controller and view
  only after the app window exists, removing the dominant macOS 26 layout loop
  measured in the installed Live view.
- Stops publishing an unchanged empty local-waveform cache on every 4 Hz
  connected-deck poll; the same physical two-player Live view fell from about
  74% to 3% CPU while paused and measured 12.4–18.8% during a warm physical
  Player 1 run.
- Makes persistent engine attachment build-exact and adds graceful SIGTERM
  shutdown, preventing old Lumi, Pro DJ Link and Ableton Link helpers from
  surviving into the next Dev version.
- Replaces generic orange Live warnings with the exact affected provider and
  recovery action.

## 0.4.0-dev-38

- Removes the duplicate Hot Cue rows from Live Decks and the Track Editor.
  Rekordbox cue letters and colours now appear only as compact markers above
  their exact waveform position.
- Moves Live waveform, phrase and AutoLoop-plan motion onto Core Animation and
  keeps equivalent visual clocks stable across routine engine polls.
- Removes a macOS hosting-view minimum-size feedback loop that kept SwiftUI
  continuously laying out the application. In the same connected Live state,
  measured UI CPU fell from 74–99% to predominantly below 1%.
- Keeps all visual interpolation outside the dedicated realtime MIDI lane. A
  60-second release soak measured 5.038 ms p95 dispatch latency with no queue
  saturation.

## 0.4.0-dev-37

- Keeps Off, Arm, Start, Pause and Off responsive while high-rate Pro DJ Link
  telemetry advances the global runtime revision. Operation controls now use
  the current operation transition as their concurrency boundary instead of
  rejecting a valid command because a beat or position update arrived first.
- Adds a deterministic regression test that advances deck state between the UI
  snapshot and Arm command and proves that Lumi still enters Armed.

## 0.4.0-dev-36

- Keeps trusted USB media bound to the exact filesystem volume identity instead
  of conflating backup devices by their visible name or cloned database.
- Presents a connected USB's current volume name during inspection so a stale
  trusted label is corrected before the next synchronization persists it.
- Resolves duplicate GRAY/CHRM Rekordbox aliases when every active alias agrees
  on the same canonical Lumi track, while conflicting aliases still fail closed.

## 0.4.0-dev-35

- SoundSwitch Bank and AutoLoop deadlines run on a dedicated, bounded realtime
  MIDI thread instead of the engine polling loop. Generation invalidation
  prevents stale output after seeks, hotcues, master handoffs, Pause or Off.
- Diagnostics exposes realtime queue depth, high-water mark, scheduling counts,
  saturation and p50/p95/p99/max latency; unhealthy timing can no longer leave
  the Live lighting status green.
- Engine snapshots carry a library revision. The macOS client only requests and
  decodes the full library projection when that revision changes.
- The engine accepts authenticated sequential UI connections and the macOS app
  reconnects to the existing channel-specific process after UI relaunch while
  preserving operation state.
- Backup and restore are engine-owned and use SQLite's online backup API,
  integrity/schema validation, atomic staging and a validated rollback copy.
- Dev packaging now includes an SPDX 2.3 SBOM beside the checksum, licence,
  notices and trademark policy.
- A configurable AutoLoop soak test and an enforced one-hour RC entry point
  retain correctness and latency evidence without treating a short Dev run as
  release evidence.

## 0.4.0-dev-34

- Pro DJ Link bridge ingress is bounded to 512 decoded messages. Continuous
  deck status and metadata can be safely coalesced, while exact beats,
  lifecycle events and errors remain ordered and fail closed on saturation.
- Live Deck polling uses a lean snapshot three out of four cycles and refreshes
  library data once per second, reducing the measured p95 snapshot construction
  time from 1.178 ms to 0.221 ms and payload from 50,409 to 20,799 bytes.
- Diagnostics exposes Pro DJ Link queue depth, capacity, high-water mark,
  coalesced traffic and critical saturation count.
- Child Pro DJ Link and Ableton Link helpers no longer inherit the Lumi session
  credential.
- Local functional, technical, security and show/lab gates are explicit and
  GitHub Actions remains reserved for deliberate release verification.
- Jackson is updated to 2.18.9 and Maven dependencies now participate in
  Dependabot and the local OSV security gate.

## 0.4.0-dev-33

- Direct Pro DJ Link publishes an explicit transport revision for every load,
  playback restart and forward/backward seek. The revision remains independent
  from snapshot polling and UI rendering.
- A Master Live Deck automatically resumes waveform follow after an
  authoritative Hot Cue, beatjump or seek, including Pause → Start followed by
  a jump from the track end back to the beginning.
- Core Animation treats a transport revision as a new motion anchor, removing
  the stale end-of-track animation before rendering the new playhead position.
- Deterministic Swift coverage and the real two-host LAN acceptance test now
  prove the complete end → beginning reconciliation path.
- Canonical dry-run evidence now records the deliberate current-phrase
  reassertion introduced by Pause → Start instead of retaining the older
  pre-resume transcript.

## 0.4.0-dev-32

- A real Live Deck `stopped → playing` edge now restores the current planned
  AutoLoop exactly once, including when playback starts from a later Hot Cue;
  repeated playing packets remain idempotent.
- `Pause → Start` now reasserts the current AutoLoop because Pause deliberately
  closes the physical output gate. Lumi no longer stays dark until the next
  phrase after an operational pause.
- Operation buttons acknowledge their requested state immediately while the
  revision-safe engine command completes, with the authoritative response still
  owning success or rollback.
- Live Deck horizontal navigation accumulates from the user's last manual
  viewport instead of repeatedly panning from a stale follow position.
- Hot-cue lines and badges are children of the same Core Animation layer as the
  RGB waveform, so they remain locked to their Rekordbox position during Live
  follow, manual scrolling and zoom.

## 0.4.0-dev-31

- Entering Start while the current Master is already playing now executes its
  current planned phrase exactly once. Lumi no longer waits silently for the
  next phrase change; an unprepared direct-deck target settles its Bank and
  emits on the first safe exact beat.
- Lighting Output Offset now uses the natural signed convention: a negative
  value sends early, zero targets the phrase boundary and a positive value
  sends late. Existing Dev preferences are migrated once so their physical
  compensation remains unchanged.
- Direct Pro DJ Link predicts negative offsets from exact future beat packets;
  positive offsets and Bank settling use bounded non-blocking timers. Pending
  changes participate in the very next phrase transition without moving
  timing work into SwiftUI.
- Canonical dry-run evidence now includes the deliberate current-phrase output
  produced when Start is entered during active playback.

## 0.4.0-dev-30

- Direct Pro DJ Link now accepts the exact empty-deck sentinel emitted by a
  physical CDJ-1500X (`beatNumber -1`, beat zero and placeholder BPM 655.35)
  without classifying it as corrupt protocol data or restarting the bridge.
- Pro DJ Link bridge failures retain their actionable cause across automatic
  recovery attempts and include the helper's latest diagnostic line. The
  status clears only after the direct deck source is genuinely ready.
- Restoring a stable direct deck source also restores the authoritative timing
  anchors consumed by Ableton Link; no simulator or Beat Link Trigger path is
  involved.

## 0.4.0-dev-29

- Manual horizontal navigation in Local Playback now suspends automatic Live
  waveform follow instead of being immediately overwritten by it. Navigation
  starts from the currently rendered track position and normal follow resumes
  on the next playback start.
- Hot-cue letters use one shared, subtly smaller typography token in the Track
  Editor, Local Playback and Live Decks while retaining their existing hit area.

## 0.4.0-dev-28

- Hot-cue controls now show only the authoritative Rekordbox letter and color
  in the Track Editor, Local Playback and Live Decks.
- Cue names and loop metadata remain preserved in the model, but Lumi no
  longer invents or exposes descriptive labels in the compact cue strip.

## 0.4.0-dev-27

- Trusted OneLibrary USB sync now imports current Rekordbox hot-cue points,
  including the point encoding used by current exports, with their letter,
  timestamp, name and RGB color.
- Hot cues have independent source provenance and can enrich or refresh a
  matched track without promoting or replacing its protected beatgrid,
  waveform, Lumi phrase timeline or lighting configuration.
- Schema 11 migrates existing Dev libraries in place. A real GRAY playlist
  sync verified two cues for `90s Bitch`, while timeline revision 35 and all
  17 authored phrase points remained unchanged.

## 0.4.0-dev-26

- The DMG Finder shortcut now targets the channel-specific installation
  directory instead of the Applications root: Production uses `Lumi`, RC uses
  `Lumi/RC` and Dev uses `Lumi/Dev`.
- Package verification rejects a disk image whose drag target does not match
  its release channel.

## 0.4.0-dev-25

- Removes the embedded rounded outline from the Lumi app-icon artwork so the
  macOS-owned icon shape no longer produces an inner/double border. The
  approved RGB waveform/light geometry stays optically centered and unchanged.

## 0.4.0-dev-24

- Rekordbox hot cues are parsed read-only from trusted USB analysis, persisted
  provider-neutrally and shown with their original letter, name, loop state
  and RGB color in Track Editor, Local Playback and Live Decks.
- Existing Dev libraries retain all Lumi-authored phrases and lighting
  configuration; their next trusted-USB sync enriches matched tracks with cue
  data automatically.
- Identically named playlists from primary and backup USB media are presented
  as one canonical Library playlist with a deduplicated track union, while
  both independent source relationships remain available for sync status.
- The colored waveform/light mark is optically centered within the unchanged
  app-icon border and all macOS icon/navigation renditions are regenerated.
- Resume/seek lighting and exact Pro DJ Link AutoLoop scheduling regressions
  remain part of the local verification gate; no UI clock enters the realtime
  output path.

## 0.4.0-dev-23

- Direct Pro DJ Link Beat packets now drive exact Lumi phrase activation after
  the matched USB/library metadata has hydrated the deck. Forward hotcue and
  beat-jump discontinuities are classified as seeks and immediately resolve
  the phrase at the landing beat.
- Automatic SoundSwitch output no longer blocks the engine for the 50 ms Bank
  settle interval. Lumi pre-arms the next Bank shortly before a planned phrase
  using the settle window plus one engine-tick safety margin, then sends the
  AutoLoop pulse on the authoritative phrase boundary.
- An unprepared Bank after a hotcue or beat jump fails predictably to the first
  safe exact beat after settling; stale deck, track, plan and phrase requests
  are cancelled before MIDI can be emitted.
- Integrations diagnostics expose bounded requested, pre-armed, emitted,
  cancelled and beat-fallback counters for the realtime AutoLoop scheduler.

## 0.4.0-dev-22

- Production, RC and Dev installations now have explicit display names,
  bundle identifiers and documented `/Applications/Lumi` channel locations so
  all three can coexist without sharing databases or preferences.
- Unsigned local DMGs consistently ad-hoc sign every embedded Mach-O member,
  including Xcode 26 debug libraries, and omit incompatible hardened-runtime
  library validation. Packaged Dev apps therefore launch normally instead of
  showing a generic macOS compatibility error.
- Dev and RC artifacts include their channel and full version in the app name;
  Production remains the stable `Lumi.app`.

## 0.4.0-dev-21

- Direct Pro DJ Link is now pumped by a dedicated 20 ms engine cadence instead
  of SwiftUI's 250 ms snapshot polling. Deck timing therefore continues while
  the UI is hidden, busy or not requesting state.
- Only beat-exact Pro DJ Link Beat packets steer advancing Ableton Link phase;
  asynchronous deck-status frames retain metadata and stopped BPM/transport
  recovery without impersonating beat boundaries.
- Stale timing and a failed bridge now hold Link transport fail-closed. A fresh
  authoritative anchor or automatically restarted bridge recovers the same
  session without restarting Lumi or emitting a lighting burst.
- Bounded session metrics expose received/applied/coalesced anchors, hard and
  soft corrections, phase-error maximums, fail-closed holds, provider failures
  and realtime engine-lane starvation in Integrations Diagnostics.
- Pause now holds Link transport immediately, while the master deck's next
  precise anchor continues to keep BPM and phase current with stopped output.

## 0.4.0-dev-20

- An enabled Ableton Link session now continues to follow the selected Live
  master deck's effective BPM and four-beat phase while Lumi lighting is
  `Off` or `Pause`. Only Link transport is held, so SoundSwitch no longer sees
  a connected peer that remains at an unrelated/default BPM.
- Local Playback uses the same rule: lighting operation state can close or
  pause output without making the active musical tempo authority disappear.
- Regression coverage verifies that `Off` and `Pause` retain master BPM, deck
  identity and bar phase while publishing stopped transport.

## 0.4.0-dev-19

- Ableton Link is now an explicit user-controlled integration with its own
  `Integrations > Ableton Link` workspace, live state, peer count, timing
  source and optional remembered app-start preference. The safe default stays
  Off.
- Live exposes a compact Link on/off control with the authoritative BPM when
  timing is active. Link lifecycle is independent from the lighting
  `Off`/`Arm`/`Start`/`Pause` state; disabling Link leaves the shared session
  without stopping SoundSwitch.
- Live system status is consolidated to Pro DJ Link, Light Output and Ableton
  Link. An intentionally unused provider or an empty deck is informational;
  only an operational failure produces `Attention`. A competing Lumi version
  now yields an actionable Light Output message instead of a CoreMIDI error.
- The macOS bundle now explains its Local Network permission before using Pro
  DJ Link discovery or Ableton Link peer discovery.
- The helper self-test remains side-effect free and is unavailable while Link
  is enabled.

## 0.4.0-dev-18

- Pro DJ Link start niet langer tijdens appstart of Local Playback. Lumi doet
  pas na de expliciete keuze voor Live Decks mee op het DJ-netwerk en stopt de
  bridge bij het verlaten daarvan.
- Een harde preflight op de vaste Pro DJ Link UDP-poorten blokkeert Live Decks
  wanneer Rekordbox of andere DJ Link-software dezelfde Mac gebruikt. Local
  Playback en de geladen sessie blijven daarbij intact en de UI krijgt een
  concrete herstelmelding.
- De beheerde Ableton Link-route is fysiek met SoundSwitch als echte peer
  gevalideerd voor 130 → 140 BPM, beat/phase, start/stop en hold zonder BLT.
- Ableton Link neemt niet deel bij appstart of Off. De helper start pas bij een
  geldige actieve timingbron en stopt volledig bij Off, zodat een idle
  standaardtempo SoundSwitch niet kan veranderen.
- Diagnostics bevat een side-effectvrije `Test Ableton Link Helper`-actie.
  Deze is alleen toegestaan wanneer Lumi op Off staat en valideert executable
  en gepinde versie zonder een Link-peer of lichtcommando aan te maken.

## 0.4.0-dev-17

- Een opnieuw gekoppelde trusted USB behoudt voortaan één stabiele identiteit,
  ook wanneer een Library Rebuild nog een oude `reset-pending` bronregistratie
  heeft achtergelaten. Dubbele USB-labels verdwijnen en Pro DJ Link-resolutie
  blijft ondubbelzinnig.
- Fysieke CDJ-1500X-spelers mogen bij `NO_TRACK` hun eigen playernummer blijven
  publiceren. Deze geldige overgangsstatus stopt de lokale Lumi-engine niet
  langer; engine-exitdetails worden bovendien zichtbaar in de lokale logs.

## 0.4.0-dev-16

- Lumi publiceert beat-, bar- en BPM-timing vanuit Local Playback of de directe
  Pro DJ Link-adapter naar SoundSwitch via een beheerde Ableton Link-helper;
  Beat Link Trigger is geen runtime-afhankelijkheid.
- Ableton Link-timing, Lumi Virtual MIDI AutoLoop-selectie en Control One
  handmatige bediening zijn expliciet drie parallelle SoundSwitch-inputs.
- De timing-worker start asynchroon, coalescet achterstallige anchors, herstelt
  een verbroken helperverbinding zelfstandig en valideert de gedeelde monotone
  klok voordat exacte Pro DJ Link-timestamps worden gebruikt.
- Integrations en Live Tech tonen de Link-provider, timing authority, BPM,
  peers, phase error en eventuele degradatie zonder de realtime outputlane te
  blokkeren.
- Lange USB-playlistnamen blijven compact en zijn via hover volledig leesbaar.
- De command-ID-cache wordt nu ook in geoptimaliseerde release-builds gevuld;
  een retry met hetzelfde ID kan daardoor geen reeds toegepaste planmutatie
  opnieuw uitvoeren of als onterechte revision conflict terugkomen.

## 0.4.0-dev-15

- `Settings > Data & Backups` maakt complete, kanaalgescheiden
  `.lumibackup`-packages en kan ze na een automatische safety backup
  terugzetten.
- `Rebuild Library Content` toont vooraf exacte impact, bewaart gekozen
  authored tracks direct en ruimt oude tracks, playlists, mirrors en
  syncgeschiedenis daarna transactioneel op.
- Creative Archive bewaart Lumi-owned phrasewerk onafhankelijk van USB- en
  playlistindeling. Een latere USB-sync koppelt een exacte, beat-compatibele
  track automatisch terug; ambiguïteit en afwijkende beatstructuren blijven
  veilig `pending` of `review`.
- Phrase Model-defaultsupgrades slaan hun versienummer nu werkelijk op; een
  app- of backup-restart verhoogt de catalogusrevisie daardoor niet langer
  opnieuw.
- De gebundelde Pro DJ Link JAR en Java-runtime staan als verzegelde app-
  resources in plaats van ongeldige losse Helper-bundles; de ad-hoc gesigneerde
  Dev-DMG doorloopt daardoor weer de volledige mount- en signaturecheck.

- USB-playlists tonen altijd een groene teller met het exacte aantal `CURRENT`
  tracks. Gesynchroniseerde playlistnamen en aantallen blijven per trusted USB
  offline zichtbaar; oudere full-device syncs worden expliciet als legacy
  zonder opgeslagen playlistnamen aangeduid.
- Exact unieke titel/artiest/BPM/duur-matches herstellen USB-tracks die eerder
  ten onrechte als tweede canonieke identiteit werden geïmporteerd. Alleen
  onaangepaste, automatisch aangemaakte en volledig ongerefereerde duplicaten
  worden tijdens een volgende sync atomair opgeruimd; Lumi-edits blijven staan.
- Library-playlists tonen hun USB-bron of legacy-herkomst, zodat gelijknamige
  oude Rekordbox- en actuele USB-playlists niet meer te verwarren zijn.

- USB Sync toont vanaf het moment van klikken in dezelfde uitgeklapte source-
  lane een geblokkeerde `Synchronizing…`-actie, indeterminate progress en daarna
  een blijvend succes- of foutresultaat; feedback verdwijnt niet meer buiten
  beeld boven de bronnenlijst.
- `NEW` tracks uit geselecteerde OneLibrary-playlists worden nu atomair als
  canonieke Lumi-tracks geïmporteerd, inclusief beatgrid, RGB-waveform, phrases,
  playlistrelatie en USB/Pro DJ Link-identiteit. Een succesvolle Sync maakt ze
  daardoor direct `CURRENT` in plaats van ze alleen als unmatched te bewaren.
- Trusted USB sources klappen voortaan direct onder hun eigen lane uit. Een
  playlistselectie toont vóór Sync automatisch de read-only impact op unieke
  tracks: nieuw, te vernieuwen, actueel, beschermd en te beoordelen.
- Read-only Rekordbox Device Library-sync koppelt echte USB/SD-track-ID's aan
  canonieke Lumi-tracks en ververst beatgrid, RGB-waveform en cue-bearing
  analyserevisies; BLT MIDI v4 ondersteunt daarnaast exacte Shallow Simulator-
  matching zonder BLT zelf te wijzigen.
- BLT-transport is begrensd tot gewijzigde state, 100 ms-positieframes en een
  heartbeat; connected decks tonen daarmee een vloeiend geïnterpoleerde
  playhead zonder de engine-commandlane met identieke frames te overspoelen.
  Een ontbrekende heartbeat ruimt per deck na 2,5 seconde veilige stale
  transportstate op.
- Live timing changes remain pending until the next actually playing phrase;
  applied and pending values are visible in Live and on both decks.
- Het goedgekeurde RGB-waveform/light-fan-logo is toegevoegd als macOS-app-icon
  en blijvend navigatiemerk, ook bij ingeklapte navigatie.
- Deterministische pause/cue/play-outputreconciliatie blijft gepland.

## 0.3.0

- Rekordbox-backed Library en Lumi-owned phrase editing.
- Stabiele Local Playback dual-deck met rolling AutoLoop Plan.
- Persistente vier-bank/32-slot SoundSwitch-mapping en generieke MIDI-output.
- Automatische phrase-boundary execution, MIDI Clock en een eerste fysieke
  SoundSwitch/Control One/DMX-run.
- Fysiek geaccepteerde SoundSwitch/Control One/DMX-keten en een geïsoleerde,
  lokaal geverifieerde Stable-DMG voor Apple Silicon.
- EPL-2.0-projectlicentie en compacte branding- en integratienotices.

## 0.1.0-dev

- Epic 1 is als reproduceerbare, volledig lokale vertical slice gehard met een
  canoniek end-to-endscenario en golden release-evidence.
- De releasegate valideert locked Rust dependencies, warnings-as-errors voor
  Rust en Swift, een release-performancecheck en dependencyrichting.
- Faulttests dekken malformed input, queue overload, stale revisions en
  proces-/transportverlies zonder ongecontroleerde lighting output.

## 0.0.8-dev

- De demo-simulator is volledig vanuit de macOS-app te bedienen: laden,
  snelheid, afspelen/pauzeren, deckwissel en reset.
- OFF, ARMED, LIVE en PAUSED zijn gekoppeld aan versioned engine commands met
  revision checks en geldige transities.
- De app toont begrensde, geordende runtime-, bron-, planner- en output-events
  met expliciet resultaat en reden.

Alle relevante wijzigingen aan Lumi worden in dit bestand bijgehouden.

Het formaat is gebaseerd op [Keep a Changelog](https://keepachangelog.com/) en
Lumi gebruikt [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Archive-safe Rekordbox XML Apply Sync met hash-gebonden diff, stabiele
  source-identiteiten, playlistmirror en reversibele archive/restore, plus een
  bounded read-only ANLZ POC-parser voor beatgrid, PSSI en RGB/three-band
  waveforms vanuit een Lumi-owned snapshot.
- Een bounded, read-only Rekordbox XML engine-adapter en native `Preview Sync`
  die gevolgde folders/playlists normaliseert, gedeelde tracks dedupliceert,
  source capabilities en SHA-256-identiteit toont en nog geen librarydata
  schrijft.
- Een ingebouwd `SoundSwitch Autoloops` Output Profile met vier benoemde banks,
  32 stabiele AutoLoop-posities per bank, vier pagina's van acht fysieke
  buttons, exacte bewerkbare AutoLoop Names,
  configureerbare Phrase Types, een gespiegelde Virtual Controller en een
  expliciete MIDI/POC-readinessweergave op basis van demo-data.
- ADR-0015 en een timeboxed CoreMIDI/SoundSwitch-POC-plan dat parallelle Control
  One-bediening en zichtbare DMX-output via Control One als harde acceptatie
  vastlegt.
- Een vaste donkere, CDJ-geïnspireerde Track Lighting Editor met maat/beatgrid,
  gekleurde performance-waveform, gekleurde phrase lane, full-track overview en
  één gedeeld beatcoördinatenstelsel.
- Geïsoleerde, read-only lokale audiopreview met play/pause/stop, scrubben,
  maatnavigatie, volume, selected-phrase-loop, toetsenbordbediening en veilige
  cleanup zonder showstate of bronbestand te muteren.
- Een native, gepagineerde Library-workspace met Collection- en playlistnavigatie,
  server-side search, expliciete readiness, metadata/provenance-inspector en een
  deep-link naar de Track Lighting Editor.
- Een begrensde library-query over de lokale engineverbinding en visuele evidence
  voor empty, importing, ready, stale, degraded, conflict en error states.
- Provider-neutrale music-librarycontracten, stabiele bron- en trackidentiteiten,
  playlists en gereviseerde Lumi phrase-timelines voor Epic 2A.
- Lokale SQLite-persistence met transactionele migraties, rollback-bewijs,
  optimistische concurrency en begrensde track-, playlist- en historyqueries.
- Een expliciete offline demoprovider met synthetische metadata, kleuren,
  beatgrids, waveforms, phrases, playlists en procedureel PCM-audio, plus een
  10.000-track schaalfixture.
- Initiële functionele en technische architectuurbaseline.
- Eerste Rust-workspace, native macOS-target en reproduceerbare
  foundationverificatie voor Epic 1.
- Transportonafhankelijk protocol v1-contract met gedeelde Rust/Swift-fixtures,
  begrensde decoding, commandidempotentie en sequence-gapdetectie.
- App-scoped Rust-engine met geauthenticeerde loopbackverbinding, native
  process supervision en zichtbare healthstatus in de macOS-app.
- Native Lumi Design System met semantische tokens, herbruikbare componenten,
  persistente dark/light/system-appearance en Camelot/Classic-keynotatie.
- Deterministische domeinkern met sterke runtime-identiteiten, plan- en
  track-loadrevisions, monotone ordering, single-writer reducer en begrensde
  eventingress met expliciet veilig overloadgedrag.
- Provider-neutrale deck-sourcepoort en deterministische tweedecksimulator met
  canonieke track-, beat- en phrasefixtures, versnelde klok en golden transcript.
- De macOS-app toont Live en Next vanuit de echte enginesnapshot, inclusief
  BPM en configureerbare Camelot-/klassieke keynotatie.
- Deterministische next-trackplanner met geïnjecteerde keuzebron, minimale
  phrasecompatibele scene-catalogus, machineleesbare redenen en veilige fallback.
- Het echte vooraf berekende Next-plan is in de macOS-app zichtbaar met theme,
  scene, loop, revision en de reden achter iedere automatische keuze.
- Theme- en scenekeuze, cue-locking en regenerate werken vanuit de inspector via
  revision-aware commands, inclusief conflict refresh en headless UI-bewijs.
- Provider-onafhankelijke phrase-execution met een dubbel gevalideerde
  operationele outputgate en een deterministische dry-run-adapter.

### Changed

- De ontwikkelbranch gebruikt een expliciete SemVer pre-releaseversie, gestart
  op `0.0.1-dev`.
- De volgende functionele bouwstap gebruikt ontwikkelversie `0.0.2-dev`.
- De simulator vertical slice gebruikt ontwikkelversie `0.0.3-dev`.
- De deterministische planner vertical slice gebruikt ontwikkelversie
  `0.0.4-dev`.
- De interactieve next-plan vertical slice gebruikt ontwikkelversie
  `0.0.6-dev`.
- De dry-run execution vertical slice gebruikt ontwikkelversie `0.0.7-dev`.

### Fixed

- Interactieve appcommando's wachten nu op de lokale engineverbinding en krijgen
  voorrang op simulatie-ticks, zodat Library-queries en showcontrols niet meer
  stil kunnen worden overgeslagen.
- `Clear search`, de vaste Collection-teller en de minimale Library-venstermaat
  blijven correct tijdens playlist- en zoeknavigatie.
- Xcode herschrijft de handmatig beheerde localization catalog niet meer tijdens
  een gewone debugrun.
- Wisselen van Light naar System volgt macOS nu zonder donkere content met
  onleesbare light-mode voorgrondkleuren achter te laten.
