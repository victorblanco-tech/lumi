# Story E9-04: Revision-safe remote booth controls

- Status: **Implementation complete; physical multi-device acceptance pending**
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
- authenticated TLS transport from every native action to the gateway;
- separate bounded gateway-to-engine command handoff with no disconnected
  replay or offline queue;
- authoritative accepted/rejected command reconciliation, fixed feedback and
  haptics;
- persisted one-Controller ownership with explicit Mac transfer and revoke.
- headed Simulator acceptance transferred the single Controller lease and sent
  `ARM`, `START` and confirmed `OFF` through the real TLS gateway to the existing
  reducer, with authoritative state reflected back in the iPhone presentation.
- Remote dev-6 exposes that existing safe command path from both the Phrase and
  Light Plan bands, with one Phrase selector and explicit read-only feedback
  for live, completed and Viewer-only selections.
- headed dev-6 acceptance opened the touch sheet on the next planned Intro,
  exposed the complete adjustment surface, and verified that selecting the
  running Drop changed it to `Live · locked` with every mutation disabled.
- dev-7 keeps the blue `NEXT` block as a full-width touch target while its red
  `ACTIVE` predecessor remains inspectable and immutable.

## Remaining gate

Validate lease transfer and duplicate-tap behavior with two physical devices
during a running hardware show. No control is enabled before the complete path
is authenticated and a current snapshot has arrived.
