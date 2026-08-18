# E6-05 – Light Plans quality gates

Status: **Done** | Priority: **P0** | Effort: **5**

## Acceptance criteria

- SQLite upgrade and restart persistence tests pass from the previous schema.
- Golden tests cover color preference, Only, weights, cooldown relaxation, manual
  override, missing mapping and identical-plan protection.
- Compiler benchmark proves bounded, I/O-free precomputation.
- Existing Pro DJ Link, Ableton Link, Local Playback and exactly-once AutoLoop
  suites pass unchanged.
- Simulator soak covers plan replacement, hotcue/beatjump and app navigation.
- Headed macOS test covers rule editing, preview regeneration and output gating.
- Physical Static Look/Color Override execution remains a separately recorded POC
  gate and cannot block safe AutoLoop variation.
