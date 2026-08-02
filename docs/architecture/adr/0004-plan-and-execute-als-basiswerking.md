# ADR-0004: Plan-and-execute als basiswerking

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

Realtime een theme en loop kiezen op de phrasegrens is computationeel haalbaar,
maar maakt gedrag minder controleerbaar. Metadata kan ontbreken, configuratie
kan ongeldig zijn en de DJ heeft geen gelegenheid om keuzes vooraf te zien of
aan te passen. Lichtoperators werken juist vooruit op basis van de track die op
het andere deck geladen staat.

## Besluit

`TrackLightingPlan` wordt het centrale domeinobject.

Wanneer een track wordt geladen, compileert en valideert Lumi vooraf:

- het theme en de bank;
- een concrete loop per phrase-instance;
- outputacties en timing;
- fallbacks;
- beslisredenen en revisions.

De gebruiker kan het voorstel aanpassen en keuzes locken. Na geslaagde preflight
krijgt het plan status `READY`. De Execution Engine voert op phrasegrenzen alleen
vooraf geplande cues uit.

## Consequenties

- `Live` en `Next` worden primaire productconcepten.
- De hot path is klein en voorspelbaar.
- Randomselectie wordt vooraf geconcretiseerd en verandert niet onverwacht.
- Een plan kan volledig worden gepreflight voordat het output mag sturen.
- Track-load-instance, metadatarevision en configuratierevision zijn vereist.
- Veranderingen in context vereisen replanning en rebase van user-locks.
- Er is expliciet fallbackgedrag nodig wanneer een track live gaat voordat zijn
  plan gereed is.

## Afgewezen alternatief

### Volledig reactief selecteren op iedere phrasegrens

Afgewezen als basiswerking vanwege de slechtere uitlegbaarheid, beperkte
voorbereiding en grotere kans op verrassingen tijdens een overgang.
