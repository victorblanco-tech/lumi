# Logical Autoloop catalog

E2A-07 introduces Lumi's provider-neutral Theme, Phrase Role, and Variant
matrix. It answers which logical Autoloop entry should be selected without
knowing how a lighting provider exposes that entry.

## Matrix model

Each row has the stable key `PhraseRoleId + AutoloopVariantId`. The same rows
appear under exactly four named logical Theme targets. A populated cell points
to a stable logical `AutoloopEntryId` and display name; an empty cell is explicit
missing coverage. Variants belong to one Phrase Role and cannot resolve through
another role. This makes a Synth phrase fail closed instead of silently using a
Breakdown or Drop loop.

The four Theme targets are not four variants. Roles may have any supported
number of variants, and variants can be added, renamed, reordered, archived,
and restored without changing their stable IDs. A newly created Phrase Role is
also a preflight issue until it has an active variant. Each active variant then
requires one cell per Theme.

Resolution accepts the expected catalog revision. An exact populated cell wins.
If it is missing, the resolver may fall back only to another populated variant
of the same role and reports that reason explicitly. If the role has no usable
coverage, resolution fails. Passing an older revision fails before a cached
choice can be reused.

Automatic resolution may use the documented same-role fallback. An explicit
per-phrase fixed Variant or exact Theme override is stricter: it retains the
chosen row and fails closed when that exact cell is missing. The complete
strategy semantics are documented in
[`phrase-loop-strategies.md`](phrase-loop-strategies.md).

## Provider boundary

This catalog contains no SoundSwitch, MIDI, bank, slot, or device addressing.
Names such as `Electric Bloom · Synth · Variant 1` are logical Lumi entries, not
physical bindings. A later target adapter owns the mapping from a logical Theme
and entry to its provider-specific address and validates physical capacity.
Consequently the catalog snapshot declares `validationOwner: targetAdapter`
and `hardCodedPhysicalCapacity: false`.

## Persistence and safety

SQLite schema v4 adds Theme, Variant, and matrix-cell tables plus one-time
defaults and revision markers. Defaults seed exactly four Themes and role-owned
variants. All mutations use optimistic concurrency and are committed atomically.
Archiving is reversible; cells and stable IDs are retained. The Rust engine is
the sole writer, while Swift renders and edits the authoritative snapshot.

Preflight reports both missing cells and active Phrase Roles without an active
variant before activation. Verification covers domain invariants, v3→v4
migration, restart persistence, stale revision rejection, Swift decoding,
Swift↔Rust process mutations, native visual evidence, and the exact
Terminal-launched macOS application.
