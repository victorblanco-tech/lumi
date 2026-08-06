# Epic 2A – Music Library and Track Lighting Editor

Status: **Demo scope complete through E2A-19; safe Rekordbox 7 import is the next selected work package**

Doelmilestone: **0.2.0 – Deck Intelligence**

## 1. Productresultaat

Epic 2A maakt van de lege `Library`-navigatie een bruikbare offline
voorbereidingsomgeving. Zonder decks of SoundSwitch-koppeling kan de gebruiker:

1. de volledige workflow eerst veilig gebruiken met een deterministische
   demo-library en later een gesloten Rekordbox 7-library read-only importeren;
2. tracks, playlists, metadata, waveform, beatgrid en bronphrases zien;
3. luisteren, scrubben, een phrase loopen en Phrase Points op hele beats plaatsen
   terwijl waveformnavigatie, zoom en playhead volledig vrij blijven;
4. eigen Lumi-phrases maken en roles toewijzen;
5. per phrase automatische selectie behouden of een vaste logische variant
   kiezen;
6. een trackkleurregel een voorlopig Theme laten kiezen;
7. het Theme in een preview wijzigen en alle concrete dry-run-Autoloops opnieuw
   laten resolven;
8. een echte librarytrack in de bestaande dual-deck simulator testen;
9. bronwijzigingen vergelijken zonder Lumi-edits te verliezen;
10. het ingebouwde SoundSwitch-outputprofiel onder `Integrations > Lighting Outputs` beheren als vier banks met ieder
    32 AutoLoop-posities, een exacte AutoLoop Name en één Phrase Type per positie.

Dit levert een zichtbare vertical slice over import, persistence, native UI,
editing en planning. De echte SoundSwitch-project- en bank/slotbinding valt
buiten deze epic.

## 2. Scope

### In scope

- alleen Rekordbox 7-detectie en -import op macOS;
- deterministische `DemoLibrarySourceProvider` met licentievrije audio en analyse;
- ontwikkel- en smoketests uitsluitend tegen een geïsoleerde wegwerp-library;
- import uitsluitend wanneer Rekordbox gesloten is;
- consistente read-only snapshot en versie/capabilityvalidatie;
- provider-neutraal `MusicLibrarySourceProvider`-contract;
- stabiele trackidentiteit en matchingfacts voor toekomstige live providers;
- lokale SQLite-opslag achter een repositorypoort;
- importbaselines en versioned Lumi phrase-timelines;
- playlistbrowser, search, importstatus en track-readiness;
- een Rekordbox/CDJ-geïnspireerde vaste donkere editorcanvas met beatgrid boven
  een continu gerenderde RGB-waveform en gekleurde Lumi-phrases eronder;
- play, pause, stop, seek, scrub, vorige/volgende maat, volume en selected-
  phrase-loop zonder de show engine te beïnvloeden;
- beat-quantized Phrase Point create/move/delete, afgeleide aaneengesloten
  ranges, role change, undo/redo en revision restore;
- configureerbare phrase roles in `Settings > Phrase Model` en providergebonden
  initiële mapping in `Library > Sources & Import`;
- default roles uit ADR-0013;
- logische `Theme × Phrase Role × Variant`-matrix met fixtures;
- `AUTO`, `FIXED_VARIANT` en `THEME_SPECIFIC_EXACT`;
- provider-neutrale kleurregels en uitlegbare Theme-selectie;
- planinstance Theme-override in preview, zonder library-Theme te muteren;
- simulatorintegratie met echte geïmporteerde tracks;
- diff, rebase, merge en replace bij sourcewijzigingen;
- contract-, parser-, persistence-, UI-, golden-, performance- en end-to-endtests.
- demo-data SoundSwitch Lighting Output met Banks & Autoloops, Test Controller
  en MIDI Status onder de taakgerichte Integrations-workspace;
- taakgerichte desktopinformatiearchitectuur met Library Sources, Deck Inputs,
  Lighting Outputs, Diagnostics en opgeschoonde globale Settings.

### Buiten scope

- Rekordbox 5 of 6;
- schrijven naar Rekordbox of audiobestanden;
- live import terwijl Rekordbox draait;
- echte PRO DJ LINK- of Beat Link-ingest;
- SoundSwitch-projectparsing en automatische catalogusrefresh;
- echte SoundSwitch-bank-, slot- of MIDI-binding;
- CoreMIDI-output;
- autonome LaunchAgent-installatie;
- iPhone-implementatie;
- cloudsync, accounts of internetafhankelijkheid;
- AI-audioanalyse;
- ontwikkeltoegang tot een productie-Rekordbox-library.

## 3. Domeinmodel

```text
MusicLibrarySourceProvider
    -> ImportedTrackAnalysis baseline
    -> LumiTrack
        -> LumiPhraseTimeline revision
            -> PhraseInstance + PhraseRoleId
            -> LoopStrategy

LogicalLightingCatalog
    -> ThemeId
        -> PhraseRoleId
            -> VariantId
                -> concrete dry-run catalog entry
```

Een tracktemplate bewaart geen vast Theme. `PhraseRoleId` bepaalt rechtstreeks
de Autoloop Category. De runtimeplanner kiest eerst een Theme en resolveert daarna
de concrete matrixcel. Een vaste variant blijft dezelfde rij gebruiken wanneer
het Theme verandert.

## 4. UX

### 4.1 Library workspace

- linker kolom: Collection, playlists en readinessfilters;
- midden: doorzoekbare tracktabel met import- en analysisstatus;
- rechter detail: metadata, source revision en warnings;
- editorcanvas: CDJ-geïnspireerd en vast donker in zowel Lumi dark als light
  appearance, met track, Camelot-key, BPM, resterende tijd en actuele bar/beat;
- tijdas van boven naar beneden: bar/beatgrid, continu gerenderde RGB-waveform,
  gekleurde Lumi-phrases en een compacte full-track overview eronder;
- inspector: role, start/eindmaat, origin, revision en loopstrategie;
- preview: voorlopig Theme, reason en opgeloste dry-run-Autoloop per phrase;
- transport: play/pause, stop, seek/scrub, vorige/volgende maat, volume en
  `Loop selected phrase`;
- editacties: plaats/verplaats/verwijder een Phrase Point, change role,
  undo/redo, save revision en revision restore; ieder eindpunt wordt afgeleid
  van het volgende Phrase Point of trackeinde;
- shortcuts: Space voor play/pause, Left/Right voor één beat, Shift+Left/Right
  voor één maat en `P` voor een nieuw Phrase Point;
- acties: Refresh, Load on Deck A/B, Compare source en revision history.

Alle controls gebruiken het bestaande Lumi Design System, dark/light appearance,
Engelse localization resources en de configureerbare Camelot/Classic-keynotatie.

### 4.2 Veilige editingregels

- iedere maat en daarmee iedere beat behoort aan precies één phrase;
- phrasegrenzen quantizen uitsluitend naar volledige beats; waveformpan, zoom,
  scrub en playhead blijven continu en zijn niet maatgebonden;
- ongeldige overlaps, gaps en zero-length phrases worden geweigerd;
- delete absorbeert de geselecteerde phrase expliciet in een buur of wordt als
  merge uitgevoerd en kan nooit een gap maken;
- iedere mutatie maakt een nieuwe revision;
- split erft de role aan beide kanten, behoudt een exacte keuze links en zet het
  nieuwe rechterdeel op `AUTO`;
- bronrefresh overschrijft nooit user revisions;
- edit van een actieve Live-track wijzigt het reeds actieve plan niet;
- editing tijdens audio-preview onderbreekt het geluid niet; de selected-phrase
  loop volgt pas na een geldige boundarywijziging de nieuwe maatgrenzen.

## 5. Theme- en matrixgedrag

- trackkleur is de eerste configureerbare Theme-fact;
- een kleur kan een Theme forceren of een gewogen kandidaatset prefereren;
- rotatie en no-repeat blijven actief wanneer geen keuze wordt geforceerd;
- een preview- of toekomstige iPhone-keuze muteert alleen de planinstance;
- Theme-wijziging resolveert alle concrete varianten opnieuw;
- `FIXED_VARIANT` blijft dezelfde matrixrij gebruiken;
- ontbrekende cellen blijven binnen dezelfde Phrase Role en worden zichtbaar als
  fallback of preflightprobleem;
- iedere Theme- en variantkeuze heeft een machineleesbare reason;
- vier banktabs vertegenwoordigen vier Theme-targets; variants zijn flexibele
  alternatieven binnen een role en bank en hebben geen vaste count.

## 6. Story map

### E2A-00 – Prove safe Rekordbox 7 analysis extraction

Timeboxed research naar detectie, consistente snapshot, trackidentiteit,
waveform, beatgrid, kleur en raw phrases op de daadwerkelijk geïnstalleerde
Rekordbox 7-versie in een geïsoleerd developmentaccount. Levert fixtures,
parserproof en een go/no-go-besluit; de productie-library wordt niet gebruikt.

### E2A-01 – Persist the canonical Lumi music library

Introduceert sourcecontracten, stabiele identities, SQLite-repository,
migraties, importbaselines en revision-safe transacties. Levert tevens de
deterministische demo-provider waarmee de rest van de epic zonder Rekordbox kan
worden gebouwd.

### E2A-02 – Import a closed local Rekordbox 7 library

Bouwt detectie, read-only snapshot, incremental import, providerstatus en een
zichtbare import/refreshflow in de macOS-app.

### E2A-03 – Browse and inspect imported tracks

Activeert de Library-workspace met playlists, search, filters, metadata,
readiness, errors en importprovenance.

### E2A-04 – Render waveform and audition local audio

Levert de CDJ-geïnspireerde editorcanvas met beatgrid, gekleurde waveform,
gekleurde phrase lane en overview. Ondersteunt lokaal play/pause/stop, seek,
scrub, maatnavigatie, volume en selected-phrase-loop zonder bestanden te
kopiëren of showstate te muteren.

### E2A-05 – Own and edit versioned Lumi phrase timelines

Maakt de Lumi-timeline autoritatief en implementeert uitsluitend bar-aligned
create, split, merge, boundary move, delete/absorb, role change, undo/redo,
revisions en validatie in engine en native UI.

### E2A-06 – Configure phrase roles and initial source mapping

Levert `Settings > Phrase Model` met stabiele IDs, toevoegen, hernoemen,
ordenen, archiveren/herstellen, usage diagnostics en de afgesproken defaults.
Provider-specifieke initiële mapping woont bij de bron in
`Library > Sources & Import`. In-use roles worden nooit hard verwijderd.

### E2A-07 – Build the logical Theme/role/variant matrix

Introduceert de provider-neutrale catalogus, consistente matrixrijen, Theme-
fixtures, vier benoembare Theme-banktabs, flexibele variants per role,
coverage/preflight en veilige role-fallbacks zonder SoundSwitch-types. Bank en
variant blijven verschillende assen.

### E2A-08 – Assign per-phrase loop strategies

Ondersteunt `AUTO`, theme-onafhankelijke `FIXED_VARIANT` en optionele
`THEME_SPECIFIC_EXACT`, inclusief locks, stale validatie en editorweergave.

### E2A-09 – Select and override late-bound Themes

Implementeert kleurregels, reasoned Theme-selectie, rotatie/no-repeat en een
planinstance Theme-switch die alle dry-run-cues opnieuw resolveert zonder de
library te muteren.

### E2A-10 – Compare and reconcile source changes

Detecteert gewijzigde metadata, beatgrids en phrases en biedt Keep, Rebase,
Merge, Replace en revision recovery.

Status: **complete**. De deterministische V1/V2 demo-baselines, onafhankelijke
change classification, metadata-safe refresh, bar-aligned Rebase,
per-conflict Merge, recoverable Replace, atomaire SQLite-transactie,
golden previewfixture en native editorflow zijn gebouwd en geverifieerd.

### E2A-11 – Run imported tracks through the simulator

Laadt een echte geïmporteerde track op Deck A/B, gebruikt uitsluitend de
Lumi-timeline en bewijst phrase- en matrixresolutie in de bestaande Live/Next UI.

Status: **complete**. De Library kan elke demo/importtrack revision-safe op Deck
A of B laden. Provider-neutrale identity facts, de exacte Lumi-timeline,
late-bound Theme-resolutie, logische Autoloop-evidence, atomaire activatie,
versnelde exactly-once dry-run-output, typed mismatchgedrag, een golden
transcript en native visuele evidence zijn gebouwd en geverifieerd.

### E2A-12 – Prove Epic 2A end-to-end

Levert golden import/editor/preview/reimportfixtures, grote-librarybenchmarks,
fault injection, architecture checks, visuele evidence en een gedocumenteerde
demo met bekende beperkingen.

Status: **demo-data proof in verification**. De golden Library/editor/restart/
refreshflow, vier-Theme-resolutie, Library→Simulator-evidence, expliciete
10.000-trackbudgetten, faultmatrix, visual manifest en demo/limitations guide
zijn onderdeel van de vaste repositorygate. Definitieve afronding wacht bewust
op E2A-00/E2A-02 met een geïsoleerde wegwerp-Rekordbox-7-library; de productie-
library wordt niet als ontwikkelbewijs gebruikt.

### E2A-13 – Align the Track Editor with Rekordbox/CDJ phrase-point workflow

Vervangt de block-achtige en bar-only editorervaring door de geaccepteerde
Track Editor UX: continu gerenderde RGB-waveform als standaard, volledige zoom
en horizontale scroll, vrije playhead, beatgrid met maataccenten, Phrase Points
die op één beat quantizen, automatisch afgeleide phrase-ranges en een compacte
full-track overview eronder. Migreert bestaande bar-aligned timelines
verliesvrij naar beatgridposities en behoudt revision-, undo/redo-, audio- en
plannerveiligheid.

Status: **implemented; local verification and native visual evidence pass**.
Het leidende design staat in
[`docs/design/track-editor`](../design/track-editor/README.md) en ADR-0014.

### E2A-14 – Integrate and resize the Track Editor workspace

Vervangt de modale editor door een ingebedde verticale split: de Track Editor
boven en de volledige Library-browser eronder. De gebruiker kan de divider
verslepen om meer editruimte of meer tracklistruimte te kiezen. De gedetailleerde
waveform ondersteunt native horizontale trackpad-/muiswielpan. Interne Phrase
Points tonen expliciete resize-handles en blijven bij slepen op hele beats
quantizen. De Swift wire-validator accepteert daarbij iedere geldige hele beat,
niet alleen maatgrenzen, zodat opgeslagen edits na herstart heropenen.

Status: **implemented; local package and native application verification pass**.
De geaccepteerde layout- en interactieregels zijn toegevoegd aan het Track
Editor design. Verdere CDJ/Rekordbox RGB-pixelfidelity blijft een aparte
[E2A-15 rendererverbetering](https://github.com/victorblanco-tech/lumi/issues/70)
en verandert deze UX-contracten niet.

### E2A-15 – Refine RGB waveform to CDJ/Rekordbox fidelity

Verbetert spectral weighting, compositing, transientdetail en zoomdensity zonder
de geaccepteerde editorinteracties of phrasegeometrie te wijzigen.

Status: **parked by product decision on 2026-08-04**. De huidige provider-
neutrale RGB-renderer blijft bruikbaar voor de demo. Pixel-fidelity wordt later
opgepakt zodat de huidige ontwikkelcapaciteit naar de SoundSwitch/MIDI-kern gaat.

### E2A-16 – Make the persistent editor and native track table the Library baseline

Maakt de Track Editor permanent onderdeel van Library en laadt bij openen veilig
de eerste beschikbare track. Een dubbelklik op iedere cel van een andere
trackrij laadt die track in dezelfde editor. De losse metadata-inspector vervalt:
trackkleur/titel, artiest, BPM, key, duration, source, Lumi timeline en readiness
worden native tabelkolommen; technische IDs en analysis revision zijn optioneel.
Kolommen zijn met standaard macOS-interacties te verslepen en in breedte aan te
passen en de lokale indeling blijft bewaard.

De app gebruikt voortaan overal de semantische near-black/dark-gray Lumi-basis
met een vaste cyan accentkleur. De Phrase Inspector en vaste editorcontrols
passen zonder geneste verticale scrollbar. Dit verandert nadrukkelijk niets aan
de onafhankelijke horizontale waveformpan: trackpad-swipe, horizontaal muiswiel
en overview-drag blijven ondersteunde navigatievormen.

Status: **implemented; package tests, full repository gate, and native
application verification pass**.

### E2A-17 – Present the built-in SoundSwitch Autoloop Output Profile

Vervangt de abstracte matrix-first Settings-weergave door de eerste bank-first
Output Profiles UX. De oorspronkelijke 32-posities-per-bankprojectie is door
E2A-18 gecorrigeerd naar de echte SoundSwitch-surface.

Status: **implemented and locally verified**. Het ontwerp staat in
[`docs/design/output-profiles`](../design/output-profiles/soundswitch-autoloops.md),
de controllergrens in ADR-0015 en het fysieke integratiebewijs in
[`soundswitch-coremidi-poc.md`](soundswitch-coremidi-poc.md).

### E2A-18 – Map the real 4×32 SoundSwitch AutoLoop surface

Status: **implemented and locally verified**. Iedere bank heeft exact 32
unieke AutoLoop-posities die samen verschijnen zodra de bank wordt gekozen. De
gebruiker beheert per positie de exacte `AutoLoop Name` en kiest een
configureerbaar Lumi `Phrase Type`. De mapping wordt atomair en persistent
opgeslagen; interne mapping-ID's worden niet als Variant Name getoond. Een
range- of pagina-indeling is bewust afwezig; de inspector vervangt de gekozen
bank niet. De
Test Controller spiegelt dezelfde 4×32-configuratie en blijft send-disabled tot
de virtuele MIDI-bron expliciet wordt gepubliceerd. De permanente `MIDI Status`-
pagina toont bronstatus, protocol, configured surface, laatste event, pulsteller,
integratiechecks en handmatige testacties. De Test Controller gebruikt dezelfde
SoundSwitch-volgorde: 1–8 verticaal, gevolgd door 9–16, 17–24 en 25–32.

### E2A-19 – Reorganize Library, Integrations, and Settings around user tasks

Status: **implemented and locally verified**. De primaire `Integrations`-
navigatie is actief met Overview, Deck Inputs, Lighting Outputs en Diagnostics.
De bestaande Beat Link Trigger- en SoundSwitch-schermen zijn zonder duplicatie
naar hun taakgerichte bestemming verhuisd. `Library` heeft Tracks en
Sources & Import; Rekordbox 7 staat daar klaar als disabled read-only source en
de initiële source phrase mapping is met de bron meeverhuisd. Settings bevat
alleen General, Phrase Model en Planning Defaults. Overview deeplinkt naar de
eigenaar van ieder component. Een grens-test voorkomt dat providerconfiguratie
weer in Settings belandt. Het geaccepteerde ontwerp staat in
[`docs/design/information-architecture`](../design/information-architecture/README.md).

### [E2A-20 – Configure a Rekordbox XML source and discover playlists](https://github.com/victorblanco-tech/lumi/issues/92)

Status: **delivered**. Configureert een read-only XML-importfolder onder
`Library > Sources & Import`, ontdekt exports en toont de playlist/folderboom
zonder de volledige Collection te importeren. Een gevolgde folder kan optioneel
toekomstige child-playlists meenemen; deze optie staat standaard aan.

### [E2A-21 – Mirror followed Rekordbox playlists without losing Lumi edits](https://github.com/victorblanco-tech/lumi/issues/91)

Status: **in progress; read-only mirror normalization delivered**. De engine
resolveert nu uitsluitend gevolgde folders/playlists, dedupliceert tracks over
playlists en normaliseert dubbele memberships binnen één Rekordbox-playlist
zichtbaar en deterministisch. De echte gekozen export is lokaal gevalideerd op
52 playlists en 684 unieke tracks. Persistente import, volgorde, archive en
restore volgen voordat deze story gereed is.

### [E2A-22 – Preview and apply Rekordbox XML sync changes](https://github.com/victorblanco-tech/lumi/issues/90)

Status: **in progress; source-scope preview delivered**. `Preview Sync` leest de
nieuwste export opnieuw in de Rust engine, bindt de uitkomst aan SHA-256 en toont
playlist-, unieke-track-, collection- en capabilitydiagnostiek zonder enige
librarywrite. `Apply Sync` blijft disabled tot persistente mirror-, archive- en
diffsemantiek gereed zijn. Daarna volgen de hash-gebonden diffpreview en atomaire
Apply Sync met aantallen voor add, update, remove, archive, restore en fouten. De
volledige beslissing staat in
[`rekordbox-xml-sync.md`](../design/library-sources/rekordbox-xml-sync.md).

## 6.1 Bouwvolgorde zonder Rekordbox-developmentlibrary

De Rekordbox-spike is alleen een harde gate voor `E2A-02`, niet voor de
provider-neutrale Library- en editorfunctionaliteit. De aanbevolen eerste slice
is:

1. `E2A-01`: librarycontract, SQLite en deterministische demo-provider;
2. `E2A-03`: browse en inspecteer demotracks in de echte native Library UI;
3. `E2A-04`: CDJ-editorcanvas en volledige lokale audio-preview;
4. `E2A-05`: bar-aligned phrase editing en revisions;
5. `E2A-06`: Settings > Phrase Model, Library Sources-mapping en editorintegratie;
6. `E2A-07` en `E2A-08`: vier Theme-banks, flexibele variants en loopstrategieën.

`E2A-00` kan parallel starten zodra het geïsoleerde macOS-account met een
wegwerp-Rekordbox-library beschikbaar is. Pas na een go-besluit start `E2A-02`.
De demo-provider blijft daarna bestaan voor CI, screenshots en foutscenario's.

## 7. Exitcriteria

- Een lokaal gevonden Rekordbox 7-library kan zonder handmatige export en zonder
  bronmutatie worden geïmporteerd wanneer Rekordbox gesloten is.
- Minimaal één echte track toont metadata, waveform, beatgrid en sourcephrases.
- Voor toegang tot Rekordbox is dezelfde volledige flow aantoonbaar met
  licentievrije demotracks en zonder productiegegevens.
- De editor toont maatnummers en alle beats boven een gekleurde waveform,
  gekleurde phrases direct eronder en een full-track overview.
- Play/pause/stop, seek/scrub, maatnavigatie, volume en phrase-loop werken zonder
  showstate of audiobestand te muteren.
- De gebruiker kan een eigen aaneengesloten Lumi-timeline maken en na restart
  terugzien.
- Geen phrase-edit kan een boundary tussen beats, gap, overlap of zero-length
  range maken; iedere range eindigt bij het volgende Phrase Point.
- De afgesproken roles, inclusief Breakdown/Buildup 1–3, Synth en Pre-drop, zijn
  configureerbaar en in de editor toepasbaar.
- Roles zijn in Settings toevoegbaar, hernoembaar, ordenbaar en archiveerbaar;
  in-use IDs blijven geldig.
- Iedere phrase gebruikt automatisch zijn gelijknamige Autoloop Category.
- Ongeconfigureerde phrases blijven volledig automatisch.
- Een vaste variant blijft behouden bij een previewswitch tussen minimaal twee
  fixture-Themes; de concrete Autoloop verandert mee met de matrixkolom.
- Trackkleur kan een uitlegbaar voorlopig Theme kiezen; een handmatige
  planinstancekeuze overschrijft dit zonder de tracktemplate te wijzigen.
- Een veranderde Rekordbox-bron kan user edits niet stilzwijgend overschrijven.
- Een geïmporteerde track doorloopt zichtbaar Library -> simulator -> Next plan
  -> dry-run-resolutie.
- Een deterministische fixture van minimaal 10.000 tracks importeert binnen de
  vastgelegde performancebudgetten en blokkeert de UI niet.
- Alle verificatie draait lokaal en in CI zonder decks, SoundSwitch of internet.
- Integrations > Lighting Outputs toont SoundSwitch als target en Lumi als eigen virtuele controller;
  Control One wordt uitsluitend als optionele parallelle controller/DMX-interface
  benoemd.

## 7.1 Geparkeerd na de demo-scope

- `E2A-00` en `E2A-02`: directe Rekordbox 7-import is het eerstvolgende
  werkpakket, maar blijft hard gated op een geïsoleerd macOS-account met
  wegwerp-library. De productie-library wordt nooit als ontwikkelbron gebruikt.
- `E2A-15`: verdere CDJ/Rekordbox RGB-pixelfidelity wordt later hervat.
- `E3-00`: de fysieke SoundSwitch/CoreMIDI-keten uit ADR-0015 is bewezen voor
  virtual MIDI discovery, Bank 1 → AutoLoop 1, parallel Control One-gebruik en
  zichtbare DMX-output. Repetition en disconnect/reconnect blijven open.
- `E3-01`: generaliseer de bewezen bank-delay-slotsequentie naar iedere bank en
  AutoLoop en verbind die later met de operationele live-execution.

## 8. Afhankelijkheden en risico's

- Rekordbox 7-opslag is geen publieke stabiele API; E2A-00 is een harde
  go/no-go-gate voor de directe adapter.
- Phrase- en waveformformats kunnen per Rekordbox-update wijzigen; capability-
  en versievalidatie moeten fail-closed zijn.
- Trackmatching met latere USB/live-identiteiten moet al in identities en
  fixtures voorbereid zijn, maar wordt pas met echte decks bewezen.
- Het ingebouwde SoundSwitch-profiel begrenst iedere bank tot 32 AutoLoop-slots
  en laat iedere bank die slots onafhankelijk aan Phrase Types koppelen.
- Echte SoundSwitch-identiteiten en projectdiffs blijven onbewezen tot de latere
  integration spike.
