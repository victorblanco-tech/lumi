# Epic 6 – Compiled Light Plans

Status: **Implemented in `0.5.0-dev-4`**

Target: **0.5.0**

## Outcome

Lumi builds a varied, deterministic and explainable full-song lighting plan before
playback. Track color and recent history influence automatic choices, while track
overrides remain authoritative. The existing realtime integration lanes receive
the same immutable physical AutoLoop addresses as before.

## Non-negotiable safety boundary

- No policy evaluation, weighted choice, database access or UI dependency in the
  realtime output lane.
- Existing Pro DJ Link, Ableton Link and exactly-once AutoLoop behavior must pass
  unchanged regression and latency gates.
- Static Look and Color Override execution remains disabled until its lifecycle is
  proven with SoundSwitch; configuration and preview may ship safely.

## Phase plan

### Phase A — Domain, policy and storage

- [E6-01](story-e6-01-light-planning-policy.md): revisioned planning policy,
  candidate rules, modifiers and transactional persistence.

Visible milestone: Light Plans opens with durable defaults and saved rules.

### Phase B — Deterministic compiler

- [E6-02](story-e6-02-deterministic-variation-compiler.md): weighted selection,
  color affinity, repeat protection, reservations and evidence.

Visible milestone: `New variation` generates a different but reproducible plan and
explains each selection.

### Phase C — Desktop workflow

- [E6-03](story-e6-03-light-plans-workspace.md): AutoLoop Rules and Plan Preview
  in a dedicated top-level workspace.

Visible milestone: a user can configure a Phrase Role and preview a real Library
track without opening Lighting Outputs.

### Phase D — SoundSwitch modifiers, safely prepared

- [E6-04](story-e6-04-soundswitch-modifier-capabilities.md): Static Look and Color
  Override catalog/mapping, MIDI-learn addressing and fail-closed capability gate.

Visible milestone: provider resources are nameable and mappable, and their planned
use is visible; automatic execution clearly remains `POC required`.

### Phase E — hardening and release evidence

- [E6-05](story-e6-05-light-plans-quality-gates.md): migration, deterministic
  golden tests, compiler performance, realtime regression, restart persistence and
  headed macOS verification.

Visible milestone: a full simulator track produces a stable compiled plan while
the existing Ableton Link and AutoLoop lanes retain their timing behavior.

## Epic acceptance

1. Rules survive restart and reject stale concurrent edits.
2. `AUTO` never crosses Phrase Roles and always resolves to an existing physical
   mapping.
3. Plans are deterministic for seed + policy revision + track + Theme.
4. Consecutive plans apply configurable repeat protection and show relaxations.
5. Track color can Prefer or exclusively select configured candidates.
6. The preview shows the entire phrase sequence, reason and physical address.
7. Current/next plans are compiled outside realtime execution.
8. Unverified modifiers emit no automatic MIDI.
9. Existing integration regression and latency tests remain green.

## Delivery evidence

- revisioned SQLite policy storage and restart/conflict coverage;
- deterministic compiler coverage for color behavior, overrides, reservations,
  cooldown relaxation and duplicate-plan protection;
- 512-phrase release compilation below the 10 ms budget;
- macOS package, native engine-process and warnings-as-errors app build green;
- four-lane 30-second simulator soak green for Pro DJ Link, Ableton Link,
  AutoLoop MIDI and their combined execution;
- installed Dev app opened with its real 65-track library and exposed the new
  Light Plans destination.

Physical Static Look and Color Override execution is intentionally not part of
this completion claim. Its catalog, rules, MIDI addresses and learn pulses are
implemented, while automatic output remains locked behind the separately
recorded SoundSwitch POC gate.
