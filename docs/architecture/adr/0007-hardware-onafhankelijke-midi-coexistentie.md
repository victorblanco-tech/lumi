# ADR-0007: Hardware-onafhankelijke MIDI-co-existentie

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

De eerste gebruiker bedient SoundSwitch ook met Control One, maar andere
gebruikers kunnen andere of geen MIDI-controllers hebben. Handmatige hardware-
bediening moet direct blijven werken. Een controllerintegratie in Lumi zou
hardwareafhankelijkheid, feedbackproblemen en statekoppeling introduceren.

## Besluit

Lumi koppelt standaard niet met een fysieke controller. Lumi en de controller
zijn onafhankelijke MIDI-besturingsbronnen voor SoundSwitch.

- De core produceert semantische lichtacties.
- Een configureerbaar SoundSwitch-outputprofiel vertaalt die naar MIDI.
- Een fysieke controller mag SoundSwitch rechtstreeks bedienen.
- Een handmatige actie wint direct en blijft geldig tot Lumi opnieuw output
  stuurt.
- Lumi stuurt standaard alleen bij phrasegrenzen.
- Bij iedere phrasegrens past Lumi bank en Autoloop volledig opnieuw toe.
- `Pause` voorkomt dat Lumi op volgende phrasegrenzen terugneemt.
- Optionele lifecycle-MIDI-input is generiek gemapt en geen deviceprofiel.

De provider- en transportgrens voor Lumi's eigen output is afzonderlijk
vastgelegd in [ADR-0011](0011-provider-onafhankelijke-midi-output.md).

## Consequenties

- Control One, APC, Launchpad en andere controllers kunnen naast Lumi bestaan.
- Lumi hoeft externe controllerstate niet te observeren.
- De daadwerkelijke SoundSwitch-state kan tussen phrasegrenzen afwijken van
  Lumi's geplande state; dit is bewust gedrag.
- Iedere boundarycue moet self-contained zijn en de bank opnieuw selecteren.
- Automatische subphrase-uitvoer is standaard uitgesloten omdat die een
  handmatige override voortijdig zou kunnen beëindigen.
- Static Looks die buiten Lumi worden geactiveerd zijn alleen betrouwbaar te
  resetten als SoundSwitch daarvoor een gemapte generieke actie biedt.

## Afgewezen alternatieven

### Lumi als verplichte MIDI-proxy voor iedere controller

Afgewezen vanwege hardwarekoppeling en het risico dat specifieke feedback- of
displayfuncties niet correct worden doorgestuurd.

### Externe controllerstate als bron van waarheid

Afgewezen omdat feedback niet op ieder apparaat of in iedere SoundSwitch-
configuratie beschikbaar is.
