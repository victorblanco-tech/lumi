# ADR-0008: Operationele toestanden

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

Het oorspronkelijke ontwerp mengde automationmode, simulator, dry-run, hold en
outputstatus. Voor veilig livegebruik moet ondubbelzinnig zijn of Lumi alleen
leest en plant of ook daadwerkelijk MIDI mag versturen.

## Besluit

De showsessie krijgt één expliciete operationele state:

| State | Sources | Planning | MIDI-output |
|---|---:|---:|---:|
| `OFF` | uit | uit | geblokkeerd |
| `ARMED` | aan | aan | geblokkeerd |
| `LIVE` | aan | aan | actief op phrasegrenzen |
| `PAUSED` | aan | aan | geblokkeerd |

De usercommands zijn:

- `Arm`: van `OFF` naar `ARMED` na startup/preflight;
- `Start`: van `ARMED` of `PAUSED` naar `LIVE`, standaard effectief vanaf de
  volgende phrasegrens;
- `Pause`: van `LIVE` naar `PAUSED`, zonder SoundSwitch-look te wijzigen;
- `Off`: naar `OFF`, zonder blackout of resettrigger;
- `Take Over Now`: expliciete onmiddellijke toepassing van de actuele cue in
  `LIVE`.

Simulator en dry-run zijn orthogonale source/outputconfiguraties, geen
operationele states. Theme Lock en user-planlocks zijn planningsregels, geen
operationele states.

## Consequenties

- De outputgate is eenvoudig zichtbaar en testbaar.
- `Arm` kan volledig preflighten zonder showrisico.
- `Pause` laat planning voor de volgende track doorgaan.
- Start midden in een phrase veroorzaakt standaard geen abrupte lookwissel.
- De UI moet `Start pending until next phrase` apart zichtbaar maken.
- Een ontbrekend MIDI-device blokkeert `LIVE`, maar niet `ARMED`.

## Afgewezen alternatief

### Overlappende flags voor Auto, Hold, Manual en Dry Run

Afgewezen omdat meerdere combinaties een onduidelijke of tegenstrijdige
outputstatus opleveren.
