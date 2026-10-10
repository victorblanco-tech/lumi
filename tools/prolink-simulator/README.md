# Lumi Pro DJ Link Simulator

For installation, Player controls, loops and playlist-driven Auto Mix, start
with the [user guide](../../docs/user-guide/pro-dj-link-simulator.md). This page
documents development builds and their API; fault injection and Recovery Soak
may not be available in an older installed release.

This is a self-contained, two-player USB-backed network simulator for testing
Lumi without physical CDJs. It intentionally simulates only the Pro DJ Link facts consumed
by Lumi:

- device discovery;
- loaded Rekordbox USB track identity;
- independent USB media libraries assigned to Player 1 and Player 2, including
  overlapping track IDs resolved from the selected source Player;
- the distinction between a USB physically inserted in a Player and a loaded
  track obtained from another Player over LINK;
- cached loaded-track origin retained after an eject, while new loads from that
  ejected source are rejected;
- play/pause, pitch, master and on-air state;
- beat number and beat within bar;
- an independent playback loop on either player;
- deterministic Auto Mix handoffs for unattended soak tests;
- playlist-driven Auto Mix which preloads a different USB track onto the idle
  player before every handoff, in playlist order or shuffled;
- CDJ-1500X-style precise-position traffic at 50 Hz;
- deterministic stale-position bursts which test that consumers retain only
  the newest continuous observation and never build a realtime backlog.
- opt-in deterministic position gaps, packet loss and temporary Player
  disconnects applied only at the network boundary;
- explicit Player remove/rejoin, Hot Cue, beat-jump and Master-handover actions;
- a repeatable Recovery Soak sequence which cycles those failures while Auto
  Mix continues to provide changing tracks and Masters.

The simulator discovers virtual players from their Pro DJ Link announcements
and sends CDJ status to each active peer exactly as a physical player does.
Beat and discovery packets remain broadcast traffic.

`cdj-1500x` is the default and required performance-acceptance profile. The
lower-traffic `classic` profile is available only to diagnose the simulator
itself; a Lumi release may not use it as Live Decks performance evidence.

It serves only the read-only Lumi USB identity marker through the production
ONC RPC/MOUNT/NFS lookup path. It does not play audio, serve audio to other players, emulate a CDJ display, or
accept Pro DJ Link remote-control commands. It is never included in the Lumi
production DMG.

## Two-Mac setup

Starting with 0.4.1-dev-4, a track whose exported beat grid fails validation
is excluded from the simulator rather than preventing the entire USB from
loading. A startup report lists each skipped track, its Rekordbox ID and the
exact beat values that failed validation. The report is also available in
`GET /api/v1/status` under `usbScanWarnings` and in the simulator log. Other
tracks and playlists remain available; skipped tracks cannot be selected by
Auto Mix. No replacement grid is generated and no USB data is modified.
This isolates invalid grids; it does not establish why a particular real-world
export failed validation. Keep the report for further diagnosis.

1. Sync the Rekordbox USB into Lumi on the MacBook so its persistent device
   mirror contains the current track IDs and analysis revisions.
2. Eject the USB and connect it to the Mac mini.
3. Connect both Macs to the same LAN; wired Ethernet is preferred.
4. For the normal Mac mini installation, build the standalone macOS DMG on the
   development Mac:

   ```bash
   ./scripts/package-prolink-simulator-app-local.sh
   ```

   Open the DMG from `build/prolink-simulator-app`, drag **Lumi Pro DJ Link
   Simulator** to Applications, and launch it from Finder. The app detects
   Rekordbox USB exports, starts the simulator, and exposes copy/open controls
   for its remote URL. It contains its own Java runtime; Java and Terminal are
   not required on the Mac mini.

   Installing into the system `/Applications` folder can require an
   administrator account. A non-admin user can instead create `~/Applications`
   and copy the app there; running the app never requires administrator rights.

   App settings are stored in:

   ```text
   ~/Library/Application Support/Lumi Pro DJ Link Simulator/config.properties
   ```

   Diagnostic logs are stored in:

   ```text
   ~/Library/Logs/Lumi Pro DJ Link Simulator/simulator.log
   ```

   The portable terminal archive remains available for development and CI
   diagnostics:

   ```bash
   ./scripts/package-prolink-simulator-local.sh
   ```

5. For the portable archive only, start two simulated players from its unpacked
   directory:

   ```bash
   LUMI_SIM_TOKEN='choose-at-least-16-characters' \
   ./lumi-prolink-simulator/bin/lumi-prolink-simulator \
     --usb '/Volumes/DJ VIC GRAY' \
     --interface en0 \
     --player 1 \
     --second-player 2 \
     --traffic-profile cdj-1500x
   ```

   To model two independent Rekordbox USBs, add the second root:

   ```bash
   ./lumi-prolink-simulator/bin/lumi-prolink-simulator \
     --usb '/Volumes/DJ VIC CHRM' \
     --usb-player-2 '/Volumes/DJ VIC GRAY' \
     --interface en0
   ```

   Player 2 can also be left without its own USB and load a track from Player 1
   over LINK. In the controls, each search result identifies its media Player;
   “Load P2 ← P1” means Player 2 loads that track from Player 1's USB. The status
   separately shows the USB inserted in each Player and the source of its loaded
   track. Ejecting a USB clears only the inserted-media status; an already loaded
   track retains its verified source and keeps playing, marked as cached after
   eject. Reinsert is available for the configured same USB. This simulates the
   Pro DJ Link track-source fields. From 0.4.1-dev-5 it also serves the existing
   `.lumi-media.json` through the same RPC/MOUNT/NFS reader used with real CDJs.
   No identity is invented and no USB files are written. Synchronize/trust the
   USB locally in Lumi first so its existing marker is registered.

6. Open the printed control URL on the MacBook or iPhone. The token is removed
   from the address bar and retained only in that browser tab's session storage.
7. Start Lumi's Direct Pro DJ Link input on the MacBook. Beat Link Trigger must
   be offline because both applications require the same Pro DJ Link UDP ports.

If `--interface` is omitted, the simulator uses the macOS default interface
among active private/link-local IPv4 broadcast interfaces. A second distinct
interface on the same broadcast network is detected automatically. Nothing
enables wifi, changes routes or configures Docker. One address supports two
Players sharing Player 1's USB over LINK; a second independent USB requires
two suitable addresses. The desktop and control page show the selected mode.
Stop/start after an address change to rediscover connections; a running session
never silently reassigns a USB to another address.

The bundled `lumi-simulator-rpc` child opens wildcard UDP 111 without root,
filters configured destinations and replies from the requested Player address.
It exits when the simulator closes, including parent-process termination.
If another service owns port 111, startup fails visibly rather than stopping it.
There is no privileged helper, system NFS export, installation daemon or firewall change.
Use a trusted LAN: the CDJ-compatible RPC protocol is not authenticated.

Media fault controls (`fault-media`, `kind`: `timeout`, `missing`, `malformed`,
`none`; `durationMillis`: 0–30000; `playerNumber`) operate independently of
timing traffic. Restore all traffic also clears these media faults.

Native regression tests (macOS): compile `native/media_rpc.c` into
`build/simulator-native/lumi-simulator-rpc`, then run
`python3 tools/prolink-simulator/native/test_media_rpc.py` from the repository root.
`LUMI_RPC_STANDARD_PORT=1` additionally verifies the unchanged production reader
on UDP 111; this requires the current bridge jar and no existing RPC service.
All marker fixtures are disposable, never real USB data.

## Remote and automated control

All mutating and track-reading endpoints require a bearer token. The health
endpoint is intentionally public on the local network.

```text
GET  /api/v1/health
GET  /api/v1/status
GET  /api/v1/tracks?q=90s%20Bitch&limit=100
GET  /api/v1/playlists
POST /api/v1/control/load       {"playerNumber":1,"trackId":1256}
POST /api/v1/control/load       {"playerNumber":2,"mediaPlayerNumber":1,"trackId":1256}
POST /api/v1/control/eject-usb  {"playerNumber":1}
POST /api/v1/control/insert-usb {"playerNumber":1}
POST /api/v1/control/play       {"playerNumber":1}
POST /api/v1/control/pause      {"playerNumber":1}
POST /api/v1/control/seek       {"playerNumber":1,"positionMillis":64000}
POST /api/v1/control/hot-cue    {"playerNumber":1,"positionMillis":32000}
POST /api/v1/control/beat-jump  {"playerNumber":1,"beats":32}
POST /api/v1/control/pitch      {"playerNumber":1,"pitchPercent":4.2}
POST /api/v1/control/master     {"playerNumber":1,"enabled":true}
POST /api/v1/control/on-air     {"playerNumber":1,"enabled":true}
POST /api/v1/control/loop       {"playerNumber":1,"startMillis":16000,"endMillis":48000}
POST /api/v1/control/loop-off   {"playerNumber":1}
POST /api/v1/control/precise-burst {"playerNumber":1}
POST /api/v1/control/player-online {"playerNumber":2,"enabled":false}
POST /api/v1/control/fault-position-gap {"playerNumber":1,"durationMillis":2000}
POST /api/v1/control/fault-packet-loss {"playerNumber":1,"lane":"timing","everyN":4,"durationMillis":5000}
POST /api/v1/control/fault-disconnect {"playerNumber":1,"durationMillis":5000}
POST /api/v1/control/clear-faults {}
POST /api/v1/control/master-handover {}
POST /api/v1/control/auto-mix   {"enabled":true,"intervalSeconds":30}
POST /api/v1/control/auto-mix   {"enabled":true,"intervalSeconds":30,"playlistId":77,"shuffle":true}
POST /api/v1/control/recovery-soak {"enabled":true,"intervalSeconds":20}
```

For repeatable agent or terminal tests:

```bash
export LUMI_SIM_URL='http://mac-mini.local:17840'
export LUMI_SIM_TOKEN='choose-at-least-16-characters'
./scripts/prolink-simulatorctl.sh tracks '90s Bitch'
./scripts/prolink-simulatorctl.sh playlists
./scripts/prolink-simulatorctl.sh load 1 1256
./scripts/prolink-simulatorctl.sh load 2 8042
./scripts/prolink-simulatorctl.sh loop 1 16000 48000
./scripts/prolink-simulatorctl.sh auto-mix on 30
./scripts/prolink-simulatorctl.sh auto-mix on 30 77 shuffle
./scripts/prolink-simulatorctl.sh recovery-soak on 20
./scripts/prolink-simulatorctl.sh fault-disconnect 1 5000
./scripts/prolink-simulatorctl.sh clear-faults
./scripts/prolink-simulatorctl.sh status
```

Do not expose the control port to the internet. Use it only on a trusted local
development LAN. A generated token is printed when neither `--token` nor
`LUMI_SIM_TOKEN` was supplied.

## Verification

```bash
./scripts/verify-prolink-simulator.sh
```

The packet tests parse generated announcements, status, beat and modern-player
precise-position packets back through beat-link itself. Deterministic state
tests verify loop wrapping, USB-grid beat jumps, deterministic fault expiry,
exclusive-master Auto Mix handoffs and the complete Recovery Soak sequence.
The status endpoint exposes both players, inserted-media state, track-source
player and cached-after-eject state, loop, Auto Mix and active fault state,
per-packet and suppressed-packet counters, profile, cadence, burst count and the last traffic error. The simulator also
fails closed when the USB database, analysis files, player numbers or requested
network interface are invalid.
