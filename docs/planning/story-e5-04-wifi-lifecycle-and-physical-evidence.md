# Story E5-04: Wi-Fi, lifecycle and physical release evidence

- Status: **Simulator/UI baseline ready; physical duration evidence pending**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-56`
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
- normal UI Quit leaves the inactive launchd engine, its supervised Pro DJ Link
  bridge and stable CoreMIDI endpoints available, but no Link peer;
- explicit service retirement leaves zero Lumi helper, Link peer or owned child
  process;
- collect latency percentiles, cue counts, transport epochs, source gaps and
  fault transitions in one bounded evidence artifact;
- repeat the core matrix on physical CDJ-1500X players and SoundSwitch.
- use the `cdj-1500x` simulator profile for all performance evidence; `classic`
  is never accepted as a substitute.

## Dev-55 baseline

The simulator, headless network matrix and actual Lumi/SoundSwitch desktop UIs
now cover the complete E5-03 action matrix. UI Quit was observed to remove the
Link peer within the bounded cleanup window while the channel engine and its
supervised Pro DJ Link bridge remained parked as designed. The Apple gate
separately retires that service and proves exclusive MIDI ownership for a fresh
engine process.

Still required before this story can close:

- one-hour Pro DJ Link-only, Link-only, MIDI-only and combined soaks;
- repeatable Wi-Fi and Ethernet evidence without a latency trend;
- the complete matrix on physical CDJ-1500X players, SoundSwitch, Control One
  and DMX output;
- one retained evidence artifact with end-to-end age percentiles, exact cue
  counts, execution epochs, queue bounds and fault transitions.
