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

## Reversible migration adapter — `0.4.0-dev-43`

Tot de ondertekende `SMAppService`-promotie gereed is, gebruikt Lumi een
channel-specifieke, niet-geprivilegieerde engine service met een
owner-only loopbacktoken en build-exact service record.

- Een gewone UI Quit beëindigt niet langer de engine en verwijdert daardoor
  niet de CoreMIDI-endpoints terwijl SoundSwitch draait.
- De engine zet zichzelf bij iedere authenticated client disconnect eerst
  fail-safe op Off, stopt MIDI Clock en verlaat Ableton Link. Alleen de
  inactieve virtuele MIDI-endpoints en passieve deck/library-service blijven
  bestaan.
- Heropenen koppelt aan exact dezelfde Dev/RC/Prod-channelengine. Een andere
  build vervangt de oude service gecontroleerd; kanalen delen geen database,
  token, endpoint of service record.
- Deze adapter geeft nog geen automatische login-start of crash-restart. Dat
  blijft expliciet onderdeel van de `SMAppService`-acceptatie voor RC.

Deze tussenstap is noodzakelijk omdat fysieke diagnose een interne
SoundSwitch 2.10.3 Control One/JLC1 device-resetdeadlock aantoonde wanneer
virtuele CoreMIDI-devices tijdens een actieve SoundSwitch-sessie verdwenen of
opnieuw verschenen. Stabiele endpoint ownership verwijdert die normale
UI-lifecycletrigger zonder showveiligheid op te geven.

## Implementatie — `0.4.0-dev-48`

De reversibele adapter is gepromoveerd naar de bedoelde per-user
`SMAppService` LaunchAgent. `launchd` bezit nu de channel-specifieke engine,
start hem met KeepAlive en behoudt hem wanneer de SwiftUI-app stopt. De losse
Rust-executable bevat de door macOS vereiste ingebedde Info.plist; packaging
controleert zowel die Mach-O-sectie als de gebundelde LaunchAgent.

De bestaande Dev-database en owner-only sessietoken blijven op hun bestaande
channelpad. De app koppelt op basis van versie, build, executablepad en SHA-256
en herstelt de verbinding wanneer launchd na een crash een nieuw endpoint
publiceert. ADR-0032 legt de definitieve service- en Link-isolatiegrenzen vast.

## Afgewezen alternatieven

### Root-LaunchDaemon

Afgewezen omdat Lumi geen elevated privileges nodig heeft en juist afhankelijk
is van resources in de gebruikerssessie.

### Alleen een menu-barapp

Afgewezen omdat UI-lifecycle en execution-lifecycle dan gekoppeld blijven.
