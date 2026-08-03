# Changelog

Alle relevante wijzigingen aan Lumi worden in dit bestand bijgehouden.

Het formaat is gebaseerd op [Keep a Changelog](https://keepachangelog.com/) en
Lumi gebruikt [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

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
