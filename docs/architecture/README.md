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
- [ADR-0010: Provider-onafhankelijke deck-sources](adr/0010-provider-onafhankelijke-deck-sources.md)
- [ADR-0011: Provider-onafhankelijke MIDI-output](adr/0011-provider-onafhankelijke-midi-output.md)
- [ADR-0012: Library-sources en Lumi-owned phrase-timelines](adr/0012-library-sources-en-lumi-owned-phrase-timelines.md)
- [ADR-0013: Late-bound Themes en de Autoloop-matrix](adr/0013-late-bound-themes-en-autoloop-matrix.md)
- [ADR-0014: Beat-quantized Phrase Points and RGB Track Editor](adr/0014-beat-quantized-phrase-points-and-rgb-editor.md)
- [ADR-0015: SoundSwitch Autoloop-surface en virtuele MIDI-controller](adr/0015-soundswitch-autoloop-surface-en-virtuele-midi-controller.md)
- [ADR-0016: Stable deck identity, rolling Live plans and waveform sources](adr/0016-stable-deck-identity-rolling-live-plans-and-waveforms.md)
- [ADR-0017: Product deck sources, Local Playback and safe unmatched tracks](adr/0017-product-deck-sources-local-playback-and-safe-unmatched-tracks.md)
- [ADR-0018: BLT MIDI deck frames and separate CoreMIDI routes](adr/0018-blt-midi-deck-frames-and-separate-coremidi-routes.md)
- [ADR-0019: Rekordbox XML mirror with read-only analysis enrichment](adr/0019-rekordbox-xml-analysis-en-lumi-owned-enrichment.md)
- [ADR-0020: Closed Rekordbox snapshot as authoritative analysis resolver](adr/0020-closed-rekordbox-snapshot-identity-resolver.md)
- [ADR-0021: Shared track timelines and source-specific playback clocks](adr/0021-shared-track-timelines-and-source-specific-playback-clocks.md)
- [ADR-0022: Separate CoreMIDI sources for lighting commands and playback timing](adr/0022-separate-coremidi-command-and-clock-sources.md)
- [ADR-0023: Independent MIDI lifecycle and phrase-boundary pre-roll](adr/0023-independent-midi-lifecycle-and-phrase-boundary-preroll.md)

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
