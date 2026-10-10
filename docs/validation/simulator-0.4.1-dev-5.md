# Simulator 0.4.1-dev-5 validation

2026-10-09, Apple silicon macOS. Cross-Mac hardware acceptance pending.

- Java verification: 39 tests, zero failures.
- Native RPC acceptance on standard UDP 111: 9 tests passed, including the unchanged production `MediaIdentityProbeMain`, per-destination isolation, stale handles, bounded reads, denied writes/path traversal/symlinks, fault recovery, malformed packets, parent EOF cleanup and port conflicts.
- Desktop/browser controls with synthetic media: LINK source versus inserted media, cached tracks after eject, new load rejection after eject, timeout status and recovery, repeated AutoMix transitions and playlist AutoMix.
- Fixed a UI issue discovered during this test: status polling immediately cleared a rejected command's error.
- Final DMG mounted read-only and app launched. Automatic single-address mode displayed correctly; second independent USB disabled with explanation; missing USB prevents start.
- App quit and DMG ejected. No application copied to Applications and no persistent system helper installed.

This does not certify a full Lumi/SoundSwitch light show or two physical network interfaces. The next acceptance is the released DMG on the Mac mini with real Rekordbox USB media, read remotely by Lumi.
