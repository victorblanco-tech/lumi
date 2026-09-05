# Lumi Remote protocol v1

This is the scoped LAN contract between the isolated Lumi Remote Gateway and
Lumi Remote for iPhone. It is not the desktop engine protocol.

- Each TLS application message contains exactly one JSON `RemoteFrame`.
- `sequence` is non-zero and contiguous per authenticated client connection.
- A connection starts with one `snapshot` frame.
- `transportAnchor` frames are latest-value visual updates and may be coalesced
  by the gateway before the per-client delivery sequence is assigned.
- Missing delivery sequences require a new snapshot before controls re-enable.
- Every mutation is revision-bound, idempotent and requires the active
  Controller lease.
- The contract contains no Library, USB, audio URI, filesystem path, engine
  token or raw integration diagnostics.

The authoritative limits and allowlists are recorded in `manifest.json`.
Fixtures are consumed by the Rust protocol tests and mirrored by the Swift
client tests.

Authentication uses a `hello` frame before the initial snapshot. Optional
`clientVersion` in both `authenticate` and `pair` payloads is a non-empty,
control-character-free string of at most 64 bytes. The response optionally
includes `controllerDisplayName`; absence must be accepted for older gateways.
Only `controllerLeaseId`, not connection health or the display name, grants
mutation permission. These additive fields retain protocol major 1 and support
older clients which neither send a version nor display the Controller's name.
