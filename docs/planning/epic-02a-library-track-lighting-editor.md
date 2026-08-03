# Epic 2A – Music Library and Track Lighting Editor

Status: **Ready for build review**

Doelmilestone: **0.2.0 – Deck Intelligence**

## 1. Productresultaat

Epic 2A maakt van de lege `Library`-navigatie een bruikbare offline
voorbereidingsomgeving. Zonder decks of SoundSwitch-koppeling kan de gebruiker:

1. een lokale, gesloten Rekordbox 7-library veilig read-only importeren;
2. echte tracks, playlists, metadata, waveform, beatgrid en bronphrases zien;
3. luisteren, scrubben en de tracktimeline op maten en beats bewerken;
4. eigen Lumi-phrases maken en roles toewijzen;
5. per phrase automatische selectie behouden of een vaste logische variant
   kiezen;
6. een trackkleurregel een voorlopig Theme laten kiezen;
7. het Theme in een preview wijzigen en alle concrete dry-run-Autoloops opnieuw
   laten resolven;
8. een echte librarytrack in de bestaande dual-deck simulator testen;
9. bronwijzigingen vergelijken zonder Lumi-edits te verliezen.

Dit levert een zichtbare vertical slice over import, persistence, native UI,
editing en planning. De echte SoundSwitch-project- en bank/slotbinding valt
buiten deze epic.

## 2. Scope

### In scope

- alleen Rekordbox 7-detectie en -import op macOS;
- import uitsluitend wanneer Rekordbox gesloten is;
- consistente read-only snapshot en versie/capabilityvalidatie;
- provider-neutraal `MusicLibrarySourceProvider`-contract;
- stabiele trackidentiteit en matchingfacts voor toekomstige live providers;
- lokale SQLite-opslag achter een repositorypoort;
- importbaselines en versioned Lumi phrase-timelines;
- playlistbrowser, search, importstatus en track-readiness;
- waveform, beatgrid, lokale audio-preview en scrub/playhead;
- create, split, merge, move, delete en role change;
- configureerbare phrase roles en Rekordbox-initiële mapping;
- default roles uit ADR-0013;
- logische `Theme × Phrase Role × Variant`-matrix met fixtures;
- `AUTO`, `FIXED_VARIANT` en `THEME_SPECIFIC_EXACT`;
- provider-neutrale kleurregels en uitlegbare Theme-selectie;
- planinstance Theme-override in preview, zonder library-Theme te muteren;
- simulatorintegratie met echte geïmporteerde tracks;
- diff, rebase, merge en replace bij sourcewijzigingen;
- contract-, parser-, persistence-, UI-, golden-, performance- en end-to-endtests.

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
- AI-audioanalyse.

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
- editor: uitgelijnde waveform-, beatgrid-, phrase- en loopstrategylanes;
- inspector: role, start/endbeat, origin, revision en loopstrategie;
- preview: voorlopig Theme, reason en opgeloste dry-run-Autoloop per phrase;
- acties: Refresh, Load on Deck A/B, Compare source en revision history.

Alle controls gebruiken het bestaande Lumi Design System, dark/light appearance,
Engelse localization resources en de configureerbare Camelot/Classic-keynotatie.

### 4.2 Veilige editingregels

- iedere beat behoort aan precies één phrase;
- standaard snapping op maatgrenzen, expliciet verfijnbaar per beat;
- ongeldige overlaps, gaps en zero-length phrases worden geweigerd;
- iedere mutatie maakt een nieuwe revision;
- split erft de role aan beide kanten, behoudt een exacte keuze links en zet het
  nieuwe rechterdeel op `AUTO`;
- bronrefresh overschrijft nooit user revisions;
- edit van een actieve Live-track wijzigt het reeds actieve plan niet.

## 5. Theme- en matrixgedrag

- trackkleur is de eerste configureerbare Theme-fact;
- een kleur kan een Theme forceren of een gewogen kandidaatset prefereren;
- rotatie en no-repeat blijven actief wanneer geen keuze wordt geforceerd;
- een preview- of toekomstige iPhone-keuze muteert alleen de planinstance;
- Theme-wijziging resolveert alle concrete varianten opnieuw;
- `FIXED_VARIANT` blijft dezelfde matrixrij gebruiken;
- ontbrekende cellen blijven binnen dezelfde Phrase Role en worden zichtbaar als
  fallback of preflightprobleem;
- iedere Theme- en variantkeuze heeft een machineleesbare reason.

## 6. Story map

### E2A-00 – Prove safe Rekordbox 7 analysis extraction

Timeboxed research naar detectie, consistente snapshot, trackidentiteit,
waveform, beatgrid, kleur en raw phrases op de daadwerkelijk geïnstalleerde
Rekordbox 7-versie. Levert fixtures, parserproof en een go/no-go-besluit.

### E2A-01 – Persist the canonical Lumi music library

Introduceert sourcecontracten, stabiele identities, SQLite-repository,
migraties, importbaselines en revision-safe transacties.

### E2A-02 – Import a closed local Rekordbox 7 library

Bouwt detectie, read-only snapshot, incremental import, providerstatus en een
zichtbare import/refreshflow in de macOS-app.

### E2A-03 – Browse and inspect imported tracks

Activeert de Library-workspace met playlists, search, filters, metadata,
readiness, errors en importprovenance.

### E2A-04 – Render waveform and audition local audio

Toont waveform en beatgrid en ondersteunt lokaal play, pause, seek, scrub en
phrase-looppreview zonder bestanden te kopiëren.

### E2A-05 – Own and edit versioned Lumi phrase timelines

Maakt de Lumi-timeline autoritatief en implementeert split, merge, boundary move,
create, delete, undo/redo, revisions en validatie in engine en native UI.

### E2A-06 – Configure phrase roles and initial source mapping

Levert stabiele maar hernoembare roles, de afgesproken defaults, Rekordbox
raw-to-Lumi mapping en per-phrase role editing.

### E2A-07 – Build the logical Theme/role/variant matrix

Introduceert de provider-neutrale catalogus, consistente matrixrijen, Theme-
fixtures, coverage/preflight en veilige role-fallbacks zonder SoundSwitch-types.

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

### E2A-11 – Run imported tracks through the simulator

Laadt een echte geïmporteerde track op Deck A/B, gebruikt uitsluitend de
Lumi-timeline en bewijst phrase- en matrixresolutie in de bestaande Live/Next UI.

### E2A-12 – Prove Epic 2A end-to-end

Levert golden import/editor/preview/reimportfixtures, grote-librarybenchmarks,
fault injection, architecture checks, visuele evidence en een gedocumenteerde
demo met bekende beperkingen.

## 7. Exitcriteria

- Een lokaal gevonden Rekordbox 7-library kan zonder handmatige export en zonder
  bronmutatie worden geïmporteerd wanneer Rekordbox gesloten is.
- Minimaal één echte track toont metadata, waveform, beatgrid en sourcephrases.
- De gebruiker kan een eigen aaneengesloten Lumi-timeline maken en na restart
  terugzien.
- De afgesproken roles, inclusief Breakdown/Buildup 1–3, Synth en Pre-drop, zijn
  configureerbaar en in de editor toepasbaar.
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

## 8. Afhankelijkheden en risico's

- Rekordbox 7-opslag is geen publieke stabiele API; E2A-00 is een harde
  go/no-go-gate voor de directe adapter.
- Phrase- en waveformformats kunnen per Rekordbox-update wijzigen; capability-
  en versievalidatie moeten fail-closed zijn.
- Trackmatching met latere USB/live-identiteiten moet al in identities en
  fixtures voorbereid zijn, maar wordt pas met echte decks bewezen.
- De logische matrix veronderstelt consistente role/variantrijen per Theme.
- Echte SoundSwitch-identiteiten en projectdiffs blijven onbewezen tot de latere
  integration spike.
