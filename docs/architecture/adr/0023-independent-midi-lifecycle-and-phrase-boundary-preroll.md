# ADR-0023: Independent MIDI lifecycle and phrase-boundary pre-roll

- Status: **Accepted**
- Date: **2026-08-08**

## Context

The first complete Local Playback → SoundSwitch → DMX run proved the product
concept, but also exposed two production-critical coupling problems:

- a failure in the continuous `Lumi Clock` route stopped the separate
  `Lumi Virtual MIDI` lighting-command source;
- every phrase command selected a bank and then waited 50 ms before sending the
  AutoLoop, so discovering a phrase boundary at the boundary made the light
  change late.

Library navigation and status snapshots also share the engine command channel
with Local Playback observations. UI work may not starve the observations that
drive lighting execution. At the same time Lumi must reassert the planned bank
at every phrase: a parallel physical Control One may have changed SoundSwitch's
active bank since the preceding Lumi cue.

## Decision

### Independent lifecycle

`Lumi Virtual MIDI` and `Lumi Clock` remain separate CoreMIDI sources and now
also have independent failure lifecycles.

- The engine best-effort publishes both sources automatically at product
  startup. Publishing sends no MIDI notes, clock ticks or light changes.
- A clock error never stops the lighting-command source.
- A lighting send failure stops only that command source, keeps audio, plans and
  engine state usable, and appears as a degraded Tech status.
- While automatic publication is enabled, the command source retries at most
  once per second. An explicit user Stop disables this retry until Publish or a
  new application session.
- Hardware-output errors fail closed but do not fail or stall the authoritative
  transport command.

### Phrase-boundary timing

For every executable phrase, including two successive phrases in the same
bank, Lumi sends:

1. the planned bank pulse 50 ms before the target light moment;
2. the planned AutoLoop pulse at the target light moment.

The bank is deliberately not cached as authoritative because Control One can
override it between Lumi cues. A signed user timing offset from -250 through
+250 ms moves the final AutoLoop moment: negative is earlier, positive is later.
The fixed 50 ms SoundSwitch bank pre-roll is internal and does not change that
user-facing meaning.

Local Playback samples its native audio transport every 10 ms. The latest
sample for each deck is coalesced, and at most one sample per deck is flushed
before queued Library or UI commands acquire the serial engine channel. Status
snapshot decoding runs away from the UI/transport lane and discards stale
snapshot sequences.

The same timing-offset policy is provider-neutral. Local Playback predicts from
its native audio clock. Direct Pro DJ Link predicts from exact beat packets and
the Lumi-owned Rekordbox beat grid, so a negative offset can leave before the
phrase boundary without sleeping or blocking the engine lane. A positive
offset is emitted by the same bounded scheduler after the boundary.

Development builds through `0.4.0-dev-30` exposed the inverse sign. The
preference is migrated once so an existing physical compensation keeps the
same timing while its displayed sign changes.

## Consequences

- Opening Library and returning to Live cannot disable lighting because of a
  clock-route failure.
- Tech Ready includes both the lighting command source and playback clock;
  source name, auto-publish, errors, active bank, pulse count, bank pre-roll and
  timing offset are inspectable.
- The Live header exposes a compact ±5 ms adjustment; Settings persists the
  same default for future sessions.
- Entering Start while a Master track is already playing schedules the current
  phrase exactly once. If its Bank is not ready yet, Lumi settles the Bank and
  emits on the first following exact beat; it never waits silently for the next
  phrase boundary.
- A negative offset cannot retroactively move that initial Start pulse into the
  past. Predictive offset timing applies to the following known boundary.
- Direct connected-deck timing remains subject to physical SoundSwitch/DMX
  acceptance even though its deterministic scheduling contract is shared with
  Local Playback.

## Rejected alternatives

### Cache the last Lumi-selected bank

Rejected because the physical controller can change SoundSwitch state without
notifying Lumi. The next phrase must reassert the complete planned address.

### Stop lighting commands when MIDI Clock fails

Rejected because sparse commands and continuous timing are separate failure
domains. Losing one cannot silently remove the other.

### Let Library commands always outrank transport updates

Rejected because visible UI work is less important than phrase-boundary output
timing during Live playback.
