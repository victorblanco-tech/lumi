# Epic 9 – Native iPhone Remote Beta

Status: **In development** | Target: **0.6.0** | Priority: **P0/P1**

## Outcome

A paired iPhone provides a focused, native booth view of the authoritative Live
and Next Players. The DJ can inspect both phrase-aware Light Plans and safely
adjust operation state, Ableton Link, timing offset and future plan choices. The
Mac engine keeps running the show independently; losing the phone or Remote
Gateway has no effect on Pro DJ Link, SoundSwitch MIDI or Ableton Link.

## Product boundaries

Included:

- Live Decks only;
- compact user-facing integration status;
- current/next waveforms, beatgrids, Hot Cues, phrases and Light Plans;
- future-phrase Theme, AutoLoop and lock mutations;
- operation, Ableton Link and timing-offset controls;
- local discovery, pairing, reconnect and device revocation.

Excluded from 0.6.0:

- Local Playback and audio streaming to the phone;
- Library, USB synchronization and Track Editor;
- output-profile or Light Plan policy configuration;
- raw integration traffic and developer diagnostics;
- cloud relay, remote internet access, watchOS and iPad-specific layouts;
- editing phrase boundaries or Phrase Roles during a show.

ADR-0040 is the accepted architecture authority. The accepted visual contract is
recorded in `docs/design/iphone-remote/README.md`.

## Current evidence (`0.6.0-dev-10` / Remote `0.1.0-dev-10`)

- the independent iOS app target builds for the generic iOS Simulator;
- portrait Master-first and landscape side-by-side Live compositions use the
  shared Lumi palette, RGB waveform data, Hot Cues, beatgrid, phrases and
  proportional Light Plans;
- headed dev-5 acceptance shows a loaded Master plus a stable second Player in
  portrait and two equal, fully visible Player surfaces in landscape, with the
  compact control bar and complete phrase/Light Plan bands still on screen;
- bounded waveform transport now preserves one real dominant RGB sample per
  bucket, preventing the independent channel peaks from creating the pale
  waveform that differed from the detailed Mac renderer;
- both the Phrase band and Light Plan band open one touch-first editor with a
  Phrase selector and Theme/Bank, AutoLoop and lock controls; current and past
  phrases remain inspectable but immutable;
- the running phrase/AutoLoop uses Lumi's red live emphasis and exactly one
  upcoming phrase/AutoLoop uses the blue `NEXT` emphasis; both follow the same
  interpolated transport position as the fixed waveform playhead;
- the Master playhead remains at 22% for the complete track on macOS and
  iPhone, using black pre-roll/post-roll at the boundaries; iPhone portrait
  and landscape both retain the same 40-bar Live zoom;
- the scoped Remote v1 contract is shared through repository fixtures and has
  matching Rust/Swift decoding tests;
- per-client delivery sequencing remains contiguous even when visual transport
  anchors are coalesced;
- command construction is revision-bound and the gateway rejects duplicate
  mutating command IDs after the first admission;
- release-channel Bonjour discovery, native Camera deep-link pairing and
  Keychain credential storage are wired without exposing engine credentials;
- a newly scanned invitation safely supersedes stale Keychain state and stores
  its replacement only after approved pinned-TLS pairing succeeds;
- the separately packaged gateway binds the LAN only after the user enables
  iPhone Remote and advertises the matching Dev, RC or Production service;
- every LAN session uses rustls TLS with a persistent per-installation
  certificate pinned from a one-use, five-minute pairing invitation;
- the Mac stores only device credential verifiers in protected, atomically
  replaced trust state and exposes approve, revoke and Controller transfer in
  `Integrations > iPhone Remote`;
- the engine now publishes an authenticated, path-free Live projection over a
  second loopback-only endpoint that is independent from desktop lifecycle;
- complete waveform/plan state is change-driven while transport/BPM anchors
  are bounded to 20 Hz and can be coalesced;
- revision-safe Remote commands reach the existing reducer through a bounded
  queue and have end-to-end command-result coverage;
- the native client authenticates, obtains exactly one current snapshot before
  controls enable, routes the complete booth-safe command allowlist, displays a
  fixed command-feedback region and emits accepted/rejected haptics;
- foreground/background handling explicitly tears down discovery and transport,
  queues nothing and keeps the iPhone awake only while Remote Live is active;
- TLS, persistent identity/trust, protected loopback administration, controller
  persistence, QR/auth wire compatibility, stale revisions and reconnect gaps
  have deterministic regression coverage;
- the macOS supervisor and Rust gateway share an explicitly tested
  lower-camel-case admin contract for IDs and SHA-256 fingerprints;
- the signed iPhone Simulator completed the real Bonjour -> pinned-TLS -> QR
  pairing -> explicit Mac approval path against a running Dev gateway;
- Controller transfer enabled the booth controls, `ARM`, `START` and confirmed
  `OFF` reached the authoritative reducer exactly once, and simulated playback
  advanced the displayed Player, waveform and Light Plan;
- terminating and relaunching the Simulator app restored the authenticated
  session from its scoped Keychain credential without scanning another QR code;
- release-scoped Bonjour metadata exposes the ephemeral TLS port only for the
  CoreSimulator host-loopback route; physical iPhones retain normal Bonjour
  endpoint routing and the same certificate pin;
- the Mac rejects a service record from an older bundled Remote Gateway and
  presents an update/approval message instead of reporting stale readiness;
- gateway disconnect, authentication failure and desktop snapshot polling do
  not park the show or add full projection work to the realtime loop.
- identical Bonjour updates preserve the active pinned-TLS session; service
  replacement and lifecycle reconnect are generation-bound so stale tasks
  cannot publish state or command failures into their successor;
- four concurrent clients remain bounded through 20,000 alternating two-Player
  anchors, while stalled client writes and engine command responses terminate
  at deterministic deadlines;
- normal landscape keeps its compact controls with 44-point hit targets, while
  accessibility Dynamic Type receives the roomier two-row control composition;
  VoiceOver announces health, value and action for every persistent booth
  status and primary control.
- Remote dev-10 removes the remaining iPhone-only playback bottleneck: one
  stable high-resolution RGB track raster is translated by Core Animation,
  rather than resampled and repainted through SwiftUI every display frame.
  Live pinch zoom remains anchored to the fixed 22% Master playhead.

Physical-iPhone pairing and real Local Network permission are proven on the
personally provisioned device. Extended rotation/gesture acceptance,
multi-device Controller transfer and the combined booth soak remain open. No
current Mac live-performance behavior is changed by this feature.

## Delivery stories

1. [E9-00 – Physical master-tempo propagation latency](story-e9-00-physical-master-tempo-latency.md)
2. [E9-01 – Remote Live projection and isolated gateway](story-e9-01-remote-live-projection-and-gateway.md)
3. [E9-02 – Local discovery, pairing and device trust](story-e9-02-local-discovery-pairing-and-trust.md)
4. [E9-03 – Native iPhone Live presentation](story-e9-03-native-iphone-live-presentation.md)
5. [E9-04 – Revision-safe remote booth controls](story-e9-04-revision-safe-remote-booth-controls.md)
6. [E9-05 – Resilience, performance and beta delivery](story-e9-05-remote-resilience-performance-and-beta.md)

The baseline is deliberately first: the remote is not allowed to hide or worsen
an existing Pro DJ Link-to-Ableton Link latency regression.

## Shared Apple architecture

The existing `LumiDesignSystem` and `LumiProtocol` packages already declare iOS
18 support. Mobile delivery extracts the provider-neutral snapshot mapping,
phrase colors, waveform/beat-space presentation and plan mutation contexts from
the macOS-only `LumiLiveWorkspace` into a cross-platform Live presentation
package. Desktop and iPhone keep distinct SwiftUI compositions; they do not
share one oversized responsive view.

`LumiEngineClient` remains macOS/loopback specific. A new `LumiRemoteClient`
owns Bonjour, TLS, pairing credentials, the Remote protocol and reconnect logic.
The iPhone target contains no engine process supervision, USB or local audio.

## Exit criteria

- A real iPhone discovers and pairs with one Production Mac without internet.
- Portrait shows the Master first and the prepared next Player below; landscape
  keeps numbered Players side by side.
- Player number, detected model, track, waveform, Hot Cues, phrases, playhead and
  Light Plan match the macOS Live state after load, seek, Hot Cue, beat jump and
  master handover.
- Operation state, Ableton Link, timing offset and allowed future-plan edits are
  accepted exactly once or fail visibly on a revision conflict.
- PDL, Light Output and Link health stay visible during discovery and reconnect
  without presenting stale status as healthy.
- Disconnect, app suspension, Wi-Fi change and gateway restart queue no command
  and do not alter the running show.
- A gateway flood/slow-client test causes no missed or late AutoLoop and no
  measurable change to the accepted Ableton Link latency distribution.
- The Mac shows connected/paired devices and can transfer or revoke control.
- Unit, protocol, security, fault-injection, simulator, headed macOS and physical
  iPhone/hardware acceptance evidence all pass before TestFlight beta.

## Distribution gate

Development begins with the iOS Simulator for deterministic presentation and a
personally provisioned physical iPhone for real Bonjour, Local Network permission,
pairing and lifecycle tests. TestFlight and external beta distribution are a
separate gate requiring Apple Developer Program membership and App Store Connect
configuration.
