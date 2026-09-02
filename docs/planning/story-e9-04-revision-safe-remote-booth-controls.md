# Story E9-04: Revision-safe remote booth controls

- Status: **In progress (command safety contract)**
- Priority: **P0 safety / P1 product**
- Target: `0.6.0-dev`
- Components: iOS Remote Client, Gateway, Engine

## User outcome

The DJ can safely operate Lumi and tune future phrases from the booth without a
double tap, reconnect or stale screen producing a second or incorrect mutation.

## Scope

- `OFF`, `ARM`, `START`, `PAUSE` with authoritative acknowledgement;
- Ableton Link enable/disable and master BPM status;
- bounded lighting offset and pending-next-phrase confirmation;
- Theme-from-phrase, phrase AutoLoop and cue lock/unlock;
- optimistic pending presentation reconciled to engine revisions;
- destructive Off confirmation while Start is active;
- accepted/rejected haptics and one stable feedback region;
- one active Controller lease; other paired clients remain view-only.

## Acceptance

- every mutation is idempotent and revision-bound;
- active and completed phrase edits fail closed;
- disconnect clears pending presentation and queues nothing;
- a plan conflict refreshes before controls are enabled again;
- repeated taps, retransmission and two paired devices cannot apply a command
  more than once.

## Implemented evidence

- explicit booth-safe command allowlist matching Rust and Swift fixtures;
- state, track-load and plan revision contexts;
- bounded offset and AutoLoop validation;
- future-phrase-only plan mutation guard;
- single Controller lease authorization;
- bounded gateway idempotency ledger with duplicate admission result;
- per-target pending command suppression and disconnect cleanup on iPhone;
- accepted, duplicate, conflict and rejected acknowledgements;
- Start-to-Off confirmation in the native Live UI.

## Remaining gate

Route admitted commands through the separate gateway-to-engine command path,
reconcile authoritative acknowledgements in the UI, add haptics and validate
lease transfer/two-device behavior. No control is enabled before this path is
authenticated end-to-end.
