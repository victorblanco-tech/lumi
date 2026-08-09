# Lumi Pro DJ Link bridge protocol v1

The supervised Java helper emits one UTF-8 JSON object per stdout line. The
Rust decoder requires a `hello` envelope with sequence 1 and then an unbroken
monotone sequence. Unknown versions, types, fields and invalid musical ranges
fail closed.

Provider callbacks only enqueue facts. Sequence numbers are assigned by the
single protocol writer, so a sequence gap means process-output corruption or an
unsupported producer and requires a source restart.

`observedAtNanos` is helper-process monotonic evidence. It is used for ordering
and latency diagnostics, not as a wall-clock timestamp.
