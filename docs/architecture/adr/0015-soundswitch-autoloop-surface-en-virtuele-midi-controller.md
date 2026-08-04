# ADR-0015: SoundSwitch Autoloop-surface en virtuele MIDI-controller

- Status: **Accepted**
- Datum: **2026-08-04**

## Context

SoundSwitch exposeert Autoloops als vier banks met Autoloop-posities. Control
One kan dezelfde functies fysiek bedienen en in de eerste opstelling tevens de
DMX-interface verzorgen. Lumi moet echter niet onder, achter of via Control One
werken. Lumi is zelf een onafhankelijke controller en moet zonder fysieke
controller bruikbaar blijven.

De SoundSwitch Autoloop-indeling moet vooraf in Lumi herkenbaar en
configureerbaar zijn. Andere workflows kunnen banks als Theme, Genre, Function
of Custom organiseren. Een latere profielbuilder moet nieuwe targets en
indelingen kunnen toevoegen zonder Library- of planneridentiteiten te wijzigen.

## Besluit

De eerste ingebouwde outputpresentatie heet `SoundSwitch Autoloops`.

- Lumi publiceert later een eigen virtuele CoreMIDI-bron.
- SoundSwitch ontvangt Lumi rechtstreeks als MIDI-controller.
- Control One of een andere fysieke controller is een optionele, parallelle
  SoundSwitch-controller en geen Lumi-dependency.
- SoundSwitch bezit de lichtuitvoer en gebruikt de gekozen hardware-interface
  voor DMX. Dat kan Control One zijn; Lumi kent deze downstream hardware niet.
- De ingebouwde surface projecteert vier banks met ieder 32 AutoLoop-posities:
  128 mappings totaal. Acht fysieke buttons zijn per pagina zichtbaar; vier
  pagina's ontsluiten alle posities in een bank.
- Een outputbank heeft een eigen naam en organisatievorm: `Theme`, `Genre`,
  `Function` of `Custom`.
- Een buttonbinding bevat de bank, stabiele buttonpositie, de exacte
  SoundSwitch AutoLoop Name en één Lumi Phrase Type.
- Een zichtbare of door de gebruiker te beheren `Variant Name` bestaat niet in
  het outputprofiel. Een interne ID ondersteunt persistence en een latere
  track-specifieke keuze, maar blijft implementatiedetail.
- De eerste demo-projectie gebruikt één Theme-group per bank. Dat is een preset,
  geen invariant van het generieke outputprofielmodel.
- De `Test Controller` spiegelt exact dezelfde vier banks, vier pagina's en
  acht fysieke buttons per pagina en introduceert geen tweede mappingmodel. Zij
  heet niet `Virtual Control One`.

```text
Beat Link Trigger ── Ableton Link ───────────────> SoundSwitch timing

Lumi Engine ── Lumi Virtual MIDI ──┐
                                   ├────────────> SoundSwitch control
Control One (optional, physical) ──┘

SoundSwitch ── selected DMX interface ──────────> Fixtures
```

## POC-gate

De productie-integratie start pas nadat een lokale POC aantoont dat:

1. SoundSwitch de virtuele Lumi MIDI-bron ontdekt;
2. Lumi minimaal één bank en meerdere Autoloops deterministisch kan bedienen;
3. Lumi en de fysieke Control One gelijktijdig blijven werken;
4. DMX via Control One aantoonbaar fixtures aanstuurt tijdens Lumi-bediening;
5. bankselectie, Note On/Off en minimale switchvertraging zijn gemeten;
6. disconnect en reconnect fail-silent blijven;
7. Lumi zonder feedback geen `CONFIRMED` targetstate claimt.

De POC stuurt nog geen show automatisch aan en bouwt geen profielbuilder.

## Consequenties

- Control One verdwijnt uit het Lumi-domein en blijft alleen als optionele
  externe controller of downstream DMX-interface zichtbaar.
- Settings kan de surface en preflight al met demo-data tonen, terwijl live
  MIDI-acties uitgeschakeld blijven.
- De planner spreekt in Bank/Theme- en Phrase Type-identiteiten. Een optionele
  exacte AutoLoop-keuze gebruikt een interne stabiele mapping-ID; alleen het
  outputprofiel kent buttons en MIDI.
- ShowNET, lasers en andere targets kunnen later een ander outputprofiel krijgen.

## Afgewezen alternatieven

### Lumi bedient Control One

Afgewezen: Control One is geen software-outputprovider maar een gelijkwaardige
controller en eventueel een SoundSwitch DMX-interface.

### Lumi verstuurt rechtstreeks DMX

Afgewezen voor de eerste integratie: SoundSwitch blijft eigenaar van fixtures,
Autoloops en DMX-uitvoer.

### De Control One-layout als domeinmodel gebruiken

Afgewezen: de SoundSwitch Autoloop-surface is het targetmodel; fysieke hardware
mag wisselen of geheel ontbreken.
