# Lumi-architectuur

Deze map bevat het actuele architectuurontwerp van Lumi en de Architecture
Decision Records (ADR's) die de belangrijkste besluiten vastleggen.

## Documenten

- [Architectuurdesign](design.md)
- [Functionele architectuurplaten](visual-overview.md)
- [ADR-0001: Autonome engine en losse clients](adr/0001-autonome-engine-en-losse-clients.md)
- [ADR-0002: Rust-engine en native SwiftUI-clients](adr/0002-rust-engine-en-native-swiftui-clients.md)
- [ADR-0003: macOS LaunchAgent via SMAppService](adr/0003-macos-launchagent-via-smappservice.md)
- [ADR-0004: Plan-and-execute als basiswerking](adr/0004-plan-and-execute-als-basiswerking.md)
- [ADR-0005: Geserialiseerde state machine](adr/0005-geserialiseerde-state-machine.md)
- [ADR-0006: Lokale native iPhone-client](adr/0006-lokale-native-iphone-client.md)
- [ADR-0007: Hardware-onafhankelijke MIDI-co-existentie](adr/0007-hardware-onafhankelijke-midi-coexistentie.md)
- [ADR-0008: Operationele toestanden](adr/0008-operationele-toestanden.md)
- [ADR-0009: Dev/main en gecontroleerde SemVer-releases](adr/0009-dev-main-en-semver-releases.md)

Release- en deploymentmanagement staat in [`docs/release`](../release/README.md).

## ADR-statussen

- `Proposed`: voorgesteld maar nog niet definitief aanvaard;
- `Accepted`: leidend voor ontwerp en implementatie;
- `Superseded`: vervangen door een nieuw ADR;
- `Deprecated`: niet meer van toepassing.

Een aanvaard ADR wordt niet stilzwijgend herschreven wanneer het besluit
verandert. In dat geval wordt een nieuw ADR toegevoegd dat het eerdere besluit
vervangt. Kleine verduidelijkingen die de beslissing niet veranderen mogen wel
in het bestaande ADR worden aangebracht.
