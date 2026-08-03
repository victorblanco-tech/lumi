# Changelog

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

- Wisselen van Light naar System volgt macOS nu zonder donkere content met
  onleesbare light-mode voorgrondkleuren achter te laten.
