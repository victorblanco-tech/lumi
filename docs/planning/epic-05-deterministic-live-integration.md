# Epic 5: Deterministic Live integration

- Status: **In progress**
- Priority: **P0 Critical**
- Target: `0.4.0` stability gate
- Components: Pro DJ Link, Engine, Ableton Link, MIDI & SoundSwitch, macOS
- GitHub tracking: [#116](https://github.com/victorblanco-tech/lumi/issues/116)

## User outcome

As a performing DJ, I can run Lumi over Wi-Fi or Ethernet and trust that
SoundSwitch follows the master-CDJ BPM in real time and receives exactly one
correct AutoLoop selection at each musical event, without timeline jumps,
duplicate peers, hidden restarts or UI influence.

## Non-negotiable product rules

- SoundSwitch owns AutoLoop progress after Lumi selects it.
- Link transports master BPM, beat phase and play state only.
- MIDI selects Bank and AutoLoop only.
- Link and MIDI fail independently.
- UI rendering and UI lifecycle are not timing authorities.
- one out-of-order network packet cannot become a seek or hotcue.
- Wi-Fi is supported and must pass the same correctness gates as Ethernet.

## Delivery stories

0. [E5-00 — Representative CDJ-1500X simulator traffic](story-e5-00-representative-cdj1500x-simulator.md)
1. [E5-01 — Isolated Transport and Ableton Link Relay](story-e5-01-isolated-transport-and-link-relay.md)
2. [E5-02 — Exactly-once phrase AutoLoop executor](story-e5-02-exactly-once-phrase-autoloop-executor.md)
3. [E5-03 — Explicit playback epochs, hotcues and output offset](story-e5-03-transport-epochs-hotcues-and-offset.md)
4. [E5-04 — Wi-Fi, lifecycle and physical release evidence](story-e5-04-wifi-lifecycle-and-physical-evidence.md)

Implementation sequence and measurable budgets are defined in the
[one-page Live integration separation plan](live-integration-separation-plan.md).

## Acceptance correction — `0.4.0-dev-52`

Dev-52 is rejected as a Live Decks performance release. Its classic MIDI 1.0
provider correction is retained, but a sustained physical run showed increasing
deck, AutoLoop and Link delay. Earlier evidence measured only the final MIDI
dispatch and the Rust-side queue. It did not measure the age accumulated in the
upstream Java bridge FIFO, and the simulator did not generate CDJ-1500X
PrecisePosition load. E5-01 and E5-02 are therefore reopened for end-to-end
freshness and isolation evidence; their delivered component behavior remains
characterization coverage, not completion evidence.

## Exit criteria

- exactly one expected MIDI AutoLoop selection per execution epoch;
- no unexpected Bank or AutoLoop selection in recorded and physical runs;
- no SoundSwitch progress rewind caused by Lumi;
- real-time master pitch/BPM changes reach SoundSwitch through one Link peer;
- no Link or MIDI restart caused by operation-state or UI changes;
- hotcue, seek, pause/resume and master handoff matrices pass deterministically;
- closing/reopening the client does not interrupt an explicitly active engine
  session, while explicit Quit performs a bounded clean shutdown;
- one-hour Wi-Fi combined soak passes with bounded queue, latency and fault
  telemetry.
- source-to-consumer age remains bounded at the Java, Rust, AutoLoop, Link and
  display boundaries and never trends upward with run duration.

## Release rule

The current Live implementation is not promoted to RC while any story in this
epic is incomplete. Passing Local Playback does not substitute for connected
CDJ evidence because their clock authorities differ.
