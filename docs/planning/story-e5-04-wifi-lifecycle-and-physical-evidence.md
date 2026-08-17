# Story E5-04: Wi-Fi, lifecycle and physical release evidence

- Status: **Refined; waiting for E5-03**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-54`
- GitHub tracking: [#120](https://github.com/victorblanco-tech/lumi/issues/120)

## Outcome

The complete connected-CDJ chain remains stable over Wi-Fi, through UI/client
changes and during clean shutdown, with measured evidence suitable for an RC
decision.

## Scope and acceptance

- explicitly select and report the Pro DJ Link network interface;
- run Pro DJ Link-only, Link-only, MIDI-only and combined one-hour soaks;
- verify exactly one Link peer, zero implicit helper restarts and bounded queues;
- verify UI occlusion, app switching and client reconnect do not affect timing;
- retain a stable CoreMIDI endpoint while the engine is active;
- explicit shutdown leaves zero Lumi helper, Link peer or owned child process;
- collect latency percentiles, cue counts, transport epochs, source gaps and
  fault transitions in one bounded evidence artifact;
- repeat the core matrix on physical CDJ-1500X players and SoundSwitch.
