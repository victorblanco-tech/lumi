# Story E5-02: Exactly-once phrase AutoLoop executor

- Status: **Ready after E5-01**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-51`
- GitHub tracking: [#118](https://github.com/victorblanco-tech/lumi/issues/118)

## Outcome

Normal sequential playback emits at most one Bank selection and exactly one
AutoLoop selection per phrase execution epoch. SoundSwitch owns all progress
after that selection.

## Scope and acceptance

- replace the predictive/pending/fallback combination with the explicit states
  `Idle -> Scheduled -> BankPrepared -> Triggered -> Completed`;
- identify an execution by deck, track load, playback epoch, plan revision and
  phrase instance;
- never emit progress, seek, correction or periodic retrigger commands;
- start with zero output offset and normal phrase boundaries only;
- deterministic full-track fixtures prove exact expected MIDI sequences and
  zero duplicates under delayed, repeated and reordered observations.
