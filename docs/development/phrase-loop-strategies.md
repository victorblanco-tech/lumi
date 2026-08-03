# Per-phrase loop strategies

E2A-08 adds an optional logical Autoloop choice to every Lumi phrase. The
choice is stored on the versioned Lumi phrase timeline and contains only stable
logical IDs. It never stores a SoundSwitch bank, slot, device, MIDI address, or
implicitly selected Theme.

## Strategy model

| Strategy | Stored choice | Resolution behavior |
| --- | --- | --- |
| `AUTO` | No Theme and no Variant | The planner resolves the active Phrase Role against the runtime-selected Theme |
| `FIXED_VARIANT` | Phrase Role ID and Variant ID | Keeps exactly that role-owned matrix row while the Theme changes |
| `THEME_SPECIFIC_EXACT` | One or more Theme-to-Variant overrides | Uses an override only when that Theme is selected; Themes without an override remain automatic |

`THEME_SPECIFIC_EXACT` does not select or prefer a Theme. Theme selection stays
late-bound and is owned by the planner or a future plan-instance override. A
fixed variant is likewise Theme-independent: switching between all four Themes
changes only the matrix column, never its row.

## Invariants and failure behavior

- a Variant must be active and owned by the phrase's current Phrase Role;
- an exact Theme override also requires a populated matrix cell;
- changing a phrase role resets its strategy to `AUTO` so an incompatible row
  can never survive the edit;
- resetting to `AUTO` is always possible, including when catalog data is stale
  or incomplete;
- every mutation requires both the expected phrase-timeline revision and the
  expected Autoloop-catalog revision;
- a catalog change makes older commands stale instead of silently applying an
  obsolete choice;
- a missing fixed cell or exact override fails closed and becomes visible in
  editor status and preflight evidence; explicit choices never jump to another
  variant row.

The timeline stores stable IDs rather than the catalog revision. Editor
snapshots attach the catalog revision used for validation plus `ready`,
`incomplete`, or `stale` status and concrete issues. This preserves the user's
intent across restarts while still forcing the current catalog to validate it.

## Native editor behavior

The Phrase Inspector shows role-filtered active Variants only. `Fixed Variant`
locks one row for every Theme. Each of the four Theme rows can optionally set an
exact override, and removing the final override returns the phrase to `AUTO`.
The `Use Auto` action is always present for a selected phrase. Every accepted
edit appends an immutable timeline revision with reason
`changeLoopStrategy`; the active Live plan remains isolated.

## Verification

- Rust domain tests cover all four Theme switches, precedence, incompatible
  role rejection, missing exact cells, stale catalog revisions, and reset;
- persistence and process tests cover save, engine restart, stale rejection,
  and durable `AUTO` reset;
- Swift decoding rejects a strategy whose row role differs from the phrase;
- native tests verify the structured presentation model and English controls;
- repository visual evidence includes a fixed-variant Phrase Inspector;
- hands-on testing uses the exact Terminal-built `Lumi.app`.
