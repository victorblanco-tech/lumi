# Story E4-03C: Engine service and data recovery

- Status: **Reconnectable adapter physically verified — SMAppService promotion pending**
- Priority: **P0 Critical**
- Effort: **8**
- Components: Engine, macOS, Persistence, Delivery
- GitHub tracking: [#102](https://github.com/victorblanco-tech/lumi/issues/102)

## User outcome

As a DJ, Lumi's engine keeps its show state independently from the window, the
UI reconnects after a relaunch, and backup, restore or recovery can never race
an active database writer.

## Scope

### C1 — Reconnectable service lifecycle

- implement ADR-0003 with the Dev/RC/Prod-specific service identity;
- support more than one sequential authenticated UI connection without engine
  exit;
- separate window lifecycle from show-engine lifecycle;
- define explicit start, ready, graceful stop, forced-failure and upgrade
  handoff states;
- preserve offline operation and loopback-only authenticated IPC.

### C2 — Graceful shutdown and recovery

- add an engine shutdown handshake and wait for confirmed process/service exit;
- close MIDI, Link, bridge and SQLite in a deterministic order;
- restart crashed helpers with bounded backoff and honest readiness;
- recover UI connection without resetting the operation state silently;
- ensure an incompatible engine/protocol version fails safe with one actionable
  message.

### C3 — Engine-owned backup and restore

- use SQLite's consistent backup mechanism from the owning engine;
- stage restore into a separate location, validate schema, integrity, channel
  and required creative/config data, then replace atomically;
- never copy or replace a live WAL-backed database from Swift;
- preserve the current Dev database during service migration;
- make backup groups (track/phrases, configuration, lighting output) explicit
  in the manifest while retaining a coherent all-data restore.

### C4 — Fault injection

- terminate UI, engine, Java bridge and Link helper independently;
- interrupt backup staging and restore validation;
- inject corrupt, old and future-schema databases;
- test power-loss-equivalent restart at transaction boundaries;
- prove Dev, RC and Prod services and databases remain isolated.

## Acceptance criteria

- quitting/reopening the UI does not stop an active engine or duplicate MIDI;
- a reconnect obtains a revisioned current snapshot without resetting decks or
  plan state;
- graceful stop is confirmed before any database replacement occurs;
- every failed restore leaves the prior database byte-for-byte usable;
- backup taken during normal library activity is internally consistent;
- helper or engine crash produces a non-green status within its freshness
  bound and recovers according to policy;
- three installed channels run side by side with separate service, IPC, MIDI
  and data identities;
- upgrade and rollback instructions are proven on a disposable channel copy.

## Safety constraint

Service migration must be delivered behind a reversible launch adapter. The
existing Dev data is backed up and validated before the first ownership switch;
automatic destructive cleanup is not allowed.

## Dev-35 implementation result

- the Rust engine accepts sequential authenticated UI sessions while retaining
  runtime and operation state between connections;
- the macOS supervisor stores a channel-specific endpoint/token record with
  owner-only token permissions and reconnects to the live process;
- monitor and interactive commands are serialized through one exchange lease,
  preventing actor reentrancy from corrupting protocol framing;
- a real-process test and an interactive Dev UI quit/relaunch prove that the
  engine PID and Armed state survive the window process;
- backup and restore now execute on the library-owning engine through SQLite's
  backup API with integrity/schema checks, atomic staging and rollback;
- WAL backup, corrupt input and live restore/rollback have repository tests.

This is the reversible lifecycle adapter required before service migration. It
does not yet register a login-capable LaunchAgent through `SMAppService`, so
automatic engine restart after an engine crash and that final ADR-0003 service
promotion remain open before RC.

## Dev-43 lifecycle correction and evidence

The temporary app-owned teardown introduced in Dev-40 removed ghost helpers but
also removed and recreated CoreMIDI devices on every UI session. A physical
SoundSwitch 2.10.3/Control One process sample showed that device-list churn can
deadlock SoundSwitch inside its JLC1 reset path even when Lumi's realtime lane
is healthy.

Dev-43 restores reconnectable channel-engine ownership with a stricter safety
contract:

- ordinary Quit and unexpected authenticated-client disconnect both transition
  operation to Off, invalidate output, stop local clock and leave Link;
- the engine and its two virtual MIDI endpoints remain stable and idle;
- UI relaunch attaches to the exact engine PID and current snapshot;
- an incompatible build still retires the old engine before replacement;
- bounded helper cleanup cannot block app termination or leave a ghost Link
  peer;
- a real-engine Swift regression proves Live -> disconnect -> Off -> exact
  endpoint reattach -> explicit shutdown.

Final packaged acceptance used engine PID 49208 before and after UI
Quit/relaunch.
Carabiner exited on Quit, SoundSwitch's Link peer disappeared, the same engine
and Pro DJ Link bridge remained, and SoundSwitch plus Control One stayed
responsive. Relaunch created only a fresh Link helper, restored the 155 BPM
peer and allowed Off -> Arm -> Start without replacing MIDI endpoints.

Still open for RC: register the engine through `SMAppService`, expose its Login
Item state, prove login launch/crash restart and complete the one-hour plus
physical-DMX evidence gates.
