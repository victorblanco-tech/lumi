# ADR-0033: Deterministic Live timing and exactly-once AutoLoop output

- Status: **Accepted**
- Date: **2026-08-17**
- Refines: ADR-0023, ADR-0026, ADR-0030, ADR-0031 and ADR-0032

## Context

The proven reference workflow used Beat Link Trigger solely to relay the live
master's tempo and beat phase through Ableton Link. A physical Control One then
selected one SoundSwitch AutoLoop. SoundSwitch owned that AutoLoop's progress
after selection; no component continuously corrected its timeline.

Lumi must automate the discrete selection without changing that ownership
model. During physical development, transport normalization, phrase planning,
predictive Bank preparation, MIDI scheduling, Link lifecycle and recovery were
coordinated by one large output worker. False transport discontinuities,
repeated Link alignment and implicit helper recovery could consequently affect
an otherwise valid SoundSwitch run. These behaviours were implementation
choices, not product requirements.

Wi-Fi remains a supported Pro DJ Link transport. Normal network jitter or
packet reordering must be absorbed before it can become a transport event.

Modern players can also publish CdjStatus less frequently than exact Beat
packets. A large difference from the previous status baseline is therefore not
itself a seek when the exact Beat lane has already reached the same
neighbourhood.

## Decision

Lumi divides Live execution into four one-way responsibilities:

1. **Pro DJ Link adapter** translates network packets into source facts;
2. **Transport Authority** selects one master and produces monotone clock
   snapshots plus confirmed discontinuity events;
3. **Ableton Link Relay** consumes only the selected clock;
4. **AutoLoop Cue Executor** consumes discrete, planned cue deadlines.

The SwiftUI and future iPhone clients observe these components and submit user
commands. Rendering, navigation and client lifecycle are never timing inputs.

## Ableton Link Relay contract

The Link Relay receives only:

- source kind and selected master deck;
- effective master BPM;
- beat within the four-beat quantum;
- deck play state;
- source observation time.

It has no phrase, lighting-plan, Theme, Bank, AutoLoop, operation-state or MIDI
input. In particular, Lumi's Off, Arm, Start and Pause states gate lighting
commands only and never stop or alter an independently enabled Link session.

Moving the pitch/BPM slider on the master CDJ updates SoundSwitch BPM through
Ableton Link in real time while preserving the established Link phase. Normal
beat/status observations never re-anchor Link. Initial acquisition, selected
master handover and stopped-to-playing are the only alignment boundaries in
the phase-1 contract.

A failed helper session enters a degraded state and cannot silently open a
replacement Link peer. Recovery requires an explicit Link disable/enable. At
most one Lumi-owned Link peer may exist.

## AutoLoop ownership and exactly-once rule

For one phrase execution epoch, Lumi may emit:

1. at most one Bank selection when the target Bank is not already prepared;
2. exactly one configured AutoLoop selection;
3. no further MIDI for that cue.

SoundSwitch exclusively owns AutoLoop playback and progress after selection.
Lumi never seeks, advances, rewinds, periodically retriggers or otherwise
corrects the active SoundSwitch AutoLoop.

A new execution epoch is created only by a confirmed phrase boundary, track
load, master handover, hotcue/seek/beatjump landing, Lumi Start while a track is
already playing, or playback resume where the current cue must be selected
again. UI refreshes, Link updates and isolated network packets cannot create an
epoch.

The output offset changes the one future cue deadline only. Before emission a
deadline may be replaced after an effective-BPM change. After emission it is
immutable and no compensating pulse is allowed.

## Transport reconciliation rule

Exact Beat packets own continuous playing progress. CdjStatus supplies the
absolute neighbourhood and may propose a discontinuity, while PrecisePosition
may corroborate an imported Hot Cue. A proposed status landing that aligns
within one beat of the already accepted exact timeline is continuous progress:
it is consumed without incrementing the transport generation. A loop, seek,
hotcue or beatjump landing farther away creates one generation only. A later
PrecisePosition packet that describes that already-accepted landing updates its
baseline without creating a second generation.

This rule prevents a sparse `64 -> 128` status update from masquerading as a
seek when exact Beat packets already place the deck at beat 126/127, while
preserving a real `192 -> 65` loop landing.

## Runtime boundary

Phase 1 extracts Link state from the lighting output worker. Lumi retains one
launchd-owned engine, the existing supervised Pro DJ Link adapter, a separately
executed Link helper and one stable CoreMIDI output actor. This gives strict
failure boundaries without adding a mesh of new macOS services.

## Verification gates

- a master BPM sequence such as `130.000 -> 136.500 -> 128.250` reaches the
  Link provider in order without a hold, phase correction or restart;
- Off/Arm/Start/Pause produces no Link provider command;
- normal playing observations produce one initial alignment and zero later
  phase moves;
- a failed helper does not create another peer until explicit disable/enable;
- one-hour Wi-Fi Pro DJ Link + Link, MIDI-only and combined physical soaks are
  required before release promotion;
- exact cue-count, hotcue and clean-shutdown gates are defined by Epic 5.

## Consequences

- existing Library, USB, plan, mapping, Local Playback and UI functionality is
  retained;
- the current predictive AutoLoop implementation is replaced in later Epic 5
  stories rather than extended with more recovery states;
- Beat Link Trigger remains a behavioural reference and fallback during
  validation, not a Lumi production dependency;
- a platform Wi-Fi driver failure is diagnosed separately from Lumi timing
  correctness and does not make Ethernet a product requirement.
