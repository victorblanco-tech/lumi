# Story E4-02: Direct Pro DJ Link input and SoundSwitch timing

## User outcome

As a DJ, I can run Lumi directly beside supported Pro DJ Link decks without
configuring Beat Link Trigger expressions. Loaded tracks are recognized using
their actual Rekordbox analysis identity and SoundSwitch follows the master
deck's effective tempo and musical phase.

## Functional slice

This story is delivered in visible, independently testable increments:

1. **Bridge health** — Deck Inputs shows the bundled bridge version, pinned
   Beat Link version, process health and discovered players.
2. **Live transport** — Live Decks uses direct play, pause, master, BPM, beat
   and position observations through the existing deck-source contract.
3. **Track recognition** — loaded media identity and Beat Link signature
   reconcile against the read-only Rekordbox Device Library mirror.
4. **Rich deck data** — Lumi hydrates waveform, beat grid and cue information
   when available without blocking transport.
5. **Lighting sync** — the engine publishes master timing through an Ableton
   Link output provider while existing MIDI notes select SoundSwitch Autoloops.
6. **Fallback and diagnostics** — source loss holds automatic output, reports a
   single actionable diagnostic and permits an explicit fallback to BLT.

## Build order

### E4-02A — Bridge foundation

- Java 21 helper module with pinned `beat-link` dependency.
- Versioned NDJSON envelopes on stdout and commands on stdin.
- Structured stderr diagnostics and deterministic protocol fixtures.
- Rust bridge decoder and supervisor contract tests.
- Local build script; no GitHub Actions consumption during development.

### E4-02B — Direct deck observations

- Device discovery and lifecycle.
- Virtual CDJ read-only session.
- Player status, master, effective BPM and beat events.
- Freshness lease and safe disconnect behavior.
- Replay fixtures captured without copyrighted audio.

### E4-02C — Identity and analysis

- Mounted-media identity and Rekordbox track references.
- Metadata, beat grid, waveform, cue list and signature retrieval.
- Exact identity/signature reconciliation and persisted aliases.
- Explicit ambiguous/unknown result; no realtime fuzzy auto-activation.

### E4-02D — SoundSwitch timing output

- Provider-neutral engine timing authority.
- Ableton Link publisher with tempo, beat/bar phase and start/stop behavior.
- Local Playback and Pro DJ Link feed the same output port.
- MIDI Clock fallback and mutually exclusive clock ownership.
- Timing offset changes apply at a safe future boundary.

### E4-02E — Packaging and physical acceptance

- Minimal bundled Java runtime and signed helper in all macOS channels.
- Third-party license/source inventory and reproducible dependency lock.
- No separately installed Java, BLT or internet dependency.
- Physical player, mixer, SoundSwitch, Control One and DMX acceptance run.

### E4-02S — USB-backed network simulator — implemented

- Development-only Mac mini player; excluded from production packaging.
- Read-only DeviceSQL and ANLZ track catalog from the same show USB.
- Real Pro DJ Link discovery, status and beat packet generation for one player.
- Rekordbox ID, exact beat grid, play/pause, seek, pitch, master and on-air.
- Token-protected browser controls and stable HTTP API for remote agent tests.
- `prolink-simulatorctl.sh` wrapper and local packet/config verification.
- Verified on `DJ VIC GRAY`: 1,138 tracks and 64 playlists indexed read-only;
  track ID 1256 resolved as `90s Bitch - Extended Mix` with an exact beat grid.
- Generated announcement, status and beat packets parse successfully through
  beat-link 8.0.0 with USB identity and effective tempo intact.
- Two-host Mac mini → MacBook discovery remains the next POC step; local bridge
  coexistence was correctly blocked while Beat Link Trigger owned UDP 50000/50001.

## Acceptance criteria

- Lumi starts and stops the bridge automatically with the engine.
- A bridge crash cannot stop local playback or corrupt a lighting plan.
- Two fixed Lumi deck identities follow two physical player identities.
- Pitch changes update effective BPM without double application.
- Master changes preserve the correct deck and do not select an unrelated
  Autoloop.
- The same track exported to different media resolves by content signature.
- An unknown or ambiguous track remains visible but automatic lighting is held.
- SoundSwitch Autoloops follow tempo and bar phase while Lumi selects them over
  the independent virtual MIDI output.
- Control One continues to operate alongside Lumi.
- All bridge and Rust contract tests run locally without paid CI minutes.
- Simulator packet tests and API acceptance checks run locally without paid CI
  minutes.

## Out of scope

- Sending load, play, sync or master commands to physical players.
- Replacing SoundSwitch fixture programming or DMX output.
- A native Rust implementation of the Pro DJ Link protocol.
- Full CDJ audio, display, media-server or remote-command emulation.
- Removing the BLT fallback before physical acceptance succeeds.
