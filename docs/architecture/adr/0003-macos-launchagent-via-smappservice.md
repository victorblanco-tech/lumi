# ADR-0003: macOS LaunchAgent via SMAppService

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

De Lumi Engine moet na login zelfstandig kunnen starten, doorlopen zonder open
venster en na een onverwachte exit kunnen worden herstart. Tegelijkertijd heeft
de engine toegang nodig tot de CoreMIDI- en gebruikerssessie en lokale
gebruikersbestanden.

## Besluit

De engine wordt als niet-geprivilegieerde per-user LaunchAgent in de appbundle
meegeleverd en door de macOS-app via `SMAppService` geregistreerd.

Lumi gebruikt geen root-LaunchDaemon voor de normale runtime.

## Consequenties

- De engine draait binnen de ingelogde gebruikerssessie.
- `launchd` beheert start en herstel.
- De gebruiker kan het achtergrondonderdeel via macOS Login Items toestaan of
  uitschakelen.
- De app moet de registratiestatus zichtbaar maken en een geblokkeerde agent
  duidelijk uitleggen.
- Signing, bundle-layout en installatie moeten de helper correct meenemen.
- Zonder ingelogde gebruiker draait Lumi niet; dat is passend voor een DJ-tool.

## Afgewezen alternatieven

### Root-LaunchDaemon

Afgewezen omdat Lumi geen elevated privileges nodig heeft en juist afhankelijk
is van resources in de gebruikerssessie.

### Alleen een menu-barapp

Afgewezen omdat UI-lifecycle en execution-lifecycle dan gekoppeld blijven.
