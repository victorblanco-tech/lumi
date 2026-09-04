# Lumi Pro DJ Link Simulator 0.4.0 release readiness

## Release identity

- product: Lumi Pro DJ Link Simulator
- version: `0.4.0`
- tag: `prolink-simulator-v0.4.0`
- platform: Apple Silicon macOS
- bundle identifier: `co.victorblan.tech.lumi.prolinksimulator`
- distribution: independent GitHub Release and self-contained DMG

## Accepted scope

The release promotes the tested `0.4.0-dev-56` feature set without adding new
simulation behavior. It contains two simulated Players, read-only Rekordbox USB
playlist discovery, ordered and shuffled Auto Mix, independent playback loops,
the CDJ-1500X traffic profile and authenticated local remote controls.

The Simulator remains isolated from Lumi's production runtime and has its own
version, tag and release assets. Promoting it does not change the Lumi 0.6.1 or
Lumi Remote 0.1.0 product versions.

## Acceptance evidence

- all simulator configuration, packet, loop, playlist rotation, security and
  Auto Mix tests pass;
- generated discovery, status, beat and precise-position packets round-trip
  through Beat Link in automated tests;
- a packaged headed app successfully discovers a real Rekordbox OneLibrary USB
  read-only, loads two Players and performs repeated playlist transitions;
- an extended two-Player playlist soak remained coherent across changing
  tracks, BPM values and Master handoffs;
- Lumi 0.6.1 remained in Start with valid live and next phrase plans,
  SoundSwitch AutoLoop output and one stable Ableton Link peer;
- the simulator reported no Pro DJ Link traffic error during the accepted soak;
- the release package contains its own Java runtime and no Homebrew-linked
  native dependency.

## Distribution boundary

The Simulator is a test tool, not a replacement for CDJs and not part of the
Lumi application bundle. It must be used on a trusted local network without
physical Pro DJ Link Players or another simulator competing for the same
protocol identity.

The DMG is ad-hoc signed and intentionally not notarized. The release workflow
creates a draft first; the DMG and checksum are verified before deliberate
publication.
