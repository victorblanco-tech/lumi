# Lumi – bouw- en trackingplan

Status: **Accepted baseline**

Datum: **2026-08-02**

## 1. Doel

Lumi gebruikt twee complementaire omgevingen:

- **Buzz** is de primaire samenwerkingsruimte voor mensen en agents;
- **GitHub** is de formele bron voor backlog, voortgang, code en releases.

Hiermee blijft dagelijkse samenwerking rijk en direct, terwijl productscope en
delivery traceerbaar verbonden blijven met issues, pull requests, commits en
releases.

## 2. Verdeling van verantwoordelijkheden

### Buzz

Buzz bevat:

- actieve product- en architectuurgesprekken;
- agentdelegatie, resultaten en handoffs;
- onderzoek, brainstorms en tijdelijke werkcontext;
- channels per actief werkgebied of release;
- canvases voor gezamenlijke refinement en planning.

Een besluit met blijvende product-, architectuur- of delivery-impact wordt ook
vastgelegd in een ADR, ontwerpdocument of GitHub-issue. Buzz is niet de enige
bewaarplaats van een bindend besluit.

### GitHub

GitHub bevat:

- de productroadmap en release-milestones;
- epics en uitvoerbare werkitems;
- status, prioriteit, effort en component;
- acceptance criteria en afhankelijkheden;
- pull requests, builds en release-evidence.

Het projectboard is:

<https://github.com/users/victorblanco-tech/projects/1>

## 3. Hiërarchie

| Niveau | GitHub-object | Betekenis |
|---|---|---|
| Release | Milestone | Een testbare productversie |
| Bouwfase | Projectveld `Phase` | Technische/productmatige ontwikkelfase |
| Epic | Parent issue | Groot resultaat met duidelijke exitcriteria |
| Werkitem | Issue of sub-issue | Uitvoerbare feature, taak, bug of research spike |
| Implementatie | Pull request | Concrete wijziging die een issue geheel of deels sluit |

## 4. Boardvelden

- `Status`: Todo, In Progress, Done;
- `Phase`: bouwfase uit dit plan;
- `Priority`: P0 Critical, P1 High, P2 Normal, P3 Later;
- `Effort`: relatieve omvang 1, 2, 3, 5 of 8;
- `Component`: Product, Engine, Simulator, Deck sources, Planner,
  MIDI & SoundSwitch, macOS, iPhone of Delivery;
- `Work type`: Epic, Feature, Task, Bug of Research.

`blocked` is een label en geen status. Zo blijft zichtbaar in welke werkfase een
geblokkeerd item stond.

## 5. Werkstroom

1. Ideeën en vragen ontstaan meestal in Buzz.
2. Alleen voldoende duidelijke en relevante items worden GitHub-issues.
3. Een issue is `Todo` totdat scope en acceptance criteria uitvoerbaar zijn.
4. Een agent of mens zet maximaal een beperkt aantal items tegelijk op
   `In Progress`.
5. Een pull request verwijst naar het issue en bevat verificatie-evidence.
6. Een item wordt pas `Done` wanneer de acceptance criteria aantoonbaar zijn
   gehaald.
7. Belangrijke uitkomsten worden teruggekoppeld in het relevante Buzz-channel.

Latere fasen blijven op epicniveau totdat ze bijna actief worden. Alleen de
eerstvolgende bouwfase wordt fijnmazig uitgewerkt. Dit voorkomt een grote,
verouderde schijnbacklog.

## 6. Bouwfasen en releases

### Fase 1 – Foundation en simulator (`0.1.0`)

- engineering foundation en domeincontracten;
- reproduceerbare Rust- en Swift-builds;
- minimale native macOS-app met het gedeelde Lumi-designsysteem;
- fixtures en simulator clock;
- volledige dual-deck demo naar vooraf zichtbaar en aanpasbaar
  `TrackLightingPlan` naar dry-run-output;
- deterministische golden end-to-endtests.

Exit: de kleinste volledige Lumi-keten is zichtbaar en bedienbaar zonder
DJ-hardware en draait in CI. De technische uitwerking staat in
[Epic 1 – First Visible Lighting Plan](epic-01-first-visible-lighting-plan.md).

### Fase 2 – Deck intelligence en planning (`0.2.0`)

- provider-onafhankelijke library-source met Rekordbox 7 als eerste read-only
  snapshotadapter;
- duurzame canonieke music library en Lumi-owned phrase-timelines;
- native Library- en Track Lighting Editor met waveform en audio-preview;
- configureerbare phrase roles, source-mapping en versioned reimport/rebase;
- logische Theme × Phrase Role × Variant-matrix met late Theme-binding;
- provider-onafhankelijk deck-sourcecontract met Beat Link als eerste live
  provider;
- dual-deck en master state;
- detectie en planning van de volgende geladen track;
- phrase/loop/themeplanning;
- handmatige overrides en stale-editbescherming.

De fase bestaat uit drie opeenvolgende verticale epics:

1. [Epic 2A – Music Library and Track Lighting Editor](epic-02a-library-track-lighting-editor.md);
2. [Epic 2B – Live deck intelligence and rolling plans](epic-02b-live-deck-workspace.md);
3. de production-ready preplanned next-track planner.

Exit: echte librarytracks hebben een duurzame, bewerkbare Lumi-timeline en vóór
de transitie bestaat een volledig, uitlegbaar en aanpasbaar plan.

### Fase 3 – SoundSwitch Live MVP (`0.3.0`)

- [E3-00](https://github.com/victorblanco-tech/lumi/issues/75): de fysieke
  virtual-MIDI/Control One/DMX-keten is bewezen; repetition en reconnect blijven
  open;
- [E3-01](https://github.com/victorblanco-tech/lumi/issues/81): generaliseer de
  bewezen bank-delay-AutoLoopsequentie naar iedere geconfigureerde target;
- generieke MIDI-output;
- OFF, ARMED, LIVE en PAUSED;
- phrase-boundary execution;
- timing-readiness voor Beat Link Trigger, Carabiner en Ableton Link;
- coexistentie met fysieke SoundSwitch-bediening;
- veilige degradatie en emergency stop.

Exit: Lumi voert betrouwbaar uit zonder de gebruikersbediening over te nemen.

### Fase 4 – macOS Beta (`0.4.0`)

- native SwiftUI-app;
- autonome Rust-service via SMAppService;
- persistence, migraties, recovery en diagnostics;
- signing, notarization en installeerbare Apple Silicon DMG.

Exit: Lumi installeert en herstelt betrouwbaar op een schone Mac.

### Fase 5 – iPhone Remote Beta (`0.5.0`)

- lokale discovery en pairing;
- current/next deck en planpreview;
- theme-, scene- en loopaanpassingen;
- operationele controls en reconnectgedrag.

Exit: de DJ kan zonder internet veilig vanaf een gepairde iPhone tunen.

### Fase 6 – Live beta en stable (`1.0.0`)

- hardware-in-the-loop validatie;
- soak-, latency- en fault-injectiontests;
- security/privacy review;
- upgrade-, migratie- en rollbackpad;
- release candidate, TestFlight en live validatie.

Exit: de liveketen is bewezen stabiel, herstelbaar en releasewaardig.

## 7. Refinementregels

Een werkitem mag naar de actieve backlog wanneer het bevat:

- een concreet resultaat;
- afgebakende scope;
- observeerbare acceptance criteria;
- bekende afhankelijkheden en risico's;
- component, fase, prioriteit en effort;
- een test- of verificatiestrategie.

Research spikes leveren een besluit, meetresultaat of prototype op. Ze leveren
niet impliciet productiecode op en hebben standaard een beperkte timebox.

## 8. Ritme

- aan het begin van een werkblok: kies één primair resultaat;
- tijdens uitvoering: updates en samenwerking in Buzz;
- bij een betekenisvolle wijziging: issue en PR actualiseren;
- na merge: verificatie vastleggen en gekoppeld issue sluiten;
- per release: epic- en milestonestatus controleren;
- periodiek: backlog opschonen en latere fasen opnieuw beoordelen.
