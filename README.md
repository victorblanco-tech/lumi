<p align="center">
  <img src="docs/assets/brand/lumi-mark.png" alt="Lumi" width="112">
</p>

<h1 align="center">Lumi</h1>

<p align="center">
  Phrase-aware lighting automation for DJ sets.
</p>

<p align="center">
  <a href="https://github.com/victorblanco-tech/lumi/releases">Download</a>
  ·
  <a href="docs/user-guide/README.md">User guide</a>
  ·
  <a href="https://github.com/victorblanco-tech/lumi/issues">Report an issue</a>
</p>

Lumi prepares the lighting for the track that is playing and the track that is
coming next. It combines your own phrase structure with the beatgrid and deck
state from the rekordbox ecosystem, then triggers the right SoundSwitch
AutoLoop at the right moment.

The complete show runs locally on your Mac. SoundSwitch remains responsible for
fixtures and DMX output; Lumi acts like a virtual lighting operator beside your
normal controller.

![The complete Lumi workflow from trusted USB source to SoundSwitch DMX](docs/assets/lumi-workflow.svg)

## What Lumi does

- Imports selected playlists, beatgrids, waveforms, Hot Cues and track metadata
  from trusted rekordbox OneLibrary USB media.
- Lets you create and protect Lumi-owned phrases such as Intro, Breakdown,
  Synth, Build-up, Pre-drop and Drop.
- Builds a full-track Light Plan before playback, with coherent SoundSwitch
  Themes, Track Color preferences and repeat protection.
- Watches both players through read-only Pro DJ Link and keeps the current and
  next track visible side by side.
- Sends exactly one mapped MIDI action when an AutoLoop or verified Static Look
  needs to change.
- Relays the live master BPM to SoundSwitch through an isolated Ableton Link
  connection.
- Supports Local Playback for preparation and dry runs without DJ hardware.

## The signal path

```text
rekordbox OneLibrary USB ──> Lumi Library + phrases + Light Plans
                                      │
CDJ / DJM ── Pro DJ Link ─────────────┤
                                      ├── MIDI ──> SoundSwitch ──> DMX
                                      └── Ableton Link ──> SoundSwitch tempo
```

Lumi never writes to rekordbox media during normal import or synchronization.
Your SoundSwitch project, fixtures and DMX interface stay under SoundSwitch's
control. A physical Control One can continue to run alongside Lumi.

## Requirements

- Apple Silicon Mac
- macOS 15 or newer
- rekordbox OneLibrary USB media for library synchronization
- SoundSwitch with a MIDI input for lighting output
- Pro DJ Link-compatible players for Live Decks

Only the Mac is needed for Library work, Track Editor, Light Plan preview and
Local Playback. Internet access is not required while using Lumi.

## Start here

1. Download the DMG and checksum from [GitHub Releases](https://github.com/victorblanco-tech/lumi/releases).
2. Install Lumi and complete the one-time macOS **Open Anyway** step if needed.
3. Add a trusted USB source and synchronize the playlists you want in Lumi.
4. Review the beatgrid and phrases, then mark prepared tracks **Ready for Show**.
5. Map your SoundSwitch Banks, AutoLoops and optional Static Looks.
6. Configure and preview the Theme Strategy in **Light Plans**.
7. Open **Live**, choose **Live Decks** or **Local Playback**, then use
   **Arm** and **Start** when you are ready.

The [user guide](docs/user-guide/README.md) explains the complete workflow,
operation states, timing offset, backups and troubleshooting.

## Project status

Lumi is in active development and the first broader production release is being
prepared. The current reference setup uses two CDJ-1500X players, a DJM-V5,
SoundSwitch, Control One and a DMX lighting rig. Back up your Lumi data before
upgrading and test a new release with your own show setup before relying on it
live.

## For contributors

Product and user documentation starts at [docs/index.md](docs/index.md).
Architecture, decisions, development setup and release procedures remain in:

- [Architecture](docs/architecture/README.md)
- [Development guide](docs/development/README.md)
- [Planning and delivery history](docs/planning)
- [Release process](docs/release/README.md)
- [Contribution guide](CONTRIBUTING.md)

## License and trademarks

Copyright © 2026 Victor Blanco. Source code is available under the
[Eclipse Public License 2.0](LICENSE). Lumi names and branding are covered by
[the trademark notice](TRADEMARKS.md). Third-party components and licenses are
listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Lumi is an independent project and is not affiliated with AlphaTheta,
rekordbox, inMusic or SoundSwitch.
