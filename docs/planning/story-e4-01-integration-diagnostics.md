# [E4-01 – Expand Integrations Diagnostics with traffic and recovery actions](https://github.com/victorblanco-tech/lumi/issues/89)

Status: **Planned for 0.4.0 – macOS Beta**

## Outcome

`Integrations > Diagnostics` becomes the single technical troubleshooting
workspace for the local Lumi signal chain. Overview remains a compact functional
summary and contains no duplicate repair controls.

## Scope

- bounded live input/output traffic views with timestamps and provider labels;
- recent warnings, validation failures and typed transport errors;
- message/frame counters, last-event age, duplicate/loss indicators and latency;
- safe provider actions such as reconnect input, restart/republish a Lumi MIDI
  endpoint and retry after a typed failure;
- explicit test-frame, MIDI-learn and output-trigger actions that never run
  automatically;
- copy/export a bounded support bundle without track audio or credentials;
- deep link from the compact technical status in Live;
- independent health for Deck Inputs, Library Sources, timing and Lighting
  Outputs.

## Acceptance criteria

- Every action states its target and never starts LIVE operation implicitly.
- Overview remains readable and action-free except for navigation.
- Logs and traffic are bounded so a long set cannot exhaust memory or disk.
- Input recovery cannot emit lighting MIDI; output recovery cannot mutate deck
  state.
- Sensitive local paths and track metadata are redacted from exported support
  evidence unless the user explicitly opts in.
- Failure and recovery paths have deterministic local tests and native desktop
  verification.
