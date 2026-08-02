# Lumi – releasechecklist

Gebruik deze checklist voor iedere productie- of hotfixrelease.

## Voorbereiding

- [ ] Releaseversie en type (`MAJOR`, `MINOR`, `PATCH`) bepaald
- [ ] Scope bevroren
- [ ] `dev` groen en up-to-date
- [ ] Releasebranch vanaf juiste commit gemaakt
- [ ] `VERSION`, Cargo- en Xcodeversies gelijk
- [ ] `CHANGELOG.md` bijgewerkt
- [ ] Configuratie-/protocolmigraties beschreven
- [ ] Bekende beperkingen beschreven

## Validatie

- [ ] Rust format, lint en tests groen
- [ ] Swift format, lint en tests groen
- [ ] Planner- en state-machine-tests groen
- [ ] Simulator end-to-endscenario's groen
- [ ] macOS unsigned build groen
- [ ] iOS Simulator-build groen
- [ ] LaunchAgent- en restartsmoketest groen
- [ ] Protocolcompatibiliteit groen
- [ ] Geen secrets of ongewenste binaries in repository

## Release

- [ ] Release-PR naar `main` gemergd
- [ ] Tag `vX.Y.Z` wijst naar juiste `main`-commit
- [ ] macOS-arm64 build gereed
- [ ] Codesigning geldig
- [ ] Notarization geslaagd en ticket gestapled
- [ ] DMG-installatie en eerste start getest
- [ ] SHA-256-checksum gemaakt en gecontroleerd
- [ ] SBOM gemaakt
- [ ] iOS archive gevalideerd en geüpload
- [ ] Exacte iOS-build via TestFlight getest
- [ ] Draft GitHub Release compleet

## Publicatie en nazorg

- [ ] GitHub Release gepubliceerd
- [ ] iOS naar App Review gestuurd
- [ ] Phased release ingesteld voor iOS-update
- [ ] `main` teruggesynchroniseerd naar `dev`
- [ ] Releasebranch verwijderd
- [ ] Installatie-/upgradepad gemonitord
- [ ] Rollbackartefact en patchprocedure beschikbaar
