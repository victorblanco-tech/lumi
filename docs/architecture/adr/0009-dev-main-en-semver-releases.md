# ADR-0009: Dev/main en gecontroleerde SemVer-releases

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

Lumi bestaat uiteindelijk uit een engine, macOS-app en iPhone-app die vanaf
dezelfde bronrevision compatibel moeten worden uitgebracht. Dagelijkse
ontwikkeling mag de productiebranch niet direct wijzigen. App Store Review en
macOS-publicatie vinden bovendien niet atomair plaats.

## Besluit

- `dev` is de standaard integratiebranch.
- `main` representeert uitsluitend releasewaardige productiecode.
- Feature- en fix-PR's worden naar `dev` gesquasht.
- Releases gaan via een merge-PR van `dev` naar `main`.
- Hotfixes vertrekken vanaf en keren terug naar `main`.
- `main` wordt na iedere release/hotfix terug naar `dev` gesynchroniseerd.
- Productversies volgen Semantic Versioning en staan canoniek in `VERSION`.
- Productietags gebruiken `vMAJOR.MINOR.PATCH` en worden niet verplaatst.
- Releasebouw en publicatie zijn afzonderlijke, gecontroleerde stappen.
- macOS en iPhone delen een marketingversie, maar hebben eigen monotone
  buildnummers en distributietiming.

## Consequenties

- `main` blijft een betrouwbare productie-indicator.
- Release- en hotfixflows zijn explicieter dan continuous deployment.
- De branches moeten na iedere release bewust worden gesynchroniseerd.
- CI moet versieconsistentie over Rust en Xcode bewaken.
- iPhone en Mac moeten protocolcompatibel blijven tijdens gefaseerde updates.
- Een solo-ontwikkelaar gebruikt geen verplichte self-review, maar wel PR- en
  CI-gates voor `main`.

## Afgewezen alternatieven

### Alleen trunk-based development op `main`

Afgewezen omdat de gebruiker bewust een afzonderlijke productiebranch en
gecontroleerde releases wil.

### Automatische release op iedere merge naar `main`

Afgewezen omdat signing, notarization, TestFlight en App Store-publicatie
expliciete acceptatie vereisen.

### Versies volledig afleiden uit commits

Afgewezen omdat release-impact en timing een menselijke productbeslissing blijven.
