# Repository structure

Lumi is a monorepo with explicit ownership boundaries:

```text
apps/       Native user-facing clients
bridges/    Supervised non-Rust provider helpers with versioned local protocols
contracts/  Versioned provider-neutral wire contracts
engine/     Autonomous Rust domain and runtime crates
fixtures/   Deterministic license-safe test and demo data
docs/       Architecture, planning, release, and development documentation
scripts/    Repository-wide validation and delivery entry points
tools/      Development-only external-system simulators and diagnostics
```

## Dependency direction

The pure Rust domain is the innermost boundary. It has no I/O, runtime,
serialization, provider, Apple, or UI dependencies. Engine application code may
depend on the domain; adapters will depend inward on application-owned ports.

Native views depend on presentation state and actions. They must not import
transport or process-supervision implementations. Those integrations map wire
contracts into client-owned models outside the view layer.

`LumiLibraryWorkspace` is the native Library feature boundary. It owns bounded
wire decoding, presentation state, Library navigation, the track table, and the
metadata inspector. The app target composes it with `LumiLiveWorkspace` and
owns the only dependency on `LumiEngineClient`; neither feature package imports
process supervision or another feature.

Wire contracts and fixtures are boundary artifacts, not dumping grounds for
cross-language business logic. Provider-specific deck and lighting types stay
behind their adapters.

`lumi-library-source` owns the provider contract for immutable analysis
baselines from local music-library adapters. `lumi-library` owns canonical
tracks, the application-owned repository port, configurable phrase roles, and
editable phrase timeline revisions. `lumi-library-demo` supplies deterministic,
license-safe development data; `lumi-library-sqlite` is the local persistence
adapter. The later Rekordbox 7 adapter will implement the source port and may
only emit canonical baseline types. Rekordbox storage types and SQL never cross
into Lumi's model.

`lumi-deck-source` owns the application-facing deck observation port.
`lumi-simulator` is one adapter for that port and maps a license-safe fixture
into domain events. `lumi-blt-midi` is the first connected-deck adapter: it
decodes versioned, atomic Beat Link Trigger MIDI frames into the same port.
CoreMIDI owns only endpoint transport and raw channel messages. A future direct
Pro DJ Link adapter can replace BLT without changing the domain or Live UI.

`bridges/prolink` owns the supervised Java helper that uses the pinned
Deep Symmetry `beat-link` dependency. It emits only the versioned local bridge
protocol; the Rust-side adapter performs all translation to Lumi deck
observations. Java, Beat Link and Pro DJ Link types never enter the engine,
domain, protocol or Swift packages.

`lumi-prolink-input` owns strict decoding and validation of that local bridge
protocol. It initially has no domain or engine dependency; the later deck
adapter consumes its immutable messages and implements `DeckSourceProvider`.

`tools/prolink-simulator` owns the development-only Pro DJ Link network player.
It reads a mounted Rekordbox USB without writing, emits only the discovery,
status and beat facts needed by Lumi, and exposes authenticated HTTP controls
for repeatable two-Mac acceptance tests. It is not a deck-source adapter, never
ships in the production DMG and never sends commands to physical players.

`lumi-planner` owns deterministic creative selection and canonical plan
evidence. It depends only on `lumi-domain`; it knows no simulator, transport,
SwiftUI, MIDI, or SoundSwitch types. The engine invokes it synchronously after
a Next load and feeds its result back through the reducer as an effect result.

`lumi-lighting-output` owns the provider-neutral lighting output port.
`lumi-output-dry-run` implements that port without external I/O and records a
bounded execution transcript. Future MIDI or other lighting integrations must
implement the same port; provider details never enter the domain or protocol.

Logical Themes, Phrase Roles, and Variants form a provider-neutral matrix in the
planner boundary. A later SoundSwitch catalog adapter binds matrix cells to
banks and slots; neither the library nor a track template stores those physical
coordinates.

## Naming

Rust crates and modules use specific domain or capability names in idiomatic
`kebab-case` and `snake_case`. Swift types and members follow the Swift API
Design Guidelines. New directories describe one bounded responsibility.

Ambiguous buckets such as `Utils`, `Common`, `Shared`, `Helpers`, and `Misc` are
rejected by repository validation. Reuse alone is not a reason to weaken a
boundary; a shared module needs a narrow, stable API and a concrete owner.
