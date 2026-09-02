# Story E9-01: Remote Live projection and isolated gateway

- Status: **Planned**
- Priority: **P0 architecture and performance**
- Target: `0.6.0-dev`
- Components: Engine, Remote Gateway, Protocol

## User outcome

The iPhone can receive current Live state while gateway or network behavior can
never delay Pro DJ Link, Ableton Link or SoundSwitch output.

## Scope

- define a bounded remote/v1 manifest and projection DTOs;
- publish immutable Live state outside the reducer;
- split static track/plan detail from latest-value transport anchors;
- implement a separately supervised macOS Remote Gateway;
- keep the engine endpoint loopback-only;
- whitelist remote commands before they reach internal protocol decoding;
- add gateway status to the macOS Integrations surface.

## Acceptance

- no Library, USB, filesystem path, raw log or engine token crosses the gateway;
- slow consumers coalesce visual anchors and cannot block authoritative events;
- killing or saturating the gateway has no output or timing effect;
- encoded sizes, rates, queue depth, drops and disconnects are bounded and
  measured;
- Dev, RC and Production service identities remain isolated.

