# E6-04 – SoundSwitch modifier capabilities

Status: **Done (Static Look mapping + physical POC; automatic execution gated)** | Priority: **P1** | Effort: **5**

## User value

As a SoundSwitch user I can prepare Static Looks and Color Overrides as optional
planning variation without risking a stuck or ambiguous live override.

## Acceptance criteria

- Provider-neutral `Atmosphere Modifier` and `Color Modifier` types exist.
- SoundSwitch profile supports a named global 32-slot Static Look surface under
  `Integrations → Lighting Outputs → Static Looks`.
- Static Look slots retain the SoundSwitch four-column order and use 32 unique
  addresses on Channel 12, Notes 64–95.
- Guided Learn advances through all slots; every slot has an explicit Toggle test.
- Rules support Application Rate, Selection Weight, Phrase Roles, track colors,
  cooldown and phrase/track scope.
- `No Override` is explicit in preview.
- Activation and release capabilities are separately verified.
- Automatic execution is impossible until both required capabilities are verified.
- UI labels unverified mappings `POC required`; it never suggests they are live.

## Physical evidence — 2026-08-22

- `Moving Heads OFF` and `Only Lasers` were learned from Lumi into SoundSwitch.
- A second Lumi pulse toggled the selected look off again.
- Selecting a second look replaced the first; two simultaneous active Static
  Looks were not possible.
- Existing policy data and rule references are preserved because the new surface
  projects mappings by provider address rather than generated identifier.

Automatic Static Look selection/output is implemented by
[E6-06](story-e6-06-automatic-static-look-execution.md). This capability story
continues to own the per-slot physical verification gate.
