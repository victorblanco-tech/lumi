# ADR-0006: Lokale native iPhone-client

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

De DJ moet in de booth de actieve show en het Lighting Plan van de volgende
track kunnen zien en aanpassen zonder steeds naar de Mac te gaan. De app moet
zonder internet blijven werken en netwerkverlies mag de show niet beïnvloeden.

## Besluit

Lumi krijgt een native SwiftUI-iPhone-app die rechtstreeks met de Mac-engine
communiceert via lokaal wifi/LAN.

- Discovery gebeurt via Bonjour.
- Pairing vereist fysieke bevestiging met QR- of eenmalige code.
- De verbinding is versleuteld en het device wordt geauthenticeerd.
- Credentials worden in de Keychain bewaard en kunnen op de Mac worden
  ingetrokken.
- Commands zijn idempotent en revision-aware.
- De Mac-engine blijft de enige bron van waarheid.
- De app bevat een standalone demomodus.
- TestFlight wordt gebruikt voor ontwikkeling; App Store-distributie is het
  productdoel.

## Consequenties

- Er is geen cloudaccount of internetrelay nodig.
- iOS en macOS hebben lokale-netwerktoestemming nodig.
- De UI moet stale state en reconnect zichtbaar en veilig afhandelen.
- App Store Review moet de kernfunctionaliteit via demomodus kunnen beoordelen.
- Een booth-netwerk blijft een operationele dependency voor remote bediening,
  maar niet voor de engine of de show.

ADR-0040 refines this decision with a separately supervised, LAN-facing Remote
Gateway, a scoped remote protocol and explicit backpressure. The loopback engine
endpoint is not exposed directly.

## Afgewezen alternatieven

### Alleen een mobiele webinterface

Afgewezen als primaire booth-interface vanwege de voorkeur voor native lifecycle,
Keychain, discovery, haptics en een App Store-product.

### Cloudbroker tussen iPhone en Mac

Afgewezen vanwege internetafhankelijkheid en extra latency/failure modes.
