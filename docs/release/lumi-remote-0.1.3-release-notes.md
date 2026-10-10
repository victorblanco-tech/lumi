# Lumi Remote 0.1.3 — Public Beta

The iPhone companion in the Lumi 0.6.4 / Lumi Remote 0.1.3 update.
Use this version with Lumi 0.6.4 for the complete feature set.

## What's new

- Show the verified USB name and native Rekordbox media color for each Player.
- Distinguish the inserted USB from the source of the loaded track, including
  tracks loaded from another Player over LINK. Keep that origin readable in
  landscape and while a cached track continues after eject.
- Share the Mac's saved lighting offset and initial-show-start policy. Choose
  Immediate or On phrase start while Off or Arm; see the upcoming launch state.
- Retain production-only QR pairing through `lumi://`, separate from Dev/RC.

The Mac remains authoritative for planning and lighting output. Remote never
sits in the Pro DJ Link, Ableton Link or MIDI timing path. This update does not
add automatic SoundSwitch phase correction.

## Installation

Install from source using Xcode, the **Release** configuration and your own
Apple development team. Updating the same production bundle preserves app
data and pairing. The Mac must run on the same reachable local network.

There is no TestFlight/App Store distribution or universally installable IPA.
A GitHub iOS Simulator artifact runs on a Mac, not on a physical iPhone.
See the [iPhone installation guide](../user-guide/iphone-remote.md).

The Mac and iPhone have independent versions and artifacts, but these releases
form one documented update. The simulator is not promoted by this bundle.
