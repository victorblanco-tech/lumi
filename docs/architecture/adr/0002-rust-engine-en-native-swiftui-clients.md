# ADR-0002: Rust-engine en native SwiftUI-clients

- Status: **Accepted**
- Datum: **2026-08-02**

## Context

De engine moet snel, resource-efficiënt, voorspelbaar en geschikt voor een lang
draaiend proces zijn. De eerste clients richten zich op macOS en iPhone. Een
Windows-versie is later denkbaar, maar is niet de eerste optimalisatiedoelstelling.

## Besluit

- De engine en domeinlogica worden in Rust gebouwd.
- De macOS- en iPhone-clients worden native in SwiftUI gebouwd.
- Gedeelde Apple-clientmodellen, networking en UI-componenten komen in Swift
  packages.
- Het wire protocol, niet de UI-technologie, vormt de hergebruikgrens voor een
  toekomstige Windows-client.

## Consequenties

- Rust ondersteunt een compacte, type-safe en platformonafhankelijke core.
- SwiftUI geeft native Apple-lifecycle, Keychain-, Bonjour- en
  ServiceManagement-integratie.
- macOS en iPhone kunnen clientcode en UX-componenten delen.
- Het project gebruikt twee talen en twee build-ecosystemen.
- Een latere Windows-interface deelt niet automatisch SwiftUI-code, maar kan wel
  de Rust-core en het protocol hergebruiken.

## Afgewezen alternatieven

### Electron/Node voor engine en UI

Afgewezen vanwege de zwaardere runtime en omdat een permanente engine dan minder
duidelijk van de UI-lifecycle wordt gescheiden.

### Tauri voor macOS en een aparte SwiftUI-iPhone-app

Afgewezen nadat native iPhone-bediening een kernrequirement werd. Native SwiftUI
voor beide Apple-clients beperkt het aantal UI-stacks en verbetert codehergebruik.

### Alles in Swift

Niet onmogelijk, maar afgewezen omdat de domein- en executioncore later ook op
Windows bruikbaar moet kunnen zijn.
