# Lumi

Lumi is een lokale lichtautomatiseringslaag voor DJ-sets. De applicatie leest
track-, beat- en phrase-informatie, plant vooraf een lichtshow voor geladen
tracks en voert dat plan via MIDI uit in SoundSwitch.

De architectuur en vastgelegde besluiten staan in
[`docs/architecture`](docs/architecture/README.md).

De drie functionele architectuurplaten staan in
[`docs/architecture/visual-overview.md`](docs/architecture/visual-overview.md).

De GitHub-, versie- en distributiestrategie staat in
[`docs/release`](docs/release/README.md).

Het bouwfase- en werktrackingbeleid, inclusief de rolverdeling tussen Buzz en
GitHub, staat in
[`docs/planning/build-and-tracking-plan.md`](docs/planning/build-and-tracking-plan.md).

Het bouwplan voor de eerste verticale productmilestone staat in
[`docs/planning/epic-01-first-visible-lighting-plan.md`](docs/planning/epic-01-first-visible-lighting-plan.md).

Het gerefineerde ontwerp en storyplan voor de volgende verticale milestone staat
in [`docs/planning/epic-02a-library-track-lighting-editor.md`](docs/planning/epic-02a-library-track-lighting-editor.md).

> Status: `0.4.0-dev-43` is de actieve ontwikkellijn. De geaccepteerde `0.3.0`
> release bevat de Rekordbox-backed Library, Track Lighting Editor, Local
> Playback dual-deck, rolling AutoLoop Plan en de eerste fysiek bewezen
> SoundSwitch/MIDI/DMX-keten.

## Development

Epic 1 contains a working native macOS demo backed by the local Rust engine,
deterministic simulator, editable next-track plan, operational output gate, and
dry-run event timeline. The no-terminal demo and known limitations are in
[`docs/release/0.1.0-demo-and-limitations.md`](docs/release/0.1.0-demo-and-limitations.md).
Environment setup and the single local verification command are documented in
[`docs/development/README.md`](docs/development/README.md).

## License

Copyright © 2026 Victor Blanco. Lumi is available under the
[Eclipse Public License 2.0](LICENSE). Names and branding are covered separately
by [the project branding notice](TRADEMARKS.md); optional external integrations
are listed in [third-party notices](THIRD_PARTY_NOTICES.md).
