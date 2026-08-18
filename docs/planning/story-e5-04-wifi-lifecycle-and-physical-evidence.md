# Story E5-04: Wi-Fi, lifecycle and physical release evidence

- Status: **Implementation and automated evidence complete; physical duration evidence pending**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-57`
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

## Dev-56 implementation completion

All software work for this story is complete:

- source-to-engine age includes Java bridge and Rust supervisor residence and
  is retained as a bounded p50/p95/p99/max histogram;
- fresh Link clock state is forwarded before library hydration and planning;
- native Integration diagnostics expose queue and source-age health;
- one configurable runner covers Pro DJ Link-only, Link-only, realtime
  MIDI-only and combined modes;
- combined stress includes 40 Hz UI polling, live pitch changes, repeated
  seeks and independent lighting Pause/Start cycles;
- the combined runner emits one credential-free evidence JSON document.

This leaves no unfinished implementation story. Closure and RC promotion are
still intentionally blocked on the one-hour Wi-Fi/Ethernet runs and physical
CDJ-1500X/SoundSwitch/Control One/DMX observation, which cannot be manufactured
by an automated simulator run.

## Dev-56 installed UI acceptance

The DMG-installed native app was exercised against the `cdj-1500x` LAN
simulator and the real SoundSwitch UI. Start selected the current phrase,
forward/backward landings selected the matching AutoLoop, pitch followed
`155.0 -> 161.5 -> 151.9`, and the lanes kept operating while Lumi was in the
background. The final runtime snapshot showed one Link peer, zero late
AutoLoop output, zero queue saturation and zero provider failures. UI Quit
removed the Link peer while leaving the same launchd engine safely parked;
reopening attached to that existing process.

## Dev-57 dynamic-device recovery

Physical testing found one lifecycle defect that the fixed simulator topology
did not cover: powering on player 2 while player 1 was already live could emit
temporary no-track/no-BPM sentinel values. The bridge published those values as
realtime tempo, Rust rejected the invalid tempo envelope and restarted the
entire Pro DJ Link child process. Discovery recovered, but the replacement
provider restarted its domain sequence at 1 under the existing source ID, so
the reducer correctly rejected all reconstructed deck loads as stale.

Dev-57 makes a joining device local to its own warm-up lifecycle. Incomplete
loaded-track and invalid realtime-tempo frames are withheld until coherent;
they cannot degrade an existing deck, Link clock or AutoLoop lane. If the child
process genuinely fails, recovery preserves the provider's monotone source
sequence and track-load allocator while resetting only bridge-session state.
Both decks must therefore rehydrate automatically without restarting Lumi.

Regression evidence covers the observed sentinel values, a complete
fail/clear/recover cycle, monotone post-recovery observations and a new deck
load after the bridge decoder itself starts again at sequence 1.

The installed dev-57 build was also tested against both physical CDJ-1500X
players and the DJM-V5. Terminating the supervised bridge while both loaded
decks were present left the engine alive, produced one bounded automatic
restart and restored both library-backed decks without an engine or Lumi UI
restart. The recovered snapshot reported zero queue saturation, ignored
events and provider failures. A real player-2 power-on during an active show is
still required as the final acceptance check for the originally observed
sequence.
