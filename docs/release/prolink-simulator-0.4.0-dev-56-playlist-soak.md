# Pro DJ Link Simulator `0.4.0-dev-56` — playlist soak testing

This simulator-only development release extends two-player Auto Mix with real
Rekordbox USB playlists. It remains isolated from the Lumi production runtime.

## Included

- read-only playlist and folder discovery from the connected Rekordbox USB;
- playlist selection in the remote browser UI;
- ordered or shuffled playback with duplicate track references collapsed;
- automatic initial loading of both players;
- preloading of the next different playlist track on the idle player after
  every exclusive Master/On Air handoff;
- manual two-track Auto Mix remains available;
- authenticated playlist API and matching CLI controls.

## Verification

- all simulator configuration, packet, loop, playlist rotation and Auto Mix
  tests pass;
- the real connected Rekordbox USB is parsed read-only and returns its complete
  nested playlist paths and track counts;
- headed packaged-app verification covers playlist selection, two initial
  loads and repeated five-second transitions with changing track identities;
- the USB is never modified and the feature adds no code to Lumi's realtime
  production lanes.

Auto Mix simulates player state and Pro DJ Link traffic; it does not mix or
play audio.
