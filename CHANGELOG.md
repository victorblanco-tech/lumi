# Changelog

## 0.4.0-dev

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
