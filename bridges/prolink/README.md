# Lumi Pro DJ Link bridge

This helper is the only Lumi process that imports Deep Symmetry Beat Link
types. It is a read-only network adapter supervised by `lumi-engine`; it is not
a user-facing application and does not require Beat Link Trigger.

## Local build

```bash
./scripts/verify-prolink-bridge.sh
```

The script selects Homebrew OpenJDK 21 when `JAVA_HOME` is not already set.
Maven and a JDK are development dependencies only. The eventual macOS package
contains the helper and a minimal runtime.

## Process protocol

- stdout: protocol v1 NDJSON envelopes only;
- stderr: logs and diagnostics only;
- stdin: lifecycle commands; EOF means the supervising engine has stopped;
- callbacks: Beat Link callback threads only enqueue immutable facts;
- writer: one dedicated thread serializes and flushes envelopes.

The bridge does not decide which track or deck controls lighting. It reports
source facts; the Rust engine remains the serialized state and timing authority.

## Opt-in physical media probe

`MediaIdentityProbeMain` is a separate hardware test entry point. Lumi and
BridgeMain do not run it. It reads only `/.lumi-media.json` on the Player's `/C/`
USB export, using Crate Digger's NFSv2/MOUNTv1 RPC types. It does not start a
virtual Player or change playback, tempo, MIDI, music or Rekordbox data.

Read the marker locally first and retain its SHA-256. After safely moving that
USB to a real CDJ, use the private IPv4 address shown in Lumi's Pro DJ Link page:

```bash
java -cp bridges/prolink/target/lumi-prolink-bridge.jar \
  co.victorblan.tech.lumi.prolink.MediaIdentityProbeMain \
  PLAYER_IPV4 EXPECTED_LOCAL_SHA256
```

The runner supervises a separate reader with an eight-second total deadline.
RPC work has a five-second budget and a 1200 ms per-call cap. A marker must be a
regular file of 1..4096 bytes before READ; each decoded response is capped at the
requested chunk size before allocation. Output is one JSON outcome. Exit 0
means validated schema and exact local byte hash; exit 1 means a different
reference; exit 2 means a classified failure. No source registration or live
track matching is performed. Network identity is not authentication.

The [hardware story](../../docs/planning/story-e10-09-network-media-resolution.md)
records observed CDJ behavior and the acceptance still required. A simulator
or fixture pass does not replace physical media testing.
