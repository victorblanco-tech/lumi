# Lumi – architectuurdesign

Status: **Accepted baseline**
Datum: **2026-08-02**

## 1. Doel en scope

Lumi is een offline-first applicatiesysteem dat DJ-trackmetadata, live
deckstatus, beatpositie en phrase-informatie omzet in vooraf gevalideerde
lichtplannen. De plannen worden op muzikale phrasegrenzen via MIDI uitgevoerd
in SoundSwitch.

De eerste doelomgeving is Apple Silicon op macOS. De primaire operatorinterface
is een native macOS-app. Een native iPhone-app biedt in de booth live inzicht,
vooruitplanning, veilige tuning en diagnostiek. Windows-ondersteuning is een
latere mogelijkheid; de kernarchitectuur mag die niet blokkeren.

Lumi programmeert geen fixtures en stuurt geen DMX aan. SoundSwitch blijft
verantwoordelijk voor fixtures, Autoloops, Static Looks en DMX-output. Ableton
Link en/of de gekozen live bron blijven verantwoordelijk voor continue timing.

## 2. Leidende principes

1. **Plan vóór uitvoering.** Creatieve keuzes worden gemaakt wanneer een track
   op een deck wordt geladen, niet pas op de phrasegrens.
2. **De Mac-engine is de bron van waarheid.** UI's en controllers zijn clients,
   geen eigenaren van runtime-state.
3. **De live hot path is klein.** Op een phrasegrens wordt een vooraf
   gevalideerde cue opgezocht en uitgevoerd.
4. **Fail silent.** Bij twijfel of fout stuurt Lumi geen onverwachte MIDI.
5. **Hardware-onafhankelijk.** Control One is geen dependency van Lumi en is
   niet onderdeel van het domeinmodel.
6. **Handmatige bediening blijft direct.** Een fysieke controller mag
   SoundSwitch tussentijds overrulen; Lumi neemt op de volgende phrasegrens weer
   over, tenzij Lumi is gepauzeerd of uitgeschakeld.
7. **Geen internetdependency.** Planning, bediening en uitvoering werken volledig
   binnen de Mac en het lokale netwerk.
8. **Deterministisch en uitlegbaar.** Iedere keuze heeft een reden, revision en
   audit-event.

## 3. Systeemcontext

De drie uitgewerkte platen voor het totale landschap, de Lumi-internals en het
functionele gat staan in [Functionele architectuurplaten](visual-overview.md).

```text
                         lokaal wifi/LAN
  Lumi Remote (iPhone) <=================> Lumi Engine (Mac)
                                                ^
                                                |
                                      lokale versiegebonden IPC
                                                |
                                        Lumi voor macOS
                                                |
                    +---------------------------+--------------------------+
                    |                           |                          |
              Source adapters            Planning & execution       MIDI output
                    |                           |                          |
     Simulator / metadata / BLT       Lighting Plans en state       SoundSwitch
                                                                           ^
                                                                           |
                                                        fysieke MIDI-controller
                                                        (buiten Lumi om)
```

## 4. Procesarchitectuur

### 4.1 Lumi Engine

`lumi-engine` is een zelfstandige Rust-binary zonder gebruikersinterface. De
engine bevat:

- source-adapters;
- trackmatching en metadatareferenties;
- lighting-leaderselectie;
- Planning Engine;
- Execution Engine;
- centrale runtime-state;
- configuratievalidatie;
- MIDI-output;
- lokale API/IPC;
- gestructureerde logging en diagnostiek.

De engine draait als niet-geprivilegieerde macOS LaunchAgent binnen de actieve
gebruikerssessie. Daardoor blijft hij functioneren wanneer het appvenster wordt
gesloten of opnieuw gestart.

### 4.2 Lumi voor macOS

De macOS-app is een native SwiftUI-client. De app:

- registreert en beheert de LaunchAgent;
- toont `Live` en `Next`;
- beheert configuratie en pairing;
- toont preflight-, source-, MIDI- en servicestatus;
- geeft operationele commands door aan de engine;
- bevat geen beslislogica die nodig is voor een lopende show.

### 4.3 Lumi Remote voor iPhone

De iPhone-app is een native SwiftUI-client en communiceert uitsluitend met de
Mac-engine via het lokale netwerk. De app ondersteunt:

- live status en huidige cue;
- het plan van de volgende geladen track;
- theme- en phrasecue-aanpassingen;
- `Arm`, `Start`, `Pause`, `Off` en `Take Over Now`;
- veilige tuning;
- diagnostiek en recente events;
- een ingebouwde demomodus voor gebruik zonder Mac en voor App Store Review.

Verlies van de iPhone-verbinding heeft geen invloed op planning of uitvoering.

## 5. Plan-and-execute

### 5.1 Planning

Zodra een nieuwe track op een deck wordt geladen, publiceert de source-adapter
een `TrackLoaded`-event met een unieke `trackLoadInstanceId`. Laden staat los van
lighting-leader- of tempo-masterstatus.

De Planning Engine:

1. matcht de track met lokale metadata;
2. leest kleur en phrase-timeline;
3. projecteert de toepasselijke theme-regels en trackrotatie;
4. kiest vooraf een concrete loop voor iedere phrase-instance;
5. voegt fallbacks en outputacties toe;
6. valideert het volledige plan;
7. publiceert een bewerkbaar `AUTO_PROPOSED`-plan;
8. markeert het plan pas `READY` nadat de preflight slaagt.

Het maken van een plan verstuurt geen MIDI en verhoogt geen trackcounter. Een
track telt pas wanneer hij daadwerkelijk lighting leader wordt.

### 5.2 Lighting Plan

Het centrale domeinobject is `TrackLightingPlan`. De onderstaande
TypeScript-achtige notatie is illustratief en schrijft niet de implementatietaal
van het model voor:

```typescript
interface TrackLightingPlan {
  planId: string;
  revision: number;
  deckId: string;
  trackId: string;
  trackLoadInstanceId: string;
  metadataRevision: string;
  configurationRevision: string;
  status:
    | "BUILDING"
    | "AUTO_PROPOSED"
    | "USER_MODIFIED"
    | "READY"
    | "ACTIVE"
    | "STALE"
    | "COMPLETED"
    | "CANCELLED";
  theme: PlannedTheme;
  cues: PlannedPhraseCue[];
  validation: PlanValidation;
}
```

Een cue is gekoppeld aan één concrete phrase-instance, geïdentificeerd door
minimaal track-load-instance en startbeat:

```typescript
interface PlannedPhraseCue {
  cueId: string;
  phraseType: PhraseType;
  startBeat: number;
  endBeat: number;
  bank: number;
  loopId: string;
  intensity: number;
  origin: "AUTO" | "USER";
  locked: boolean;
  fallbackLoopId?: string;
}
```

Daardoor kunnen twee drops in dezelfde track verschillende vooraf gekozen loops
hebben.

### 5.3 Handmatige aanpassing en replanning

De gebruiker kan op Mac of iPhone:

- het geplande theme vervangen;
- één of meerdere phrasecues vervangen;
- keuzes locken;
- variaties opnieuw laten genereren;
- terugkeren naar het automatische voorstel.

Elke wijziging gebruikt optimistic concurrency met `planId`, `revision`,
`trackLoadInstanceId` en `configurationRevision`. Een wijziging voor een track
die inmiddels vervangen is, wordt geweigerd.

Wanneer automatische context verandert, wordt een nieuw basisplan berekend en
worden compatibele user-locks daarop gerebased. Een niet meer geldige gelockte
keuze maakt het plan `STALE`; Lumi vervangt die keuze niet stilzwijgend.

### 5.4 Activatie en uitvoering

Wanneer een deck lighting leader wordt, valideert Lumi dat het ready plan
nog bij exact dezelfde track-load-instance hoort. Daarna wordt het plan
atomair `ACTIVE`.

Op een phrasegrens voert de Execution Engine alleen de passende vooraf geplande
cue uit. Weighted random, kleurregels en theme-rotatie draaien niet in deze hot
path.

Een geplande handmatige themekeuze heeft standaard deze prioriteit:

1. globale Theme Lock;
2. user-modified Lighting Plan;
3. `FORCE`-kleurregel;
4. `PREFER`-kleurregel;
5. automatische rotatie;
6. huidig of standaardtheme.

Een handmatig gepland theme reset bij activatie standaard de rotatiecounter. Een
handmatig aangepaste loop binnen hetzelfde theme doet dat niet.

## 6. Live en Next

De primaire read model bevat minimaal:

```text
LIVE                           NEXT PER DECK
lighting leader               geladen track
actieve track                 metadata-/matchstatus
actieve phrase en cue         voorgesteld theme
actief theme en bank          geplande phrasecues
uitvoerstatus                 planrevision en preflight
laatste MIDI-resultaat        user-locks en stale-status
```

Lumi onderhoudt een kandidaatplan per deck. Bij twee decks wordt het plan van
het niet-leidende deck doorgaans als `Next` getoond. Het systeem blijft echter
deck-onafhankelijk en neemt niet aan dat iedere geladen kandidaat werkelijk de
volgende leader wordt.

Tempo-master, on-air en playstatus zijn signalen voor leaderselectie, maar zijn
geen voorwaarden om alvast een plan te bouwen.

## 7. Operationele toestand

De outputlevenscyclus staat los van planstatus en themeregels:

| State | Sources | Planning | MIDI-output |
|---|---:|---:|---:|
| `OFF` | uit | uit | geblokkeerd |
| `ARMED` | aan | aan | geblokkeerd |
| `LIVE` | aan | aan | actief op phrasegrenzen |
| `PAUSED` | aan | aan | geblokkeerd |

- `Arm` start bronnen, planning en preflight zonder output.
- `Start` opent de outputgate. Midden in een phrase wacht Lumi standaard tot de
  volgende geldige phrasegrens.
- `Take Over Now` is een afzonderlijk expliciet command dat de actuele cue
  onmiddellijk toepast.
- `Pause` sluit de outputgate maar laat tracking en planning doorlopen. Er wordt
  geen stop-, blackout- of resetcommando gestuurd.
- `Off` beëindigt de showsessie en ontkoppelt bronnen en output. Bestaande logs en
  configuratie blijven behouden.

Een ontbrekend MIDI-device blokkeert `Start`, maar niet `Arm` of de simulator.

## 8. MIDI en SoundSwitch

### 8.1 Semantische output

De core produceert semantische outputacties en kent geen hardcoded MIDI-noten:

```text
SELECT_AUTOLOOP_BANK(bank)
START_AUTOLOOP(slot)
ENABLE_STATIC_LOOK(id)
DISABLE_STATIC_LOOK(id)
```

Een SoundSwitch-outputprofiel vertaalt deze acties naar MIDI-messages. De
capaciteiten van het actieve targetprofiel bepalen onder andere het aantal
banks, slots en ondersteunde Static Look-acties.

### 8.2 Co-existentie met fysieke controllers

Een fysieke controller praat rechtstreeks met SoundSwitch en is standaard niet
gekoppeld aan Lumi:

```text
Control One of andere controller ---> SoundSwitch
Lumi via virtuele MIDI-poort --------> SoundSwitch
```

Een handmatige controlleractie krijgt daardoor onmiddellijk effect in
SoundSwitch. Lumi blijft intern het actieve plan en de timeline volgen, maar
stuurt standaard niets meer binnen dezelfde phrase. Bij de volgende
phrasegrens bevestigt Lumi opnieuw de volledig geplande basisstate. Wie langer
handmatig wil bedienen zet Lumi op `PAUSED`.

Omdat Lumi de externe bankstate niet kent, is iedere boundarycue zelfvoorzienend:

```text
SELECT_BANK (altijd, ook als Lumi denkt dat de bank al actief is)
WAIT configured bank-switch delay
START_AUTOLOOP
APPLY geplande overlays
```

Automatische subphrase-acties zijn geen onderdeel van de eerste versie, omdat
die een handmatige override vóór de volgende phrasegrens zouden kunnen
verstoren.

Static Looks kunnen bovenop een Autoloop actief blijven. De SoundSwitch-spike
moet vaststellen of een betrouwbare generieke resetactie bestaat. Tot die tijd
garandeert Lumi alleen het opnieuw toepassen van bank en Autoloop en het beheren
van Static Looks die expliciet in het outputprofiel zijn opgenomen.

### 8.3 Optionele generieke MIDI-input

Lifecyclecommands mogen optioneel aan willekeurige MIDI-notes of CC's worden
gekoppeld. Dit is een generieke usermapping en geen apparaatdependency.

## 9. iPhone-verbinding en beveiliging

De engine exposeert een versiegebonden lokaal control protocol. Transport en
protocol zijn gescheiden:

- macOS-client: lokale IPC, bij voorkeur een Unix-domain socket;
- iPhone-client: versleutelde verbinding over lokaal wifi/LAN;
- toekomstige Windows-client: hetzelfde semantische protocol met een passend
  lokaal transport.

De iPhone ontdekt Lumi via Bonjour. Pairing vereist fysieke bevestiging via een
QR-code of eenmalige code op de Mac. Het gekoppelde device bewaart zijn
credential in de iOS Keychain; de Mac kan devices intrekken.

Commands bevatten minimaal:

- uniek command-ID voor idempotentie;
- client-ID;
- verwachte plan- of state-revision;
- protocolversie;
- acknowledgement of concrete fout.

State-events bevatten oplopende sequence numbers. Bij een gemiste sequence haalt
de client een volledige snapshot op. Een stale of losgekoppelde client mag nooit
doen alsof een command is uitgevoerd.

Er is geen cloudaccount, internetverbinding of remote relay nodig. Voor
ontwikkeling wordt TestFlight gebruikt; de doelrelease van de native iPhone-app
is de App Store. Een ingebouwde simulator/demomodus maakt de app bruikbaar en
reviewbaar zonder aangesloten Mac of DJ-hardware.

## 10. Event- en concurrencymodel

Alle statewijzigingen lopen door één begrensde, geserialiseerde eventqueue en één
single-writer reducer:

```text
Source- of user-event
        -> reducer
        -> nieuwe state + beslisreden
        -> nul of meer effects
        -> output worker
        -> effectresultaat terug als event
```

Adapters en UI's wijzigen nooit rechtstreeks centrale state. I/O mag concurrent
zijn, maar de domeinbeslissingen zijn serieel. Monotone tijd wordt gebruikt voor
debounce, cooldowns en duplicate suppression; wall-clocktijd alleen voor
menselijke logging.

Queues zijn begrensd. Bij overbelasting worden nooit stilletjes kritieke
commands weggegooid: de engine gaat naar een gedegradeerde of veilige state en
rapporteert de oorzaak.

## 11. Data en lokale opslag

Werkbesluiten die tijdens implementatierefinement nog in een eigen ADR kunnen
worden vastgelegd:

- handmatig beheerbare configuratie in YAML;
- machineleesbaar schema voor startup- en preflightvalidatie;
- SQLite voor sessiehistorie, planrevisions en diagnostiek;
- roterende gestructureerde logs;
- secrets en pairingcredentials uitsluitend in Keychain-opslag;
- alle applicatie-assets lokaal gebundeld.

De engine schrijft nooit rechtstreeks in de live Rekordbox- of SoundSwitch-
database. Imports gebeuren read-only, bij voorkeur vanuit exports of snapshots.

## 12. Fout- en herstelgedrag

- Een UI-crash stopt de engine niet.
- `launchd` kan de engine na een onverwachte exit herstarten.
- Na een crash wordt runtime-state hersteld, maar de output start veilig gesloten.
- Een sourcefout maakt bestaande plannen niet automatisch actief.
- Een gewijzigde track-load-instance invalideert het bijbehorende oude plan.
- Geen of ongeldige metadata levert een expliciet fallbackplan of behoud van de
  huidige look, nooit een crash.
- Een iPhone-disconnect heeft geen invloed op de showsessie.
- Onbevestigde MIDI-output wordt als `ASSUMED`, niet als `CONFIRMED`, getoond.
- `Off` en `Pause` sturen geen onverwachte blackout of andere destructieve look.

## 13. Packaging en distributie

De macOS-distributie bestaat uit één gesigneerde en genotariseerde `Lumi.app`
voor Apple Silicon. De app bevat de enginehelper en registreert deze via
`SMAppService`. Voor normaal gebruik zijn geen rootrechten nodig.

De iPhone-app wordt als afzonderlijke native app gebouwd. TestFlight dient voor
beta's; App Store-distributie is het productdoel. De runtimefunctionaliteit blijft
offline en lokaal, ook wanneer installatie en updates via Apple lopen.

## 14. Nog te valideren technische spikes

De volgende onderwerpen zijn bewust nog geen bewezen capabilities:

1. exacte Rekordbox-phrase- en kleurimportstrategie;
2. live deck-, load-, beat-, on-air- en masterevents via Beat Link Trigger of
   PRO DJ LINK;
3. stabiele trackidentiteit tussen metadata-export, USB en live deck;
4. SoundSwitch MIDI-bank-, Autoloop- en Static Look-mappings;
5. betrouwbare bank-switchdelay en quantisatie;
6. resetgedrag van handmatige Static Looks;
7. gedrag wanneer Control One en Lumi gelijktijdig SoundSwitch bedienen;
8. lokale Bonjour-, pairing- en reconnectflow in een druk booth-netwerk;
9. App Store Review-flow met ingebouwde demomodus;
10. exacte Windows-service- en MIDI-adapters in een latere fase.

## 15. Niet in dit document

Dit document is de architectuurbaseline. Het bevat bewust nog geen:

- implementatieplanning;
- backlog of sprintindeling;
- definitieve repository/package-structuur;
- definitieve configuratieschemas;
- uitgewerkte UI-wireframes;
- resultaten van hardware- en metadataspikes.

Die artefacten volgen op basis van deze architectuur en de bijbehorende ADR's.
