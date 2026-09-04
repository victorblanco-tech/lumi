# Pro DJ Link Simulator `0.4.0-dev-55` — two-player soak control

This simulator-only development release turns the narrow USB-backed player
into a two-player lab source without adding any code to the Lumi production
runtime.

## Included

- two independently discoverable player identities from one Mac and USB;
- separate load, play, pause, seek, pitch, Master and On Air controls;
- bounded loops with visible loop range and wrap count per player;
- Auto Mix with a configurable 5–3600 second interval;
- deterministic handoffs: the incoming player restarts, becomes On Air and
  Master, then the outgoing player pauses;
- authenticated API and CLI control for every new function;
- a compact two-player browser UI suitable for remote Mac mini operation.

## Verification

- all 20 simulator configuration, API, packet, loop and Auto Mix tests pass;
- the shaded Java 21 build passes;
- version consistency and repository structure pass locally;
- the packaged app read a real Rekordbox USB, loaded separate tracks on both
  players, preserved overshoot through a 10–15 second loop and completed three
  five-second Auto Mix handoffs;
- stopping the simulator released ports 50000 and 17840, after which the Lumi
  Dev engine and Pro DJ Link bridge restarted normally.

The Auto Mix mode is a repeatable soak driver, not a DJ transition model. It
does not mix audio. A two-host LAN run remains the final evidence that Lumi sees
both player identities and follows repeated Master changes for an extended
period.
