# ADR-0011: Provider-onafhankelijke MIDI-output

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

SoundSwitch is het eerste lightingtarget van Lumi en wordt via MIDI bediend.
De planner en execution engine mogen echter niet afhankelijk worden van
SoundSwitch-specifieke banks, MIDI-noten, CoreMIDI-poorten of een bepaalde
controller. Later moet een andere MIDI-integratie, lightingapplicatie of
platformimplementatie kunnen worden toegevoegd zonder de Lumi-core te wijzigen.

ADR-0007 bepaalt al dat fysieke controllers buiten Lumi om naast de
automatisering mogen werken. Dit ADR legt de technische outputgrens van Lumi
vast.

## Besluit

Lumi bezit een versiegebonden `LightingOutputProvider`-contract. De core levert
alleen semantische, vooraf gevalideerde lichtacties aan deze outputpoort. De
eerste implementatie is `SoundSwitchMidiOutputProvider`.

De provider:

- declareert capabilities en configuratielimieten;
- valideert of semantische acties uitvoerbaar zijn;
- vertaalt acties met een targetprofiel naar concrete MIDI-messages;
- gebruikt een vervangbare `MidiTransportProvider` om messages te verzenden;
- rapporteert ieder resultaat als genormaliseerd effectevent aan de centrale
  queue;
- ondersteunt preflight en dry-run zonder live output te versturen.

De eerste transportimplementatie is een lokale CoreMIDI-provider op macOS. Het
transport kent MIDI-poorten, bytes, timestamps en verzendresultaten, maar geen
SoundSwitch-banks, themes of phrases. Het SoundSwitch-profiel is configuratie en
bevat de concrete mapping en benodigde sequencing, zoals bankselectie, delay en
Autoloop-start.

```text
Lighting Plan
    -> semantische LightingAction
    -> LightingOutputProvider
    -> targetprofiel
    -> MidiTransportProvider
    -> MIDI-target
```

Er is per showsessie één actieve autoritatieve outputprovider. Een audit- of
dry-runprovider mag dezelfde acties observeren, maar kan de live provider niet
stilzwijgend vervangen. De operationele outputgate (`OFF`, `ARMED`, `LIVE`,
`PAUSED`) staat vóór alle live providers.

## Consequenties

- Planning en execution bevatten geen hardcoded SoundSwitch- of MIDI-details.
- CoreMIDI is een platformadapter, geen dependency van het domeinmodel.
- Een Windows-MIDI-transport of andere lightingtargetprovider kan later worden
  toegevoegd achter dezelfde contracten.
- Providercapabilities worden tijdens planning/preflight gebruikt; een
  unsupported actie wordt niet pas op een phrasegrens ontdekt.
- Providerfouten komen terug als events met command-ID, status en foutreden.
- Geen acknowledgement betekent `ASSUMED` of `UNKNOWN`, nooit automatisch
  `CONFIRMED`.
- De co-existentie en override-regels uit ADR-0007 blijven ongewijzigd.
- De eerste provider bedient SoundSwitch rechtstreeks via Lumi's eigen virtuele
  MIDI-bron. Control One is geen provider of transport, maar hoogstens een
  parallelle controller en/of downstream DMX-interface van SoundSwitch, zoals
  vastgelegd in ADR-0015.

## Afgewezen alternatieven

### MIDI-noten rechtstreeks vanuit de planner versturen

Afgewezen omdat creatieve beslislogica dan gekoppeld raakt aan één profiel,
target en platform.

### SoundSwitch als domeinobject in de core

Afgewezen omdat SoundSwitch de eerste outputintegratie is, niet de definitie van
Lumi's lichtplan.

### Eén interface waarin targetmapping en platform-MIDI niet te scheiden zijn

Afgewezen omdat dezelfde SoundSwitch-mapping anders per besturingssysteem
opnieuw geïmplementeerd moet worden.
