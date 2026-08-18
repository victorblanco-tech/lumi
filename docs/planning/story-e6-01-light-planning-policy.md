# E6-01 – Revisioned Light Planning Policy

Status: **Done** | Priority: **P0** | Effort: **5**

## User value

As a user I can maintain reusable AutoLoop variation rules without changing my
physical SoundSwitch mappings or individual track phrases.

## Acceptance criteria

- Policy contains Theme, Phrase Role, Variant, enabled state, Selection Weight and
  `Neutral`/`Prefer`/`Only` track-color affinity.
- Defaults preserve current behavior for existing installations.
- Theme, role, AutoLoop and whole-plan repeat windows are configurable.
- Policy uses optimistic revision checks and one SQLite transaction.
- Migration never mutates existing AutoLoop, phrase or track data.
- Policy can be exported in engine state and survives restart.
