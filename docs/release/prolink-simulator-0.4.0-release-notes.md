# Lumi Pro DJ Link Simulator 0.4.0

Pro DJ Link Simulator 0.4.0 is the first stable, independently versioned
release of Lumi's self-contained macOS test tool. It generates the limited
Pro DJ Link traffic Lumi consumes, allowing representative two-Player testing
without physical CDJs.

## Highlights

- **Two simulated Players** — independently load, play, pause, seek, pitch,
  elect Master and set On Air state for Player 1 and Player 2.
- **Rekordbox USB playlists** — browse a connected OneLibrary USB read-only and
  select a playlist as the source for ordered or shuffled Auto Mix runs.
- **Unattended soak testing** — Auto Mix loads both Players, preloads a new
  track on the idle Player and performs exclusive Master and On Air handoffs.
- **Loop testing** — configure or clear an independent playback loop on either
  Player to exercise long-running phrase and transport behavior.
- **Representative timing** — the default CDJ-1500X profile emits precise
  position traffic at 50 Hz, beat events and changing effective BPM.
- **Remote controls** — an authenticated local-network UI and CLI API make
  repeatable headed and automated integration tests possible.

## Installation

1. Download `Lumi-Pro-DJ-Link-Simulator-0.4.0-macOS-arm64.dmg` and its
   SHA-256 checksum from this release.
2. Open the DMG and drag **Lumi Pro DJ Link Simulator** to Applications.
3. If macOS blocks the ad-hoc signed app, follow the installation instructions
   included in the DMG.
4. Connect a Rekordbox OneLibrary USB and launch the Simulator.
5. Run Lumi on another Mac on the same trusted LAN. Do not run physical CDJs or
   another Pro DJ Link simulator on that network at the same time.

The app includes its own Java runtime and does not require Terminal, Java or
administrator rights to run. A non-admin user can install it in
`~/Applications`.

## Scope and safety

The Simulator does not play audio, serve rekordbox media, emulate a CDJ screen
or control physical Players. It reads the selected USB and broadcasts simulated
Player state; it does not modify the USB.

The remote-control token grants control of the simulated Players. Use it only
on a trusted local network and never expose its control port to the internet.

This macOS app is Apple Silicon-only, ad-hoc signed and not notarized.

See the [Simulator guide](../../tools/prolink-simulator/README.md) for setup,
Auto Mix and remote-control details.
