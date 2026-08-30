# Lumi Public Beta

Lumi is functionally complete enough for real-world evaluation, but it remains
beta software. The purpose of the public beta is to validate the USB-to-show
workflow on more hardware and library combinations than the reference setup.

## What to test

- trusted OneLibrary USB recognition, reconnect and selected-playlist sync;
- waveform, beatgrid, Hot Cue and changed-track review accuracy;
- Track Editor preparation and protected Lumi phrase data;
- Live Deck discovery, first Play, master handoff, Hot Cue and Beat Jump;
- exactly-once AutoLoop and Static Look selection in SoundSwitch;
- live BPM relay through Ableton Link;
- Local Playback and recovery after closing and reopening Lumi.

## Before a show

1. Back up Lumi and keep the source USB media backed up through your normal
   rekordbox workflow.
2. Test the exact Lumi release, macOS version, player firmware and SoundSwitch
   project you intend to use.
3. Run the complete set or a representative dry run before connecting DMX.
4. Keep physical/manual lighting control available as a fallback.

Lumi reads trusted rekordbox USB media without modifying it during normal scan
and synchronization. A beta release must nevertheless never be the only copy of
show-critical preparation.

## Reporting a field result

Open a GitHub issue with:

- Lumi version and macOS version;
- Mac model;
- player, mixer and controller models plus firmware;
- SoundSwitch version;
- USB filesystem and whether one or multiple trusted sources were connected;
- the exact sequence that succeeded or failed;
- a Diagnostics export when available.

Do not upload music, USB databases, SoundSwitch projects, credentials, remote
control URLs or other personal data. Suspected security problems belong in a
private vulnerability report as described in [SECURITY.md](../SECURITY.md).

## Support boundary

The current supported baseline is Apple Silicon, macOS 15 or newer, rekordbox
OneLibrary USB media and the integrations listed in the user guide. Compatibility
with other combinations is learned through the beta; it is not implied merely
because a device uses MIDI or Pro DJ Link.
