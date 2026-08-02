# Epic 1 – First Visible Lighting Plan

Status: **Ready for build review**

Doelrelease: **0.1.0**

## 1. Productresultaat

Epic 1 levert de eerste zichtbare, volledige Lumi-keten op een Apple Silicon
Mac, zonder DJ-hardware, Rekordbox-livekoppeling of SoundSwitch:

1. de gebruiker opent de native Lumi-app;
2. de app maakt verbinding met de lokale Rust-engine;
3. een gesimuleerde huidige en volgende track worden geladen;
4. Lumi maakt vooraf een deterministisch lighting plan voor de volgende track;
5. de gebruiker ziet `Live`, `Next` en het phraseplan;
6. theme, scene en lock kunnen worden aangepast;
7. een gesimuleerde masterwissel activeert het voorbereide plan;
8. versnelde playback voert phrasecues uit via een dry-run-outputprovider;
9. input, beslissingen en output zijn zichtbaar en reproduceerbaar.

Dit is een productmilestone en geen verzameling losse foundationtaken. Iedere
technische bouwstap eindigt in aantoonbaar werkende gebruikersfunctionaliteit.

## 2. Scope

### In scope

- native SwiftUI macOS-app met het afgesproken Lumi-appshell;
- dark appearance als standaard en volledig ondersteunde light appearance;
- Engelstalige UI via localization resources;
- centrale design tokens en herbruikbare UI-componenten;
- Camelot-keynotatie als standaard en Classic als configureerbare optie;
- zelfstandige Rust-engine als apart lokaal proces;
- versiegebonden lokaal command-, snapshot- en eventcontract;
- geserialiseerde single-writer state machine;
- `SimulatorDeckSourceProvider` met twee gesimuleerde decks;
- fixturedata met track-, beat- en phrase-informatie;
- minimale deterministische Planning Engine;
- `TrackLightingPlan` met revisions, origin en locks;
- `DryRunLightingOutputProvider` achter het generieke outputcontract;
- versnellen, pauzeren en resetten van de simulatie;
- zichtbare timeline van relevante source-, plan- en outputevents;
- unit-, contract-, golden- en end-to-endtests;
- CI voor Rust en de unsigned macOS-build.

### Buiten scope

- Rekordbox-library-import;
- Beat Link of native PRO DJ LINK;
- echte CoreMIDI- of SoundSwitch-output;
- autonome LaunchAgent-installatie via `SMAppService`;
- persistence over app-/engine-restarts;
- iPhone-client en netwerkpairing;
- signing, notarization, DMG en App Store-distributie;
- productiegeschikte automatische creatieve selectie;
- fixture-, DMX- of SoundSwitch-librarybeheer.

De simulator gebruikt realistische contracten, maar bewijst nog geen live
hardwarecompatibiliteit.

## 3. UX-scope

### 3.1 Appstructuur

De eerste versie implementeert uitsluitend de delen die voor de vertical slice
werken:

- globale `Arm`, `Start`, `Pause` en `Off`-control in de titelbalk;
- linkernavigatie met `Live` actief;
- `Plans`, `Library`, `Integrations` en `Settings` zichtbaar maar duidelijk als
  nog niet beschikbaar gemarkeerd;
- een `Live`-deckkaart en `Next`-deckkaart;
- phraseplanlijst voor de volgende track;
- rechter inspector voor geselecteerde phrasecue;
- onderste statusbalk met engine-, source- en outputstatus;
- compacte simulatorbediening binnen de Live-workspace.

### 3.2 Design system

Schermen definiëren geen eigen fonts, kleuren, spacing of controlhoogtes. De
macOS-target bevat één intern `LumiDesignSystem` met:

- semantische kleurrollen voor canvas, surface, border, text, accent, success,
  warning en destructive;
- typografierollen gebaseerd op native San Francisco system styles;
- spacingstappen `4`, `8`, `12`, `16`, `24` en `32` punten;
- standaard controlhoogtes en hoekradii;
- generieke `DeckCard`, `StatusBadge`, `PhraseRow`, `InspectorField`,
  `ProviderStatus` en `OperationControl`-componenten;
- consistente states voor loading, empty, ready, stale, degraded en error.

`AppearancePreference` ondersteunt `dark`, `light` en technisch alvast `system`.
De eerste installatie start in `dark`. `KeyNotationPreference` ondersteunt
`camelot` en `classic` en start in `camelot`.

Alle zichtbare tekst staat vanaf het begin in een String Catalog. Engels is de
enige meegeleverde en tevens de fallbacklocale. Trackkeys worden canoniek
opgeslagen als pitch class plus major/minor en uitsluitend weergegeven via één
`KeyNotationFormatter`.

## 4. Technische architectuur

```text
Lumi macOS – SwiftUI
        |
        | versioned commands, snapshots and events
        v
Local engine client + process supervisor
        |
        | loopback transport, session token
        v
lumi-engine – Rust process
        |
        v
bounded event queue -> single-writer reducer -> effects
        |                                      |
        |                                      v
SimulatorDeckSourceProvider       DryRunLightingOutputProvider
        |                                      |
        +----------> Planning Engine <----------+
                           |
                           v
                 TrackLightingPlan store
```

De domeinlogica bestaat uitsluitend in Rust. Swift bevat presentatie, lokale
preferences, engineprocesbeheer en protocolmapping, maar geen eigen planner of
alternatieve runtime-state.

### 4.1 Proces en transport

Voor Epic 1 start de macOS-app de gebundelde engine als child process voor de
duur van de appsessie. De lifecycle zit achter `EngineProcessSupervisor`, zodat
dit later door `SMAppService` kan worden vervangen zonder views of domeinlogica
te wijzigen.

Het eerste transport is loopback TCP op `127.0.0.1` met:

- een door de engine atomair gebonden vrije poort via `127.0.0.1:0`;
- een cryptografisch willekeurig, eenmalig session token;
- een eenmalige startup-readyregel via stdout met de gekozen endpointgegevens;
- newline-delimited JSON-envelopes;
- geen listener op LAN-interfaces;
- begrensde messagegrootte en connectietimeout.

Loopback TCP gebruikt native Network.framework aan Swift-zijde en Tokio aan
Rust-zijde. Het semantische protocol staat los van het transport. Een Unix
domain socket of `SMAppService`-lifecycle kan later worden toegevoegd achter
dezelfde interfaces.

### 4.2 Protocolminimum

Iedere envelope bevat minimaal:

```text
protocolVersion
messageType
messageId
sequence
correlationId
sentAt
payload
```

Commands voor Epic 1:

```text
GetSnapshot
LoadDemoSession
SetOperationState
SetSimulationSpeed
AdvanceToNextTrack
SelectTheme
SelectScene
SetCueLock
RegeneratePlan
ResetDemoSession
```

Engine-events voor Epic 1:

```text
EngineReady
StateSnapshot
StateChanged
DeckSourceStatusChanged
DeckStateChanged
PlanCreated
PlanRevised
PlanActivated
PhraseChanged
OutputEffectRecorded
OperationStateChanged
DiagnosticRaised
```

Muterende commands dragen de verwachte state- of planrevision. Duplicate
`messageId`-waarden zijn idempotent. Na een sequencegap vraagt de client een
volledige snapshot op.

### 4.3 Domeinminimum

Epic 1 implementeert alleen de minimaal benodigde domeintypes:

```text
DeckId
TrackId
TrackLoadInstanceId
TrackAnalysis
PhraseInstance
DeckObservation
LightingLeader
TrackLightingPlan
PlannedPhraseCue
PlanRevision
OperationState
SemanticLightingAction
OutputEffectResult
```

`TrackAnalysis` bevat voor de demo een canonical key, BPM, beatcount en een
geordende phrase-timeline. De fixture bevat geen UI-specifieke labels of
SoundSwitch MIDI-mapping.

### 4.4 Planningminimum

De eerste planner is bewust klein maar echt:

- een configuration revision identificeert de actieve regels;
- phrase type bepaalt de toegestane scenecategorie;
- een vaste seed afgeleid van track-ID en configuration revision maakt de
  keuze deterministisch;
- iedere phrase-instance krijgt vooraf precies één cue;
- iedere keuze bevat een machineleesbare reason;
- een userwijziging maakt een nieuwe revision met origin `USER`;
- een lock overleeft `RegeneratePlan` zolang de keuze geldig blijft;
- iedere track krijgt een expliciet fallbackplan.

De planner kent geen MIDI-notes, SoundSwitch-banks, Swift-types of
simulatorclock.

### 4.5 Simulator

De simulator is een echte `DeckSourceProvider` en publiceert dezelfde
genormaliseerde events als toekomstige live providers. De eerste demo bevat:

- Deck 1 als actieve lighting leader met een lopende track;
- Deck 2 met een geladen volgende track en volledige analyse;
- een voorspelbare beat- en phraseclock;
- een expliciete master-/leaderwissel;
- snelheden `1x`, `4x`, `16x` en `64x`;
- pause, resume en reset;
- een injectable monotone clock voor tests.

Fixtures zijn versioned, handmatig leesbare JSON-bestanden en bevatten een
verwacht plan- en outputtranscript.

### 4.6 Dry-run-output

`DryRunLightingOutputProvider` declareert capabilities maar verstuurt niets
buiten Lumi. Hij registreert per uitgevoerde cue:

- command-ID en planrevision;
- track-load-instance en phrase-instance;
- semantische lightingactie;
- geplande en werkelijke monotone executietijd;
- resultaat `SIMULATED`, `REJECTED` of `SKIPPED`;
- concrete reden bij afwijzen of overslaan.

Hiermee wordt het outputprovidercontract bewezen zonder CoreMIDI of
SoundSwitch-specifieke mapping.

## 5. Voorgestelde repositorystructuur

```text
Cargo.toml
engine/
  crates/
    lumi-domain/          pure domain types, reducer and invariants
    lumi-planner/         deterministic TrackLightingPlan generation
    lumi-protocol/        versioned wire DTOs and validation
    lumi-simulator/       SimulatorDeckSourceProvider and fixtures
    lumi-output-dry-run/  DryRunLightingOutputProvider
    lumi-engine/          process, queue, effects and transport
apps/
  macos/
    Lumi.xcodeproj
    Lumi/
      App/
      DesignSystem/
      EngineClient/
      Features/Live/
      Resources/Localizable.xcstrings
    LumiTests/
    LumiUITests/
contracts/
  protocol/v1/
fixtures/
  demo-session-v1/
scripts/
  verify.sh
```

Crates mogen tijdens implementatie worden samengevoegd als een grens nog geen
zelfstandige dependencyrichting oplevert. Domein, protocol, provider en UI
blijven wel logisch gescheiden; er worden geen lege architectuurcrates gemaakt.

## 6. Bouwvolgorde: vijf demoable increments

### Increment 1 – App-to-engine walking skeleton

Resultaat: de native app opent in dark mode, start de Rust-engine en toont een
echte health- en protocolstatus.

Werk:

- Rust workspace en macOS Xcode-target;
- engine entrypoint, structured logging en graceful shutdown;
- process supervisor, loopbackhandshake en reconnect/errorstate;
- protocolenvelopes, `GetSnapshot` en `EngineReady`;
- basis `LumiDesignSystem`, appshell en String Catalog;
- Rust- en macOS-buildchecks in CI.

Acceptatie:

- een clean checkout bouwt beide targets;
- de UI toont `Engine healthy` op basis van engine-data;
- protocolmismatch en engine-startfout zijn zichtbaar en crashen de app niet;
- de app bevat geen hardcoded niet-Engelse gebruikerscopy.

### Increment 2 – Visible next-track plan

Resultaat: `Load Demo Session` toont Live, Next en een gegenereerd phraseplan.

Werk:

- domeintypes, bounded queue en reducer;
- simulatorfixture en `SimulatorDeckSourceProvider`;
- minimale planner en planstore;
- state snapshot en events naar Swift;
- `DeckCard`, `StatusBadge` en `PhraseRow`;
- Camelot/Classic formatter en appearance preferences.

Acceptatie:

- dezelfde fixture maakt byte-equivalente canonieke plan-JSON;
- Next wordt gepland vóór leaderwissel;
- dark/light en Camelot/Classic wijzigen de presentatie, niet de domeindata;
- ontbrekende analyse geeft zichtbaar een fallbackplan.

### Increment 3 – User tuning and plan revisions

Resultaat: de gebruiker past theme of scene aan en kan een phrasecue locken.

Werk:

- revisioned mutationcommands en optimistic concurrency;
- user origin, locks, regenerate en rebase;
- inspectorcomponenten en selection state;
- stale-commandfeedback en audit reasons.

Acceptatie:

- iedere geldige edit verhoogt exact één planrevision;
- een stale edit wordt geweigerd en de UI synchroniseert opnieuw;
- gelockte geldige cues blijven behouden bij regenerate;
- de engine, niet de SwiftUI-view, bepaalt de resulterende planstate.

### Increment 4 – Simulated execution and output timeline

Resultaat: een masterwissel activeert het plan en versnelde playback produceert
zichtbare dry-run-output op phrasegrenzen.

Werk:

- operationele state machine;
- leaderwissel en planactivatie;
- phrase-boundary lookup;
- `DryRunLightingOutputProvider`;
- timeline/eventweergave en simulatorcontrols;
- pause, resume, reset en outputgate.

Acceptatie:

- `ARMED` plant maar produceert geen outputeffects;
- `LIVE` voert uitsluitend cues uit het actieve plan uit;
- `PAUSED` behoudt state en plan zonder nieuwe output;
- dezelfde fixture en commandreeks produceert hetzelfde outputtranscript;
- een oude track-load-instance kan nooit na de wissel uitvoeren.

### Increment 5 – Hardening and 0.1.0 evidence

Resultaat: de volledige demo is reproduceerbaar, getest en bruikbaar als
functionele milestone.

Werk:

- golden end-to-endscenario's;
- queue-overload-, disconnect- en malformed-message-tests;
- accessibility identifiers en kern-UI-tests;
- performance- en determinismechecks;
- README demo-instructies, known limitations en release-evidence;
- developmentversie naar `0.1.0-dev` en releasevoorbereiding voor `0.1.0`.

Acceptatie:

- één commando voert alle formattering, tests en builds uit;
- CI bouwt engine en unsigned macOS-app op een schone runner;
- een volledige sessie draait op `64x` zonder eventverlies;
- faults openen de outputgate nooit onverwacht;
- de demo kan door een eindgebruiker worden uitgevoerd zonder terminalkennis.

## 7. Uitvoerbare werkitems

| ID | Werkitem | Component | Effort | Afhankelijk van |
|---|---|---|---:|---|
| E1-00 | Lokale toolchainbootstrap en gecontroleerde environment check | Delivery | 2 | – |
| E1-01 | Rust workspace, macOS-target en gezamenlijke versioning | Delivery | 3 | E1-00 |
| E1-02 | Protocol v1 envelopes, schemas en contractfixtures | Engine | 5 | E1-01 |
| E1-03 | Engine process supervisor en loopbacktransport | macOS | 5 | E1-02 |
| E1-04 | LumiDesignSystem, English localization en preferences | macOS | 5 | E1-01 |
| E1-05 | Domeinmodel, bounded eventqueue en single-writer reducer | Engine | 5 | E1-01 |
| E1-06 | SimulatorDeckSourceProvider, clock en demo fixtures | Simulator | 5 | E1-05 |
| E1-07 | Deterministische minimale Planning Engine | Planner | 5 | E1-05, E1-06 |
| E1-08 | Live/Next read model en SwiftUI deck cards | macOS | 5 | E1-02, E1-04, E1-07 |
| E1-09 | Phraseplan inspector, revisions, locks en regenerate | Product | 8 | E1-07, E1-08 |
| E1-10 | Operation state, phrase execution en dry-run provider | Engine | 5 | E1-05, E1-07 |
| E1-11 | Simulatorcontrols en inspecteerbare outputtimeline | macOS | 5 | E1-08, E1-10 |
| E1-12 | Golden E2E, fault tests, CI en 0.1.0 evidence | Delivery | 5 | E1-03 t/m E1-11 |

Effort is relatief en geen urenraming. Werkitems worden per increment gegroepeerd
in kleine PR's; een item van effort 8 mag tijdens implementation refinement in
meerdere PR's worden gesplitst zonder een tweede productepic te maken.

## 8. Bouwomgeving en prerequisites

De environmentaudit op **2026-08-02** toont:

```text
Machine:       Apple Silicon arm64
macOS:         26.5.2
Swift CLI:     6.2.4
Xcode:         volledige Xcode niet actief; alleen Command Line Tools
Rust/Cargo:    niet geïnstalleerd
```

E1-00 levert daarom vóór productcode:

- volledige Xcode-installatie en selectie met `xcode-select`;
- acceptatie van de Xcode-license en een werkende `xcodebuild -version`;
- Rust via rustup met een in `rust-toolchain.toml` gepinde stable toolchain;
- target `aarch64-apple-darwin`;
- environmentcheck die Xcode, Swift, Rust, Cargo en architectuur valideert;
- een korte bootstrapinstructie zonder machinegebonden absolute paden.

De eerste appdeploymenttarget wordt **macOS 15.0** op Apple Silicon. De exacte
Xcode buildversie wordt na installatie in E1-00 vastgelegd en door CI
afgedwongen. Windows en Intel Macs zijn geen Epic 1-buildtargets.

## 9. Verificatiestrategie

### Rust

- unit tests voor invariants, reducertransities, planning en locks;
- property tests voor eventordering, duplicate commands en revisions;
- golden JSON voor plans en outputtranscripts;
- integratietest die de echte enginebinary start;
- `cargo fmt`, `cargo clippy --all-targets --all-features` en `cargo test`.

### Swift/macOS

- unit tests voor protocoldecoding, preferences en key formatting;
- viewmodeltests met opgenomen engine-events;
- UI-smoketest voor demo-load, phrase-selectie en appearancewissel;
- `xcodebuild build` en `xcodebuild test` op Apple Silicon CI.

### End-to-end

Een canoniek scenario bewijst minimaal:

```text
engine start
-> app handshake
-> demo session load
-> next plan created
-> user changes and locks Break 2
-> leader changes to Deck 2
-> playback at 64x
-> phrase actions recorded
-> pause blocks output
-> resume continues at next valid boundary
-> transcript equals golden fixture
```

## 10. Niet-functionele bouwgrenzen

- runtime werkt volledig zonder internet;
- engine luistert uitsluitend op loopback;
- alle queues en messages zijn begrensd;
- een plan voor maximaal 200 phrases wordt in een releasebuild binnen 50 ms
  berekend op de ondersteunde ontwikkel-Mac;
- kritieke source-to-state-events worden in normale simulatie binnen 10 ms door
  de reducer verwerkt;
- UI toont een geaccepteerde statewijziging binnen 100 ms na ontvangst;
- fixtureplayback op `64x` verliest geen kritieke events;
- onbekende enumwaarden en nieuwere protocolvelden falen gecontroleerd;
- engine- of transportverlies sluit output veilig en is zichtbaar;
- logs bevatten geen session tokens.

De eerste benchmarkresultaten vormen een baseline, geen marketingclaim.

## 11. Definition of Done

Epic 1 is pas gereed wanneer:

- de volledige productflow uit sectie 1 aantoonbaar werkt;
- alle vijf increments zijn geïntegreerd op `dev`;
- alle acceptance criteria en CI-checks groen zijn;
- `SimulatorDeckSourceProvider` en `DryRunLightingOutputProvider` uitsluitend via
  de vastgelegde providercontracten gekoppeld zijn;
- Swift geen planning- of executionbusinesslogica bevat;
- dark/light, English localization en Camelot/Classic uniform werken;
- de golden fixture en verwachte output gereviewd zijn;
- bekende beperkingen en demo-instructies zijn gedocumenteerd;
- er geen open P0/P1-defects binnen de epicscope zijn;
- release-evidence aan het GitHub-epic en milestone `0.1.0` is gekoppeld.

## 12. Risico's en beheersing

| Risico | Beheersing |
|---|---|
| Rust/Swift-integratie kost vroeg relatief veel tijd | Walking skeleton als eerste increment; protocol klein houden |
| UI gaat tijdelijke mockdata als waarheid gebruiken | Alle runtime-data komt via snapshots/events van de echte engine |
| Simulator wijkt semantisch af van latere live input | Zelfde `DeckSourceProvider`-contract en contracttests gebruiken |
| Planner wordt te vroeg creatief complex | Alleen minimale deterministic rules en reasons in Epic 1 |
| macOS lifecyclewerk groeit naar productieservice | Child-process lifecycle achter vervangbare supervisor houden |
| Golden tests worden fragiel | Canonieke serialisatie en alleen domeinrelevante velden snapshotten |
| UI-design raakt per scherm versnipperd | Alle styles en generieke componenten uitsluitend in DesignSystem |
| Lokale Apple/Rust-toolchain blokkeert de eerste build | E1-00 afronden voordat E1-01 start |

## 13. Bouwstartchecklist

Voor de eerste implementatie-PR:

- dit plan en de scope zijn productmatig goedgekeurd;
- GitHub Epic 1 beschrijft `First Visible Lighting Plan` als outcome;
- E1-00 t/m E1-12 staan als gekoppelde werkitems in Project 1;
- milestone `0.1.0` bevat alleen deze vertical-slice scope;
- `dev` is groen en de featurebranch start vanaf actuele `dev`;
- E1-00 heeft Xcode en Rust geïnstalleerd en de environmentcheck is groen;
- exacte Xcode buildversie is na installatie vastgelegd;
- de demo fixture bevat geen gelicentieerde audio en vereist geen externe data.
