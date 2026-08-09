# ADR-0012: Library-sources en Lumi-owned phrase-timelines

- Status: **Accepted**
- Datum: **2026-08-03**

> **Partial supersession:** ADR-0014 replaces the bar-only phrase-boundary
> decision below with beat-quantized Phrase Points. The remaining provider,
> ownership, safety, and persistence decisions stay accepted.

## Context

Lumi heeft trackmetadata, beatgrids, waveforms en een initiële phrase-analyse
nodig voordat een lichtplan kan worden gemaakt. Rekordbox 7 is de eerste en voor
de AlphaTheta-doelomgeving meest logische bron, maar de beschikbare
phrasevocabulaire is beperkt en blijft eigendom van Rekordbox. De gebruiker moet
grenzen en Lumi-specifieke rollen zoals `Synth`, `Pre-drop`, `Breakdown 1` en
`Buildup 3` onafhankelijk kunnen beheren.

Deckobservatie en library-import zijn verschillende verantwoordelijkheden. Een
live deck-source vertelt welke track geladen is en welke beat speelt; een
library-source levert duurzame analysegegevens. Geen van beide mag impliciet
eigenaar worden van door de gebruiker bewerkte Lumi-data.

## Besluit

Lumi introduceert een versiegebonden `MusicLibrarySourceProvider` naast het
bestaande `DeckSourceProvider`-contract. Een library-provider levert een
onveranderlijke importbaseline met bronidentiteit, bronrevision, capabilities,
trackmetadata, beatgrid, waveform en waar beschikbaar raw phrase-observaties.

`Rekordbox7LibrarySourceProvider` is de enige Rekordbox-provider in de eerste
versie. Lumi:

- detecteert uitsluitend Rekordbox 7;
- importeert alleen wanneer Rekordbox gesloten is;
- maakt eerst een consistente read-only snapshot buiten de live library;
- schrijft nooit in Rekordbox, analysebestanden of audiobestanden;
- ondersteunt Rekordbox 5 en 6 bewust niet.

Ontwikkeling en geautomatiseerde tests gebruiken nooit een productie-library.
Een deterministische `DemoLibrarySourceProvider` levert licentievrije tracks,
audio, waveforms, beatgrids, kleuren en raw phrase-observaties via exact hetzelfde
providercontract. De Rekordbox-spike en smoke tests worden uitsluitend uitgevoerd
tegen een wegwerp-library in een geïsoleerd macOS-developmentaccount. Toegang tot
een productie-library is geen ontwikkelvoorwaarde en wordt pas na de spike als
expliciete gebruikersactie toegestaan.

Bij de eerste import maakt een configureerbaar source-mappingprofiel uit de raw
phrase-observaties een eigen `LumiPhraseTimeline`. Vanaf dat moment is deze
timeline autoritatief voor planning. Een timeline:

- is gekoppeld aan een stabiele Lumi-trackidentiteit;
- gebruikt de beatgrid als tijdas en is volledig aaneengesloten;
- gebruikt stabiele, configureerbare `PhraseRoleId`-waarden;
- ondersteunt split, merge, boundary move, create, delete en role change;
- heeft oplopende revisions en herstelbare historie;
- bewaart provenance naar de importbaseline zonder daarvan afhankelijk te
  blijven.

Phrasegrenzen liggen uitsluitend op de eerste beat van een volledige maat.
Individuele beats blijven zichtbaar en adresseerbaar voor waveform, playhead en
uitvoering, maar de editor kan geen phrasegrens binnen een maat maken. Zoom,
selectie, split en boundary move werken in gehele maten; vrije beat- of
millisecondegrenzen zijn niet toegestaan. Een
library-refresh werkt metadata en veilige waveformdata bij, maar overschrijft
nooit een Lumi-timeline. Wanneer beatgrid of source-phrases wijzigen, biedt Lumi
een expliciete vergelijking met `Keep Lumi`, `Rebase`, `Merge` en
`Replace with source`. Ook `Replace` maakt eerst een herstelbare revision.

De canonieke library, baselines en timeline-revisions worden duurzaam opgeslagen
in lokale SQLite-opslag achter een application-owned repositorypoort. Het
domeinmodel en de clients kennen geen SQL- of Rekordbox-types. Audio blijft op de
oorspronkelijke locatie; alleen analyse, verwijzingen en afgeleide caches worden
opgeslagen.

Een gematchte live track gebruikt altijd de Lumi-timeline. Een onbekende live
track mag tijdelijke provideranalyse gebruiken voor een expliciet fallbackplan,
maar die analyse muteert de library niet stilzwijgend.

## Consequenties

- Rekordbox initialiseert Lumi-data maar blijft geen runtime-afhankelijkheid.
- Toekomstige library-adapters kunnen dezelfde importbaseline leveren.
- Handmatige phrase-edits overleven bronverlies, refresh en apprestarts.
- Persistence verhuist naar de Library-epic; alleen service-lifecycle en
  volledige crashrecovery blijven voor de latere macOS-service-epic.
- Reimport vereist een zichtbaar conflict- en rebasepad.
- Trackmatching tussen library- en live-identiteit blijft een expliciete service.
- De planner leest geen raw Rekordbox-phrasetypes.
- De volledige Library- en editorstroom kan tegen de demo-provider worden gebouwd
  en bewezen voordat toegang tot een Rekordbox-developmentlibrary bestaat.
- Een bronadapter die onverwacht schrijfaccess, een onbekende versie of een
  instabiele snapshot nodig heeft, faalt gesloten.

## Afgewezen alternatieven

### Rekordbox als blijvende bron van waarheid

Afgewezen omdat de vaste phrasevocabulaire onvoldoende is voor de gewenste
lichtstructuur en toekomstige adapters dan tweederangs worden.

### De live Rekordbox-database rechtstreeks muteren

Afgewezen vanwege corruptie-, compatibiliteits- en ownershiprisico. Import is
altijd read-only en snapshotgebaseerd.

### Reimport vervangt automatisch alle Lumi-edits

Afgewezen omdat sourcewijzigingen dan zonder toestemming handmatig ontworpen
lichtgedrag kunnen vernietigen.

### Phrasegrenzen op absolute tijd opslaan

Afgewezen omdat live uitvoering beatgebaseerd is en tijdposities bij tempo- en
beatgridcorrecties onnodig fragiel zijn.

### Phrasegrenzen op iedere willekeurige beat toestaan

Afgewezen omdat lichtphrases in deze workflow op volledige muzikale maten worden
voorbereid. Beats blijven zichtbaar voor herkenning en timing, maar phrase-editing
blijft bewust bar-aligned en daardoor sneller, voorspelbaarder en veiliger.
