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
