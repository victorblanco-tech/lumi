# ADR 0047 — Explicit Quit stops the channel services

Date: 2026-10-09. Status: implementation in progress; desktop acceptance pending.

## Decision

The owner explicitly requires Quit to mean that all services belonging to that
Lumi channel are stopped. This supersedes ADR 0003's parked-engine policy for
explicit Quit only. A temporary UI transport reconnect remains a separate event.

Settings exposes service control and process details. Shutdown first requests
Off and Link departure, disables the Remote gateway, unregisters the managed
engine and verifies termination. A verified owned engine may receive bounded
TERM/KILL escalation. Unknown process identity is never permission to kill.
Service records are retained on failed shutdown. Quit is asynchronous and a
second Quit must wait rather than bypass the first shutdown. Startup and stop
operations are serialized; automatic reconnection must not resurrect services
during explicit shutdown. A failed stop keeps the app open with actionable state.

Remote enablement is an app preference independent of current registration, so
a successful Quit does not erase the user's intention to use Remote next time.
Pairings, databases, mappings and other channels are preserved.

## Required validation

Previous ADR 0003 evidence associated CoreMIDI endpoint recreation with a
SoundSwitch device-reset deadlock. Therefore real SoundSwitch responsiveness
through repeated stop/start is a required gate, not waived by process tests.
Validate startup interrupted by Quit, repeated Quit, manual stop then Quit,
frozen/failed helpers, old registration/new bundle, and macOS Activity Monitor
process disappearance. Do not claim completion from an unregister return alone.
