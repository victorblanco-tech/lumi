# Local engine session

During Epic 1, the native macOS app owns one child `lumi-engine` process for
the duration of its app session. The process boundary is already replaceable so
a later `SMAppService` lifecycle does not change views or protocol models.

## Bootstrap sequence

1. The app creates a cryptographically random, ephemeral session token.
2. Xcode bundles the matching Rust binary in `Lumi.app/Contents/Helpers`.
3. The app starts that helper with the token in its child environment.
4. The engine binds atomically to `127.0.0.1:0` and prints one JSON startup
   record containing only host, port, and protocol version.
5. The app rejects non-loopback endpoints and protocol mismatches.
6. A replaceable Network.framework transport connects and sends one bounded
   authentication frame.
7. The UI becomes ready only after decoding the engine's authoritative protocol
   v1 state snapshot.
8. One authenticated connection remains open for bounded command/response
   traffic. Each plan mutation returns either a complete snapshot or a typed
   validation/revision error correlated to the command ID.

The authentication frame belongs to local transport bootstrap and is not an
application command. After authentication, all semantic traffic uses the
versioned envelopes in `contracts/protocol/v1`.

Session tokens are never printed or included in structured logs. The engine
accepts one loopback client and exits when that client disconnects. Startup,
connection, and authentication all have bounded input and timeouts.

## Local verification

`./scripts/verify.sh` builds the real Rust executable, launches it from both
Rust and Swift integration tests, bundles it into the unsigned native app, and
checks that the helper exists as an executable in the resulting app bundle.
