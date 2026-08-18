# E6-04 – SoundSwitch modifier capabilities

Status: **Done (safe preparation; physical output POC deferred)** | Priority: **P1** | Effort: **5**

## User value

As a SoundSwitch user I can prepare Static Looks and Color Overrides as optional
planning variation without risking a stuck or ambiguous live override.

## Acceptance criteria

- Provider-neutral `Atmosphere Modifier` and `Color Modifier` types exist.
- SoundSwitch profile supports named Static Look and Color Override resources with
  configurable MIDI channel/note and learn pulse.
- Rules support Application Rate, Selection Weight, Phrase Roles, track colors,
  cooldown and phrase/track scope.
- `No Override` is explicit in preview.
- Activation and release capabilities are separately verified.
- Automatic execution is impossible until both required capabilities are verified.
- UI labels unverified mappings `POC required`; it never suggests they are live.
