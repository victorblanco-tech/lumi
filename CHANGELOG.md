# Changelog

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
  configureerbare Phrase Types, een gespiegelde Test Controller en een
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
