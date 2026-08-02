# Lumi protocol v1

Protocol v1 is the stable semantic boundary between the autonomous Rust engine
and native clients. It is independent of TCP, Unix sockets, Network.framework,
and process lifecycle.

Every UTF-8 newline-delimited JSON envelope contains the seven fields required
by `envelope.schema.json`. A complete encoded message may not exceed 65,536
bytes. Unknown fields are ignored so optional additions remain backward
compatible within v1. Unknown protocol versions, malformed JSON, invalid
required fields, and oversized input are rejected before application mapping.

`manifest.json` is the reviewable contract index for limits, supported command
and event names, and canonical cross-language fixtures. Field and kind names are
stable English `lowerCamelCase`.

Mutating commands use `messageId` as their idempotency key and include an
expected state or plan revision in their payload. The engine applies a given
command ID at most once. Event sequences are monotonic per connection. A client
that receives a forward sequence gap requests a full snapshot before trusting
incremental state again.

Wire DTOs are mapped into Rust domain and Swift presentation types. They are
never the authoritative runtime state.

State snapshots may include `runtimeCore`, a presentation-safe summary of the
serialized reducer model, health, bounded ingress, processed-event count, and
last structured decision reason. It exposes evidence without leaking Rust
domain types into clients.
