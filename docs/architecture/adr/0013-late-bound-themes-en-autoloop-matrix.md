# ADR-0013: Late-bound Themes en de Autoloop-matrix

- Status: **Accepted**
- Datum: **2026-08-03**

## Context

Lumi moet grotendeels automatisch blijven werken, terwijl een gebruiker voor
een belangrijke trackphrase optioneel een specifieke Autoloop-variant kan
voorbereiden. Themes zijn niet noodzakelijk vooraf bekend: de engine kan ze op
basis van trackkleur en andere regels kiezen, en de gebruiker moet het
voorgestelde Theme vlak voor de transitie via Mac of iPhone kunnen wijzigen.

SoundSwitch organiseert Autoloops in banks. De gebruiker richt iedere
Theme-bank in met dezelfde phrase- en variantstructuur, maar de concrete
lichtinhoud verschilt per Theme. Een tracktemplate mag daarom niet aan één bank
of concrete MIDI-locatie worden vastgezet.

## Besluit

Een Lumi-phrase heeft één `PhraseRoleId`. Die role bepaalt rechtstreeks de
bijbehorende Autoloop Category; er bestaat geen afzonderlijke category-override.
Wie een Synth-Autoloop wil, maakt de phrase `Synth`. De startset is:

- `Intro / Outro`;
- `Bridge`;
- `Breakdown 1`, `Breakdown 2`, `Breakdown 3`;
- `Synth`;
- `Pre-drop`;
- `Buildup 1`, `Buildup 2`, `Buildup 3`;
- `Drop`.

De set is uitbreidbaar en namen zijn aanpasbaar, maar IDs blijven stabiel.
Levels `1`, `2` en `3` drukken intensiteit uit en zijn niet verplicht of
automatisch opeenvolgend; de tracktimeline bepaalt de volgorde.

Het logische Autoloop-catalogusmodel is een matrix:

```text
                        Theme A       Theme B       Theme C
Breakdown 1 / variant 1 concrete loop concrete loop concrete loop
Breakdown 1 / variant 2 concrete loop concrete loop concrete loop
Synth / variant 1       concrete loop concrete loop concrete loop
Drop / variant 1        concrete loop concrete loop concrete loop
```

De rij wordt geïdentificeerd door `PhraseRoleId + VariantId`; de kolom door
`ThemeId`. Een targetprofiel vertaalt de gekozen cel later naar een concrete
providerbinding zoals SoundSwitch-bank en -slot. Banks, slots en MIDI-noten zijn
geen library- of planneridentiteiten.

Een trackphrase bewaart geen vast Theme. De library ondersteunt drie
loopstrategieën:

1. `AUTO`: de planner kiest na Theme-selectie een geldige variant;
2. `FIXED_VARIANT`: de track bewaart een theme-onafhankelijke matrixrij;
3. `THEME_SPECIFIC_EXACT`: optionele geavanceerde overrides per Theme, zonder
   dat die override het Theme zelf kiest.

Het Theme wordt per `trackLoadInstanceId` tijdens planning gekozen. De
standaardprecedence is:

1. globale Theme Lock;
2. handmatige keuze voor de actuele planinstance via Mac of iPhone;
3. `FORCE`-regels op genormaliseerde metadata;
4. gewogen `PREFER`-regels, waaronder trackkleur;
5. rotatie en no-repeat;
6. het standaardtheme.

Regels consumeren provider-neutrale facts. Rekordbox-kleur is de eerste
praktische input, maar de Theme Engine blijft uitbreidbaar met playlist, genre,
tags, energie of toekomstige adapterdata. Iedere beslissing bevat een reason en
configuration revision.

Een Theme-wijziging op `Next` maakt een nieuwe planrevision en resolveert alle
concrete Autoloops opnieuw. `FIXED_VARIANT` blijft dezelfde matrixrij gebruiken.
Bij activatie wordt het plan atomair bevroren. Een expliciete wijziging op een
live track kan uitsluitend via een afzonderlijk command op een veilige volgende
boundary ingaan.

Als een matrixcel ontbreekt, blijft de planner binnen dezelfde Phrase Role. Hij
kiest een andere geldige variant of een geconfigureerde role-fallback en markeert
de reason. Lumi vervangt nooit stilzwijgend `Synth` door `Breakdown` of `Drop`.

## Epicgrens

De Library-epic bouwt het logische catalogusmodel, fixtures, editorstrategieën,
Theme-regels en dry-run-resolutie. Het uitlezen van een echt SoundSwitch-project,
detecteren van gewijzigde Autoloops en binden aan bank, slot en MIDI wordt in de
latere SoundSwitch integration epic uitgevoerd achter hetzelfde contract.

## Consequenties

- De Library Editor bevat geen verplichte of persistente Theme-keuze.
- Phrase Role en Autoloop Category kunnen niet inconsistent worden.
- Een vaste variant blijft bruikbaar wanneer het runtime-Theme verandert.
- Iedere Theme-bank hoort dezelfde logische matrixstructuur te implementeren.
- Onvolledige Themes zijn zichtbaar en blokkeren of degraderen tijdens preflight.
- Een concrete SoundSwitch-reorganisatie vereist alleen een targetprofielrefresh,
  niet het aanpassen van alle tracks.
- iPhone- en Mac-edits wijzigen standaard alleen de actuele planinstance.

## Afgewezen alternatieven

### Theme vast opslaan in iedere track

Afgewezen omdat dit kleurregels, rotatie en last-minute tuning onnodig beperkt.

### Een phrase onafhankelijk aan een andere Autoloop Category koppelen

Afgewezen omdat een `Breakdown`-phrase met category `Synth` dubbelzinnig is. De
gebruiker verandert in dat geval de Lumi-phrase zelf naar `Synth`.

### Tracks rechtstreeks aan bank en slot koppelen

Afgewezen omdat SoundSwitch-reorganisatie dan alle tracktemplates breekt en
providerdetails in de library lekken.

### Theme pas op de phrasegrens bepalen

Afgewezen omdat creatieve selectie buiten de execution hot path hoort te blijven.
