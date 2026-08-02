# ADR-0001: Autonome engine en losse clients

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

Lumi moet tijdens een liveshow blijven functioneren wanneer het appvenster wordt
gesloten, de UI opnieuw wordt gestart of de iPhone-verbinding wegvalt. De
beslislogica mag niet afhankelijk zijn van een foreground-app.

## Besluit

Lumi wordt opgesplitst in:

1. een zelfstandige `lumi-engine` zonder UI;
2. een native macOS-client;
3. een native iPhone-client.

De engine is de enige bron van waarheid voor runtime-state, Lighting Plans en
output. Clients sturen versiegebonden commands en consumeren snapshots/events.
Clients bevatten geen essentiële showlogica.

## Consequenties

- Een UI-crash of gesloten venster stopt de show niet.
- Meerdere clients kunnen dezelfde engine veilig bedienen.
- IPC en protocolversionering worden expliciete architectuuronderdelen.
- Lokale ontwikkeling en packaging zijn complexer dan bij één monolithische app.
- De engine moet zelfstandig lifecycle, logging en herstel afhandelen.

## Afgewezen alternatieven

### Alle logica in de macOS-app

Afgewezen omdat sluiten of crashen van het venster dan ook de automation stopt.

### De iPhone als tweede bron van waarheid

Afgewezen omdat netwerkverlies dan tot conflicterende of onvolledige show-state
kan leiden.
