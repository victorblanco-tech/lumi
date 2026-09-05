# E10-07 — Stable Remote Controller ownership

Status: In progress · target Lumi 0.6.2-dev-6 / Remote 0.1.1-dev-3

The user observed a Simulator becoming view-only during Mac mode switching.
The dev-5 test already recorded it as Observer; that observation does not
establish why or when ownership changed. Do not claim a historical cause
without evidence.

## Contract

- The first pairing may automatically become Controller. Subsequently only an
  explicit Mac transfer changes the owner. Revocation leaves no owner; another
  connection must not silently take over.
- Offline owners retain ownership. Mac show modes, engine/gateway restarts,
  reconnect ordering and client versions do not select another owner.
- Persisted ownership is authoritative. Registry and live command permissions
  change atomically; failed persistence changes neither. Existing pairings and
  old clients remain compatible.
- Show connection health separately from Controller / View only. Show the
  controlling device and Remote version without enlarging the Live header.
- Record bounded, credential-free ownership transitions with their reason.
- Do not modify lighting, tempo or waveform behavior, or install on the physical
  iPhone as part of this increment.

## Acceptance

Production gateway tests: two clients; both reconnect orders; simultaneous
authentication; persistence/restart; explicit transfer/revoke; re-pairing; failed
saves; no implicit takeover. Native Mac/Simulator checks: mode changes, explicit
transfer, reconnect, disabled Observer controls, readable connection details and
version. Preserve the original physical-iPhone ownership after testing.

Record measured results and limitations before completion. Follow Epic 10 HQ
screenshot rules; leave the latest numbered Mac build open and push to dev.
