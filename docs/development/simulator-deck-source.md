# Simulator deck source

Epic 1 uses a deterministic two-deck simulator to develop the complete Lumi
flow without DJ hardware or Rekordbox. It is production code behind the same
application-owned port that future live providers will implement.

## Boundary

`lumi-deck-source::DeckSourceProvider` emits only normalized domain events.
`lumi-simulator` owns fixture decoding, timing controls, and simulated source
state. No simulator type crosses into `lumi-domain`, the protocol contract, or
Swift presentation state. The protocol exposes `providerKind` for diagnostics,
not for business decisions.

The simulator remains an internal acceptance source. Product Library playback
uses the separate Local Playback provider described in
[`library-local-playback.md`](library-local-playback.md).

## Canonical session

`fixtures/demo-session-v1/session.json` contains two synthetic, license-free
tracks with canonical title, artist, milli-BPM, musical key, duration, and
contiguous phrases. Deck 1 is initially Live and Deck 2 is loaded as Next before
the leader event is emitted.

The provider accepts an injected monotonic clock and supports 1x, 4x, 16x, and
64x speed, pause, resume, reset, and leader advance. Integer beat accumulation
avoids floating-point drift. At 64x it still emits every beat and phrase
boundary in order.

## Determinism evidence

`fixtures/demo-session-v1/initial-transcript.ndjson` is the reviewed golden
event transcript. Tests prove identical fixture and controls produce identical
bytes, accelerated playback loses no critical event, a regressing clock fails
with a typed error, and reset restores the initial canonical snapshot exactly.

Run the focused verification with:

```bash
cargo test -p lumi-simulator
```
