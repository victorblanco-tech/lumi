# ADR-0005: Geserialiseerde state machine

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

Deck-, beat-, phrase-, UI-, netwerk- en MIDI-events kunnen gelijktijdig
binnenkomen. Ongecoördineerde mutatie van runtime-state kan dubbele triggers,
trackcounterfouten en inconsistente Lighting Plans veroorzaken.

## Besluit

Alle domeinevents worden verwerkt door één begrensde eventqueue en één
single-writer reducer. Adapters mogen concurrent I/O uitvoeren, maar wijzigen
nooit rechtstreeks de domeinstate.

De reducer produceert:

1. een nieuwe immutable state;
2. gestructureerde beslisredenen;
3. nul of meer effects voor outputworkers.

Effectresultaten komen als nieuwe events terug de reducer in.

## Consequenties

- Beslissingen zijn reproduceerbaar en eenvoudig testbaar.
- Race conditions worden sterk beperkt.
- Langdurige I/O mag nooit in de reducer plaatsvinden.
- Queues moeten begrensd zijn en expliciet overloadgedrag krijgen.
- De volgorde van events en commands wordt onderdeel van het protocol en de
  tests.
- Monotone tijd wordt gebruikt voor runtime-intervallen; wall-clocktijd alleen
  voor presentatie en logging.

## Afgewezen alternatief

### Gedeelde mutable state met locks per adapter

Afgewezen vanwege lockcomplexiteit, moeilijk reproduceerbare fouten en de kans
op dubbele output.
