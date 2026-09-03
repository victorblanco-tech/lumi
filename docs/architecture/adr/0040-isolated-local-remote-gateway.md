# ADR-0040: Isolated local Remote Gateway and scoped iPhone protocol

- Status: **Accepted**
- Date: **2026-09-02**
- Target: **0.6.0**

## Context

ADR-0006 chose a native iPhone client that works directly with the autonomous
Lumi engine over the local network. Since that decision, the production runtime
has been hardened around three show-critical lanes: Pro DJ Link ingestion,
Ableton Link publication and exactly-once SoundSwitch MIDI output. The current
engine control endpoint is authenticated but intentionally loopback-only and its
desktop client uses request/response snapshots.

Exposing that endpoint directly to the LAN would broaden its command surface,
couple an unreliable mobile connection to the engine lifecycle and make a slow
phone capable of creating control-plane work. Polling complete desktop snapshots
would also repeat waveform and plan serialization and risk competing with the
show-critical runtime.

## Decision

Lumi introduces a separate **Lumi Remote Gateway** process as its only LAN-facing
service.

### Process and failure boundary

- The engine keeps listening only on loopback and remains the sole source of
  truth.
- The Remote Gateway is a separately supervised macOS helper with its own
  lifecycle, memory limit, queues and logs.
- Gateway failure, restart, overload or network loss cannot change operation
  state, stop an AutoLoop, change Ableton Link or restart the engine.
- The gateway uses one authenticated loopback command connection and a separate
  read-only Live projection subscription.
- The Mac app keeps one authenticated admin connection to the gateway and
  reuses it for status and management requests. Periodic health observation
  must not create and tear down a new localhost socket on every poll.
- Remote work runs at utility priority. It never executes on the Pro DJ Link,
  timing-output or realtime MIDI threads.

### Remote projection

The engine publishes an immutable, presentation-safe `RemoteLiveProjection`
from its latest authoritative state. Projection work occurs outside the reducer
and contains no Library, USB, Track Editor, raw diagnostics or filesystem data.

The subscription sends:

1. one complete Live snapshot after authentication or reconnect;
2. static deck detail only when track-load, waveform, phrase, Hot Cue or plan
   revision changes;
3. small latest-value transport anchors for visual interpolation;
4. operation, leader, health, offset and plan events when those values change.

Backpressure coalesces transport anchors per Player. Intermediate visual frames
may be dropped; plan, operation and command-result revisions may not. A slow or
stalled remote is disconnected before it can block production state. Every LAN
write and gateway-to-engine command handoff has a finite deadline; exhausted
clients release their bounded connection permit instead of holding capacity.

The iPhone renders smoothly from the newest transport anchor. That visual clock
never becomes a lighting clock and never feeds state back into the engine.

### Discovery, encryption and pairing

- Bonjour advertises `_lumi-remote._tcp` with protocol major, release channel
  and a non-sensitive installation identifier.
- Network traffic uses TLS. The iPhone pins the Mac installation certificate
  during physical pairing instead of trusting an unauthenticated discovered
  endpoint.
- `Pair New iPhone` creates a one-use, short-lived invitation. Its QR payload
  contains the invitation secret and certificate fingerprint, never the engine
  session token.
- A short code is shown on both devices and the Mac user explicitly approves
  the device.
- The gateway issues a random per-device credential. iOS stores it in Keychain;
  the Mac stores only the required verifier and device metadata. Credentials can
  be revoked individually.
- Pairing attempts, authentication failures and command rates are bounded. No
  cloud relay, account or internet fallback is introduced.

Production, RC and Dev use separate service identities, certificates and trust
stores so a development client cannot silently control a Production engine.

### Scoped command contract

The LAN protocol has its own versioned manifest and accepts only an explicit
booth-safe allowlist:

- set `OFF`, `ARM`, `START` or `PAUSE` with expected state revision;
- enable or disable Ableton Link;
- save a bounded lighting offset whose normal safe application remains the next
  phrase boundary;
- select Theme from a future phrase;
- select AutoLoop for a future phrase;
- lock or unlock a future plan cue;
- request a new Remote Live snapshot.

Every mutation carries an idempotency key and the same authoritative state,
track-load and plan revisions as the local client. Library, USB, output mapping,
manual MIDI test, service-control and developer diagnostic commands are rejected
at the gateway even if they exist in the internal protocol.

The first beta grants at most one paired device a Controller lease. Other paired
devices are view-only. The Mac explicitly transfers or revokes that lease.

### iOS lifecycle

The iPhone app is a convenience client, never a show dependency. It does not
claim continuous background execution. When suspended or disconnected it queues
no commands. On foreground it discovers or reconnects, authenticates and obtains
one new authoritative snapshot before enabling controls.

Bonjour result updates are treated as desired connection state rather than as a
reconnect signal. An identical service set is a no-op. Every real replacement,
network loss or lifecycle stop advances a connection generation, closes the old
transport and prevents late frames or send failures from mutating the new
session.

## Consequences

- The proven show-critical lanes are unchanged and remain operational if every
  iPhone and the gateway disappear.
- A dedicated gateway and remote projection add implementation and packaging
  work, but make the trust and performance boundary reviewable and testable.
- Bonjour requires local-network permission and can be blocked by venue client
  isolation; the Mac app must expose actionable status.
- Remote state can be visually a fraction behind the engine, but it can never
  delay or schedule lighting output.
- Public iPhone beta distribution will eventually require Apple Developer
  Program membership; development on the owner's connected device can start
  before that.

## 0.6.1 control-plane correction

A physical long-running session exposed that the first 0.6.0 Mac client opened
and cancelled a new Remote Gateway admin socket every two seconds. After about
870 short-lived connections, new Network.framework loopback connections began
to stall and the already authenticated desktop-engine session was reset. The
show-critical engine lanes remained separate, but the desktop lost its decks
and its single recovery attempt could remain failed.

The admin client now retains one serialized authenticated connection, replaces
it only when the gateway service identity changes and closes it explicitly when
Lumi exits or disables the gateway. Desktop-engine connection and authentication
retry transient local failures with bounded delay, while authentication,
protocol and approval failures remain fail-closed. A detected established
session I/O failure always parks operation and output safely before the engine
accepts a replacement UI client.

## Rejected alternatives

### Expose the existing engine socket on all interfaces

Rejected because the internal token and complete command protocol have a wider
trust and data boundary than a booth remote needs.

### Route the phone through the macOS SwiftUI application

Rejected because closing, blocking or foregrounding the desktop UI would then
affect remote availability.

### Poll complete snapshots from the phone

Rejected because repeated deck, plan and waveform serialization is unnecessary
work and provides poor backpressure behavior.

### Cloud broker

Rejected because it adds latency, accounts, privacy scope and an internet
dependency to a local booth workflow.
