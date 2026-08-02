# ADR-0010: Provider-onafhankelijke deck-sources

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

Lumi heeft live informatie nodig over decks, geladen tracks, timing en
master-/on-airstatus. Beat Link is daarvoor de eerste praktische integratie,
maar mag geen blijvende architectuurdependency van de core worden. Later moet
een native PRO DJ LINK-implementatie of een andere lokale integratietool kunnen
worden gebruikt zonder de planner, execution engine of clients te wijzigen.

De ontwikkelomgeving zonder fysieke decks heeft daarnaast simulator- en
replay-input nodig. Niet iedere bron levert dezelfde gegevens of dezelfde
betrouwbaarheid.

## Besluit

Lumi bezit een versiegebonden `DeckSourceProvider`-contract. Beat Link is de
eerste live provider achter dit contract en niet onderdeel van het Lumi-
domeinmodel.

Een provider vertaalt zijn bronspecifieke data naar genormaliseerde events,
waaronder waar beschikbaar:

- deck discovery, connectie en disconnectie;
- track-load en stabiele trackidentiteit;
- play-, cue-, tempo-master- en on-airstatus;
- BPM, beatnummer, beatfase en afspeelpositie;
- bronstatus, timestamps, sequence en datakwaliteit.

Iedere provider publiceert bij activatie een capability-set. De core gebruikt
alleen capabilities die expliciet aanwezig zijn en verzint ontbrekende data
niet. Genormaliseerde observaties bevatten minimaal bronidentiteit,
`observedAt`, sequence en freshness/quality zodat stale en onbetrouwbare input
veilig behandeld kan worden.

Voor autoritatieve deckstate is per showsessie precies één actieve live
provider aangewezen. Metadata-import en timing kunnen aanvullende bronnen zijn,
maar mogen niet ongemerkt conflicterende deckstate publiceren.

De eerste providers zijn:

1. `BeatLinkDeckSourceProvider` voor de productie-integratie;
2. `SimulatorDeckSourceProvider` voor ontwikkeling en demo;
3. `ReplayDeckSourceProvider` voor deterministische regressietests.

Een eventuele Beat Link/JVM-companion blijft buiten `lumi-core`. De adaptergrens
vertaalt proces- en libraryspecifieke types voordat events de centrale queue
bereiken. Een toekomstige `NativeProDjLinkDeckSourceProvider` implementeert
hetzelfde contract.

## Consequenties

- Geen Beat Link-, JVM- of PRO DJ LINK-type lekt naar planning, state of UI.
- Een provider kan crashen of herstarten zonder de engine mee te trekken.
- Sourceverlies sluit de outputgate of brengt Lumi in een expliciete degraded
  state; bestaande data wordt niet onbeperkt als actueel behandeld.
- Simulator, replay en live input doorlopen dezelfde reducer en planner.
- Contracttests en gedeelde eventfixtures zijn verplicht voor iedere provider.
- Trackmatching met de lokale Rekordbox-library blijft een afzonderlijke
  verantwoordelijkheid en is niet ingebakken in de live provider.

## Afgewezen alternatieven

### Beat Link rechtstreeks in de core

Afgewezen omdat Beat Link-types, lifecycle en runtimekeuzes dan door het hele
systeem gaan lekken en vervanging onnodig duur wordt.

### Lumi implementeert direct eerst zelf PRO DJ LINK

Afgewezen voor de eerste releases vanwege ontwikkel- en stabiliteitsrisico. De
adapter houdt deze optie voor later wel expliciet open.

### Meerdere gelijkwaardige live providers tegelijk

Afgewezen als standaard omdat volgorde-, identiteit- en masterconflicten dan
geen eenduidige bron van waarheid hebben.
