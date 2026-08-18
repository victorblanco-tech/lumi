# Local engine service

Since `0.4.0-dev-48`, every installed Lumi release channel owns one per-user
engine service. SwiftUI is a client of that service; it is not the engine
process owner.

## Bootstrap sequence

1. Xcode bundles the Rust executable in `Contents/Helpers`, embeds its own
   Info.plist and writes the channel LaunchAgent to
   `Contents/Library/LaunchAgents`.
2. The app creates or reuses a cryptographically random owner-only token in
   the channel Application Support directory.
3. `SMAppService` registers the non-privileged LaunchAgent. macOS Login Items
   approval is reported when required.
4. launchd starts the engine with the channel identity and KeepAlive policy.
5. The engine opens that channel's SQLite database, binds to
   `127.0.0.1:0`, and atomically publishes an owner-only discovery record.
6. The record identifies protocol, endpoint, PID, product/build, executable
   path and executable SHA-256. The app rejects a mismatching record.
7. A replaceable Network.framework transport authenticates with the local
   token and obtains the authoritative protocol-v1 snapshot.
8. One authenticated connection remains open for bounded command/response
   traffic. UI Quit makes the engine fail safe to Off and leave Link, while
   launchd retains the inactive engine and stable CoreMIDI endpoints.
9. UI relaunch attaches to the same service. If launchd replaces a crashed
   engine, repeated stale-socket failures cause the UI to attach to the new
   atomic record automatically.

Session tokens are never logged. Discovery/token files have mode `0600` and
the channel directory has mode `0700`. The engine only accepts loopback
clients, authenticates before semantic commands and bounds all frames and
timeouts.

## Legacy/package-test adapter

`EngineProcessSupervisor` retains a direct child-process path for Swift package
integration tests and unbundled development hosts which have no packaged
LaunchAgent. Installed Lumi applications always take the `SMAppService` path.

## Verification

- Rust tests validate service-record compatibility and atomic ownership.
- Swift integration tests validate authenticated reconnectable sessions.
- macOS packaging rejects a missing LaunchAgent or embedded engine Info.plist.
- installed acceptance checks launchd ownership, UI detach/reattach and
  launchd restart/UI reconnect without touching the channel database.
