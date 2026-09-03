# Story E9-05: Remote resilience, performance and beta delivery

- Status: **In progress (automated resilience and headed Simulator acceptance complete)**
- Priority: **P0 show safety**
- Target: `0.6.0-beta`
- Components: Engine, Gateway, iOS, Release

## User outcome

The remote remains useful on a real booth network without becoming a dependency
or destabilizing the show.

## Scope

- deterministic protocol and visual fixtures;
- gateway crash, slow-client, malformed-frame, reconnect and credential tests;
- Wi-Fi loss/change, app background/foreground and Mac UI quit tests;
- simulator plus physical iPhone test matrix;
- combined two-Player, SoundSwitch MIDI and Ableton Link soak;
- privacy strings, permission guidance and support diagnostics;
- TestFlight signing, review demo path and beta release notes after membership.

## Acceptance

- remote failure produces zero missed/duplicate AutoLoops and zero Link peer
  duplication;
- command latency and visual-state age have measured p50/p95/p99 targets;
- no unbounded queue, retry storm, synchronous hot-path logging or sensitive
  diagnostic export is present;
- foreground reconnect obtains a current snapshot before enabling control;
- the beta is installable on a clean physical iPhone and pairs with a clean Mac
  installation using documented steps.

## Prepared foundation

- Remote Client and Remote Live packages run warnings-as-errors tests;
- the Xcode product is built by local Apple verification;
- malformed, oversized, stale, duplicate and sequence-gap contract behavior is
  covered without a network;
- independent iPhone and simulator tags create controlled draft releases;
- physical-iPhone and show-lab evidence remains mandatory and is not replaced
  by the Simulator artifact.

## Implemented evidence

- bounded frame sizes, client counts, authentication attempts, admin messages,
  engine command queues and per-client delivery buffers;
- slow clients are disconnected while latest-value transport anchors coalesce;
- malformed, oversized, stale, duplicate and sequence-gap behavior is covered;
- gateway TLS, protected identity/trust persistence and authenticated admin
  operations execute in local network tests;
- iOS foreground/background explicitly closes and recreates transport; no
  command survives disconnect;
- macOS and iOS product targets build with strict concurrency and warnings as
  errors; the packaged Mac app contains the independent gateway executable and
  channel-specific LaunchAgent.
- a signed headed Simulator client completed fresh pairing, Controller transfer,
  portrait and landscape live projection and operation commands, then
  reconnected from Keychain after leaving the app without replaying a command;
- simulator-only Bonjour routing and iOS Keychain entitlements have regression
  coverage without changing physical-device discovery behavior;
- Mac helper-version validation prevents an obsolete registered gateway from
  presenting false Ready state after a Dev app update.
- duplicate Bonjour result callbacks are no-ops; real service replacement and
  lifecycle stops invalidate a connection generation, close the old transport
  and suppress late state or command failures from that stale generation;
- a deterministic four-client/two-Player storm keeps only two latest anchors
  per client through 20,000 updates and preserves contiguous delivery order;
- stalled LAN writers and gateway-to-engine command responses have tested
  deadlines and release their bounded client/task capacity;
- compact landscape controls retain their accepted visual size with full
  44-point touch targets, accessibility Dynamic Type fallback and explicit
  VoiceOver health/value/action descriptions.
- Mac transport timestamps are translated from source age into the receiving
  iPhone clock domain before interpolation; raw source ordering remains
  independently monotone and cannot be weakened by that translation.
- the signed dev-12 build restored its physical aiVoon Controller session and a
  20.951-second live Animation Hitches trace recorded zero hitches, zero
  interaction delays above 33 ms and 53–60 displayed surfaces per complete
  second after startup.

## Remaining beta gate

- deterministic screenshots across supported iPhone sizes and accessibility
  settings;
- physical iPhone discovery, pairing, rotation and lifecycle matrix;
- two-phone Controller transfer/revoke evidence;
- combined two-Player, SoundSwitch and Ableton Link soak with Remote connected,
  disconnected and deliberately overloaded;
- measured command latency and visual-state-age percentiles;
- signing/TestFlight setup only after the separate Apple membership decision.
