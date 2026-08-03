# Repository structure

Lumi is a monorepo with explicit ownership boundaries:

```text
apps/       Native user-facing clients
contracts/  Versioned provider-neutral wire contracts
engine/     Autonomous Rust domain and runtime crates
fixtures/   Deterministic license-safe test and demo data
docs/       Architecture, planning, release, and development documentation
scripts/    Repository-wide validation and delivery entry points
```

## Dependency direction

The pure Rust domain is the innermost boundary. It has no I/O, runtime,
serialization, provider, Apple, or UI dependencies. Engine application code may
depend on the domain; adapters will depend inward on application-owned ports.

Native views depend on presentation state and actions. They must not import
transport or process-supervision implementations. Those integrations map wire
contracts into client-owned models outside the view layer.

Wire contracts and fixtures are boundary artifacts, not dumping grounds for
cross-language business logic. Provider-specific deck and lighting types stay
behind their adapters.

`lumi-deck-source` owns the application-facing deck observation port.
`lumi-simulator` is one adapter for that port and maps a license-safe fixture
into domain events. Future Beat Link or direct Pro DJ Link adapters implement
the same port; neither the domain nor the client wire model imports their
provider-specific types.

`lumi-planner` owns deterministic creative selection and canonical plan
evidence. It depends only on `lumi-domain`; it knows no simulator, transport,
SwiftUI, MIDI, or SoundSwitch types. The engine invokes it synchronously after
a Next load and feeds its result back through the reducer as an effect result.

## Naming

Rust crates and modules use specific domain or capability names in idiomatic
`kebab-case` and `snake_case`. Swift types and members follow the Swift API
Design Guidelines. New directories describe one bounded responsibility.

Ambiguous buckets such as `Utils`, `Common`, `Shared`, `Helpers`, and `Misc` are
rejected by repository validation. Reuse alone is not a reason to weaken a
boundary; a shared module needs a narrow, stable API and a concrete owner.
