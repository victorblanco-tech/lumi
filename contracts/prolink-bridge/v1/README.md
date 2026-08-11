# Lumi Pro DJ Link bridge protocol v1

The supervised Java helper emits one UTF-8 JSON object per stdout line. The
Rust decoder requires a `hello` envelope with sequence 1 and then an unbroken
monotone sequence. Unknown versions, types, fields and invalid musical ranges
fail closed.

Provider callbacks only enqueue facts. Sequence numbers are assigned by the
single protocol writer, so a sequence gap means process-output corruption or an
unsupported producer and requires a source restart.

`deckStatus.beatWithinBar` is `0` while a loaded track is before its first
beat, and may also be `0` when a deck is empty. Values `1...4` identify an
active musical beat. Empty-deck status always carries Rekordbox ID zero. Some
physical players, including CDJ-1500X, retain their own player number while
publishing `NO_TRACK`; others publish source player zero. Both forms mean the
deck is unloaded, and their placeholder BPM fields are deliberately ignored.

`observedAtNanos` is helper-process monotonic evidence. It is used for ordering
and latency diagnostics, not as a wall-clock timestamp.
