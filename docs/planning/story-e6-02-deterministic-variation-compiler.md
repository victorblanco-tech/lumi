# E6-02 – Deterministic variation compiler

Status: **Done** | Priority: **P0** | Effort: **8**

## User value

As a DJ I get musical lighting variation without surprises or realtime timing
risk.

## Acceptance criteria

- Compiler input is immutable policy + catalog + track template + track color +
  Theme + bounded history + seed.
- Same input produces byte-for-byte equivalent selections and evidence.
- Fixed/exact track choices win; `AUTO` never changes Phrase Role.
- Selection Weight, Prefer/Only colors and all repeat windows are enforced.
- Current and next reservations prevent repetition; replacement releases an
  unexecuted reservation and first execution commits it.
- Every result records policy revision, seed and human-readable evidence.
- Typical 512-phrase compilation is measured below 10 ms on the local quality
  runner and performs no I/O.
