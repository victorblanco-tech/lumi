# Lumi – release- en deploymentplan

Status: **Accepted baseline**
Datum: **2026-08-02**

## 1. Doel

Dit plan beschrijft hoe Lumi voorspelbaar van ontwikkeling naar productie gaat
voor:

- de Rust Lumi Engine;
- Lumi voor macOS;
- Lumi Remote voor iPhone;
- configuratieschemas en het lokale wire protocol;
- documentatie en release-assets.

De releaseketen mag geen runtime-internetdependency introduceren. GitHub en Apple
worden alleen gebruikt voor bouwen, testen, ondertekenen en distribueren.

## 2. Uitgangspunten

1. `main` representeert de productieversie.
2. `dev` representeert de eerstvolgende geïntegreerde ontwikkelstaat.
3. Productiereleases zijn bewust en worden niet automatisch door iedere merge
   naar `main` gepubliceerd.
4. Een Git-tag identificeert exact één onveranderlijke releasecommit.
5. Dezelfde bronrevision bouwt alle platformartefacten van een release.
6. Versienummers zijn platformoverstijgend; buildnummers zijn platform-/CI-
   specifiek.
7. Release-assets worden opnieuw opgebouwd door CI, niet handmatig op een
   ontwikkelmachine samengesteld.
8. Deployment naar gebruikers blijft gescheiden van het creëren van binaries.
9. Een release kan worden tegengehouden zonder geschiedenis of tags te
   herschrijven.

## 3. Branchmodel

```text
feature/* ──squash PR──> dev ──release PR──> main ──tag──> vX.Y.Z
                           ^                       |
                           └──── sync main ────────┘

main ──> hotfix/vX.Y.Z ──PR──> main ──tag──> vX.Y.Z
                                  |
                                  └── sync naar dev
```

### 3.1 `main`

- bevat uitsluitend releasewaardige code;
- iedere gepubliceerde productieversie is een tag op `main`;
- accepteert alleen PR's vanaf `dev` of `hotfix/*`;
- geen force-push of branch deletion;
- geen directe normale development;
- release-PR's gebruiken een merge commit om ancestry met `dev` te behouden.

`main` kan kort een geteste releasecommit bevatten voordat de bijbehorende draft
release wordt gepubliceerd. Publicatie blijft een afzonderlijke gecontroleerde
stap.

### 3.2 `dev`

- standaardbranch op GitHub;
- integratiepunt voor feature- en fixwerk;
- moet steeds compileerbaar en testbaar blijven;
- feature-PR's worden gesquasht;
- force-push en branch deletion zijn niet toegestaan;
- directe pushes worden tijdens de vroege solo-fase technisch toegestaan, maar
  PR's blijven de standaardwerkwijze;
- zodra de volledige CI bestaat, wordt ook voor `dev` een geslaagde PR-check
  verplicht.

### 3.3 Werkbranches

| Type | Startpunt | Doelbranch | Gebruik |
|---|---|---|---|
| `feature/*` | `dev` | `dev` | Nieuwe functionaliteit |
| `fix/*` | `dev` | `dev` | Normale bugfix |
| `chore/*` | `dev` | `dev` | Onderhoud en tooling |
| `release/vX.Y.Z` | `dev` | eerst `dev` | Versie/changelog voorbereiden |
| `hotfix/vX.Y.Z` | `main` | `main` | Urgente productiefix |

Codex-branches mogen de afgesproken `codex/`-prefix gebruiken vóór het type,
bijvoorbeeld `codex/feature/lighting-plan-store`.

## 4. Mergebeleid

### Naar `dev`

- standaard: squash merge;
- PR-titel volgt Conventional Commits en wordt het commitbericht;
- branch wordt na merge verwijderd;
- rebase merge wordt niet gebruikt.

### Naar `main`

- alleen release- of hotfix-PR;
- normale merge commit;
- geen squash, zodat `main` en `dev` ancestry blijven delen;
- alle releasechecks moeten slagen;
- na release volgt een sync-PR van `main` naar `dev`.

## 5. Versienummering

Lumi volgt Semantic Versioning:

```text
MAJOR.MINOR.PATCH
```

- `MAJOR`: incompatibele wijzigingen in configuratie, wire protocol, projectdata
  of ondersteunde bediening waarvoor migratie nodig is;
- `MINOR`: backward-compatible functionaliteit;
- `PATCH`: backward-compatible bug- en stabiliteitsfixes.

Tijdens de pre-1.0-fase:

- `0.MINOR.0` voor een nieuwe MVP-capability;
- `0.MINOR.PATCH` voor fixes binnen die capability;
- `1.0.0` zodra de liveketen, upgrade-/migratieflow en publieke contracten
  stabiel genoeg zijn voor duurzaam gebruik.

### 5.1 Initiële versie

De repository start op `0.0.0`: er is nog geen uitvoerbaar product uitgebracht.
De eerste bruikbare simulator-MVP wordt naar verwachting `0.1.0`.

### 5.2 Canonieke versie

Het rootbestand `VERSION` is de canonieke marketingversie. Een releasevalidatie
controleert later automatisch dat deze gelijk is aan:

- de Rust workspace/packageversie;
- macOS `MARKETING_VERSION` / `CFBundleShortVersionString`;
- iOS `MARKETING_VERSION` / `CFBundleShortVersionString`;
- documentatie- en protocolversieverwijzingen waar van toepassing;
- de Git-tag zonder `v`-prefix.

### 5.3 Buildnummers

Buildnummers zijn monotonisch en veranderen bij iedere CI-build:

- Apple `CFBundleVersion`: afgeleid van de CI-run plus retry-attempt;
- artefactnaam: versie, channel, korte commit-SHA en buildnummer;
- GitHub developmentbuild: `0.1.0-dev.<run>+<sha>` als display-/artefactversie;
- release candidate: `0.1.0-rc.<n>` in GitHub/artefactnamen, met Apple-compatible
  numerieke marketing- en buildversies in de appbundle.

Een buildnummer is nooit een vervanging voor de productversie.

## 6. Commit- en changelogbeleid

Conventional Commits vormen de invoer voor release notes:

| Type | Releasecategorie |
|---|---|
| `feat` | Features |
| `fix` | Fixes |
| `perf` | Performance |
| `docs` | Documentation |
| `build`, `ci` | Build & delivery |
| `refactor`, `test`, `chore` | Internal changes |

De release-impact wordt bewust gekozen; Lumi publiceert niet automatisch een
versie uitsluitend op basis van commitberichten.

`CHANGELOG.md` bevat een `Unreleased`-sectie. Tijdens releasevoorbereiding wordt
die omgezet naar `[X.Y.Z] - YYYY-MM-DD` en wordt een nieuwe lege `Unreleased`-
sectie toegevoegd.

GitHub-generated release notes vullen dit aan met gekoppelde PR's en auteurs.

## 7. Delivery channels

| Channel | Bron | Doel | Automatisch? |
|---|---|---|---:|
| PR validation | iedere PR | checks en unsigned testbuilds | ja |
| Development | `dev` | tijdelijke CI-artefacten | ja |
| Internal beta | handmatige run op `dev`/RC | TestFlight internal + macOS beta | bewust gestart |
| Release candidate | releasecommit | signed kandidaten en acceptatietest | bewust gestart |
| Production | tag `vX.Y.Z` op `main` | GitHub Release + App Store | na goedkeuring |

## 8. CI-pipeline

De concrete workflows worden toegevoegd zodra de betreffende projecttargets
bestaan. De beoogde checks zijn:

### 8.1 Snelle PR-checks

- repository- en versieschemavalidatie;
- Markdown- en linkcontrole;
- Rust format, lint en unit tests;
- Swift format/lint en unit tests;
- configuratieschema- en fixturetests;
- wire-protocolcompatibiliteit;
- deterministic planner- en state-machine-tests;
- build van de Rust-engine;
- unsigned macOS-build;
- iOS Simulator-build;
- dependency- en secret scanning.

### 8.2 Integratiechecks

- simulator end-to-endscenario's;
- restart/state-recoverytests;
- source-adaptercontracttests;
- MIDI dry-run snapshots;
- verpakking van engine in de macOS-app;
- installatie-/LaunchAgent-smoketest op een macOS-runner;
- reconnect- en stale-commandtests voor de iPhone-client.

### 8.3 Releasechecks

- alle PR- en integratiechecks;
- `VERSION` komt exact overeen met de gevraagde tag;
- tagcommit bevindt zich op `main`;
- releaseversie bestaat nog niet;
- clean, reproduceerbare releasebuild;
- codesigningvalidatie;
- notarization en stapling van macOS-artefacten;
- Apple archive validation;
- installatie-/launchtest van het uiteindelijke DMG;
- SHA-256-checksums;
- SBOM en dependencyoverzicht;
- changelog en release notes aanwezig.

## 9. Releasevoorbereiding

### Stap 1 – Scope bevriezen

- bepaal `MAJOR`, `MINOR` of `PATCH`;
- controleer dat `dev` groen is;
- stel functionele scope en bekende beperkingen vast;
- stop nieuwe featuremerges tot de releasebranch gereed is.

### Stap 2 – Releasebranch

Maak `release/vX.Y.Z` vanaf `dev` en wijzig uitsluitend releasegerelateerde
zaken:

- `VERSION`;
- Cargo- en Xcodeversies;
- changelog;
- migratie-/compatibiliteitsnotities;
- release notes;
- eventuele laatste release-only fixes.

### Stap 3 – Release preparation PR naar `dev`

- alle checks moeten slagen;
- genereer unsigned kandidaten;
- voer simulator- en regressietests uit;
- merge de voorbereiding terug naar `dev`.

### Stap 4 – Release PR `dev` naar `main`

- PR-titel: `release: vX.Y.Z`;
- alleen merge als versie, changelog en checks kloppen;
- normale merge commit;
- geen publicatie tijdens de merge zelf.

### Stap 5 – Release workflow

Start bewust de releaseworkflow op de mergecommit:

1. valideer `main` en `VERSION`;
2. maak de beschermde tag `vX.Y.Z`;
3. bouw alle platformartefacten vanaf die tag;
4. sign en notarize macOS;
5. upload iOS naar App Store Connect/TestFlight;
6. maak een draft GitHub Release;
7. voeg DMG, checksums, SBOM en release notes toe;
8. voer final smoke tests uit;
9. publiceer GitHub Release na expliciete goedkeuring;
10. submit iOS afzonderlijk naar App Review.

### Stap 6 – Terugsynchroniseren

Merge `main` terug naar `dev` zodat release- en taghistorie gemeenschappelijk
blijven. Verwijder de releasebranch na succesvolle synchronisatie.

## 10. macOS deployment

De macOS-productieartefacten zijn minimaal:

```text
Lumi-X.Y.Z-arm64.dmg
Lumi-X.Y.Z-arm64.dmg.sha256
Lumi-X.Y.Z-sbom.spdx.json
```

Releasebuilds worden:

- gebouwd op een schone macOS-runner;
- ondertekend met Developer ID Application;
- voorzien van Hardened Runtime en minimaal benodigde entitlements;
- genotariseerd met Apple's notary service;
- voorzien van een gestapled notarization ticket;
- gevalideerd met `codesign`, `spctl` en installatie-/launchsmoketests.

De DMG wordt aan de draft GitHub Release gekoppeld. Publicatie van de release is
de productie-deployment voor macOS. Automatische updates zijn geen MVP-
dependency; gebruikers kunnen een vorige DMG blijven installeren.

## 11. iPhone deployment

### Development en beta

- lokale Xcode-builds voor snelle ontwikkeling;
- iOS Simulator in PR-CI;
- echte devicebuilds via TestFlight Internal;
- externe TestFlightgroep pas wanneer pairing en demomodus reviewbaar zijn.

### Productie

1. Upload de releasebuild naar App Store Connect.
2. Wacht op Apple processing en controleer waarschuwingen.
3. Test exact die build via TestFlight.
4. Koppel screenshots, privacygegevens, review notes en demoflow.
5. Submit voor App Review.
6. Gebruik voor updates standaard phased release.
7. Monitor crashes, pairingproblemen en lokale-netwerkproblemen.
8. Pauzeer de phased release bij serieuze regressies.

De GitHub Release en iOS App Store-versie gebruiken hetzelfde SemVer-nummer,
maar kunnen op verschillende momenten publiek beschikbaar worden door App
Review.

## 12. Secrets en signing

Secrets worden nooit in de repository of release-assets opgeslagen.

Benodigd in een latere CI-fase:

- Apple signing certificate en wachtwoord;
- App Store Connect API key, key ID en issuer ID;
- Apple team ID;
- notarizationcredentials;
- eventueel een afzonderlijk signingprofiel per apptarget.

Releasejobs importeren signingmateriaal alleen tijdelijk in een ephemeral
keychain en verwijderen die na afloop. PR-workflows uit forks of onbevestigde
branches krijgen nooit signingsecrets.

GitHub environments worden gebruikt zodra het repositoryplan dit voor private
repositories ondersteunt:

- `development`: geen productiecredentials;
- `beta`: TestFlight en beta-signing;
- `production`: release- en App Store-credentials, alleen tags vanaf `main`.

## 13. Rollback en incidenten

### macOS

- verwijder of pauzeer een foutieve draft vóór publicatie;
- na publicatie blijft de tag onveranderlijk;
- markeer een problematische release duidelijk in release notes;
- publiceer zo snel mogelijk een patchrelease;
- bied de vorige genotariseerde DMG als expliciete rollback aan;
- migrations moeten waar mogelijk backward-compatible of herstelbaar zijn.

### iOS

- pauzeer een phased release;
- een reeds verspreide App Store-binary wordt niet technisch teruggedraaid;
- maak bij een defect een patchrelease met hoger build- en versienummer;
- behoud protocolcompatibiliteit met minimaal de vorige ondersteunde Macversie,
  omdat iPhone- en Macupdates niet atomair plaatsvinden.

### Engine/configuratie

- maak vóór een migratie een lokale backup/snapshot;
- schrijf migraties versioned en idempotent;
- start na onherstelbare mismatch in `ARMED` of veilige read-only state, nooit
  automatisch in `LIVE`;
- log releaseversie, configuratierevision en protocolversie in iedere sessie.

## 14. Hotfixprocedure

1. Maak `hotfix/vX.Y.Z` vanaf de actuele `main`.
2. Pas alleen de urgente fix, tests, versie en changelog toe.
3. Open een PR naar `main`.
4. Doorloop alle releasechecks.
5. Merge met een normale merge commit.
6. Tag en publiceer als patchrelease.
7. Merge `main` onmiddellijk terug naar `dev`.
8. Controleer dat eventuele doorontwikkeling de fix niet opnieuw breekt.

## 15. GitHub-inrichting

### Huidige repositorystatus (2026-08-02)

- `dev` is aangemaakt en ingesteld als default branch;
- squash merge en merge commits zijn actief, rebase merge is uitgeschakeld;
- head branches worden na een merge automatisch verwijderd;
- de bronbranchguard voor PR's naar `main` en alle release-labels zijn actief;
- remote branch protection voor de private repository is nog niet actief, omdat
  GitHub dit op het huidige Free-plan weigert. Hiervoor is GitHub Pro nodig of
  moet de repository publiek worden gemaakt.

Tot branch protection beschikbaar is, blijft "nooit direct naar `main` pushen"
een procesregel. De Actions-guard controleert PR's, maar kan een directe push
niet blokkeren.

### Direct activeren na de initiële repositorycommit

- `dev` aanmaken en als default branch instellen;
- squash merge en merge commits toestaan;
- rebase merge uitschakelen;
- merged branches automatisch verwijderen;
- `main`: PR vereist, geen force-push of deletion (wacht op GitHub Pro);
- `dev`: geen force-push of deletion (wacht op GitHub Pro);
- guard-workflow voor toegestane bronbranches richting `main`;
- PR-template en release-notescategorieën toevoegen.

### Activeren zodra de eerste echte CI-checks bestaan

- vereiste checks op `dev` en `main`;
- conversation resolution op `main`;
- release-tagbescherming voor `v*`;
- dependency review en secret scanning waar beschikbaar;
- production environment en branch/tag policies;
- immutable GitHub Releases nadat de assetpipeline bewezen is.

Required reviews worden in de solo-fase niet verplicht, omdat de auteur zijn
eigen PR niet kan goedkeuren. De PR- en CI-gates blijven wel verplicht voor
`main`.

## 16. Release Definition of Done

Een versie is pas uitgebracht wanneer:

- de releasecommit op `main` staat;
- de beschermde tag exact naar die commit wijst;
- alle tests en releasechecks groen zijn;
- versievelden consistent zijn;
- changelog en release notes compleet zijn;
- macOS DMG geldig ondertekend, genotariseerd en geïnstalleerd is;
- checksums en SBOM gepubliceerd zijn;
- iOS-build in TestFlight op een echt device is gevalideerd;
- bekende beperkingen gedocumenteerd zijn;
- rollbackpad getest of expliciet beschreven is;
- `main` terug naar `dev` is gesynchroniseerd.
