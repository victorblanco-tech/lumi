# Story E9-01: Remote Live projection and isolated gateway

- Status: **Implementation complete (`0.6.0-dev-6`); physical isolation acceptance pending**
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

## Implemented evidence

- bounded path-free `remote/v1` DTOs, manifest and cross-language fixtures;
- actual engine snapshot-to-Remote projection regression test;
- projection validation limits for Players, waveform, beatgrid, Hot Cues,
  phrases, plans and text;
- bounded critical queues and latest-value transport-anchor coalescing;
- independent contiguous delivery sequences and mandatory reconnect snapshot;
- gateway client saturation disconnect and metrics;
- fail-closed gateway binary with loopback-only engine configuration.
- a distinct authenticated engine-to-gateway loopback endpoint whose disconnect
  never parks or mutates the running show;
- static projection publication only on actual state/plan changes plus
  coalescible 20 Hz transport anchors for BPM, beat and playhead;
- bounded remote command handoff into the existing reducer with authoritative
  state/plan revision conflicts and no alternate mutation path;
- end-to-end loopback authentication, snapshot and command-result tests;
- desktop polling cannot force waveform projection work into the realtime loop.
- a pinned-rustls LAN listener, persistent installation identity and protected
  device trust store;
- an independently packaged, opt-in Remote Gateway LaunchAgent with
  release-channel Bonjour identity and a protected loopback management API;
- bounded LAN clients, frames, authentication attempts, command queues and
  slow-consumer handling with deterministic regression coverage;
- macOS status, one-use pairing invitation, approval, revocation and Controller
  transfer controls without exposing engine credentials or paths.
- explicit cross-language admin-wire tests for service records, status,
  invitations, approval, revocation and Controller-transfer identifiers.

## Remaining gate

Validate discovery and certificate pinning on a physical iPhone, then run the
combined two-Player, SoundSwitch, Ableton Link and deliberately overloaded
Remote soak. The Gateway remains opt-in until that evidence passes.
