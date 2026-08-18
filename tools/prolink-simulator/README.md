# Lumi Pro DJ Link Simulator

This is a development-only, USB-backed network player for testing Lumi without
physical CDJs. It intentionally simulates only the Pro DJ Link facts consumed
by Lumi:

- device discovery;
- loaded Rekordbox USB track identity;
- play/pause, pitch, master and on-air state;
- beat number and beat within bar.
- CDJ-1500X-style precise-position traffic at 50 Hz;
- deterministic stale-position bursts which test that consumers retain only
  the newest continuous observation and never build a realtime backlog.

The simulator discovers virtual players from their Pro DJ Link announcements
and sends CDJ status to each active peer exactly as a physical player does.
Beat and discovery packets remain broadcast traffic.

`cdj-1500x` is the default and required performance-acceptance profile. The
lower-traffic `classic` profile is available only to diagnose the simulator
itself; a Lumi release may not use it as Live Decks performance evidence.

It does not play audio, serve media to other players, emulate a CDJ display, or
accept Pro DJ Link remote-control commands. It is never included in the Lumi
production DMG.

## Two-Mac setup

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

5. For the portable archive only, start one simulated player from its unpacked
   directory:

   ```bash
   LUMI_SIM_TOKEN='choose-at-least-16-characters' \
   ./lumi-prolink-simulator/bin/lumi-prolink-simulator \
     --usb '/Volumes/DJ VIC GRAY' \
     --interface en0 \
     --player 1 \
     --traffic-profile cdj-1500x
   ```

6. Open the printed control URL on the MacBook or iPhone. The token is removed
   from the address bar and retained only in that browser tab's session storage.
7. Start Lumi's Direct Pro DJ Link input on the MacBook. Beat Link Trigger must
   be offline because both applications require the same Pro DJ Link UDP ports.

If `--interface` is omitted, the simulator selects the first active macOS-style
`en*` interface with an IPv4 broadcast address and prints the choice.

## Remote and automated control

All mutating and track-reading endpoints require a bearer token. The health
endpoint is intentionally public on the local network.

```text
GET  /api/v1/health
GET  /api/v1/status
GET  /api/v1/tracks?q=90s%20Bitch&limit=100
POST /api/v1/control/load       {"trackId":1256}
POST /api/v1/control/play       {}
POST /api/v1/control/pause      {}
POST /api/v1/control/seek       {"positionMillis":64000}
POST /api/v1/control/pitch      {"pitchPercent":4.2}
POST /api/v1/control/master     {"enabled":true}
POST /api/v1/control/on-air     {"enabled":true}
POST /api/v1/control/precise-burst {}
```

For repeatable agent or terminal tests:

```bash
export LUMI_SIM_URL='http://mac-mini.local:17840'
export LUMI_SIM_TOKEN='choose-at-least-16-characters'
./scripts/prolink-simulatorctl.sh tracks '90s Bitch'
./scripts/prolink-simulatorctl.sh load 1256
./scripts/prolink-simulatorctl.sh master on
./scripts/prolink-simulatorctl.sh play
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
precise-position packets back through beat-link itself. The status endpoint
exposes per-packet counters, profile, cadence, burst count and the last traffic
error. The simulator also fails closed when the USB database, analysis files,
player number or requested network interface is invalid.
