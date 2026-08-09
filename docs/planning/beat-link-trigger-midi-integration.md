# Beat Link Trigger MIDI integration

## Goal

Prove and ship the first live Deck A/Deck B adapter without coupling Lumi to BLT
or SoundSwitch internals. The result must update the existing Live Decks
Live workspace and fail silent when a frame is incomplete or unsupported.

## BLT setup

Create two triggers:

| Setting | Trigger 1 | Trigger 2 |
| --- | --- | --- |
| Comment | Lumi Deck 1 | Lumi Deck 2 |
| Watch | Player 1 | Player 2 |
| MIDI Output | Lumi Deck Input | Lumi Deck Input |
| Message | Custom | Custom |
| Enabled | Never | Never |
| Expression | same Tracked Update expression | same Tracked Update expression |

The exact versioned expression is available in Lumi under **Settings →
Integrations → Deck Inputs → Beat Link Trigger → Copy Tracked Update Expression**. Lumi shows
the same protocol version and live counters beside that user-facing template.

The expression deliberately distinguishes BLT's Shallow Playback Simulator
from real device updates. The simulator writes an already pitch-adjusted value
into its raw BPM field and also supplies the pitch multiplier, causing BLT's
normal `effective-tempo` accessor to apply pitch twice. For simulated updates,
the expression therefore uses raw BPM as effective BPM and divides out pitch
for immutable track BPM. Real Pro DJ Link updates continue to use BLT's official
`effective-tempo` value without this compatibility correction.

## Frame v3

All values are seven-bit Control Change values. Multi-byte numbers use
least-significant seven-bit chunks first.

| CC | Value |
| --- | --- |
| 16 | flags: loaded 1, playing 2, master 4, on-air 8 |
| 17–20 | rekordbox ID, 28 bit |
| 21 | source player |
| 22 | source slot: SD 1, USB 2, collection 3, CD 4 |
| 23–25 | original track BPM × 1000, 21 bit |
| 26–28 | absolute beat number, 21 bit |
| 29–31 | duration in seconds, 21 bit |
| 32 | frame sequence modulo 128 |
| 33–35 | effective deck BPM × 1000 (including pitch), 21 bit |
| 36–40 | shallow-simulator metadata signature, 35 bit; zero on real decks |
| 119 | protocol version and atomic commit; value 3 |

MIDI channel 1 is Deck A/Player 1 and channel 2 is Deck B/Player 2. Channel 16
is reserved for the independent Lumi → SoundSwitch output profile.

## Acceptance evidence

- `Lumi Deck Input` appears as a selectable BLT MIDI output.
- Raw Note/CC traffic increments `receivedMessageCount` without affecting the
  SoundSwitch source.
- Complete Player 1 and Player 2 frames render as fixed Deck A and Deck B.
- Changing Master moves Live between those fixed deck positions.
- Play/pause, beat and pitch-adjusted effective BPM updates appear without a local simulator in Lumi.
- In the Shallow Playback Simulator, displayed BPM and Lumi effective BPM are equal above, below and at zero pitch.
- Unknown tracks show `AUTO HELD` and never trigger an automatic MIDI output.
- Real decks resolve the Device Library ID from the synced USB/SD source.
- The shallow simulator resolves its metadata signature because BLT publishes
  the fixed Rekordbox ID `42` for every simulated track.
- Partial, duplicate and foreign messages are counted and do not mutate state.
- Disconnect/restart produces no unsolicited SoundSwitch output.

## Current limitation

BLT MIDI v3 deliberately remains a compact transport and identity protocol. It
does not stream RGB waveform, beatgrid or phrase analysis. Lumi hydrates those
from its read-only synchronized Rekordbox Device Library alias. An unmatched or
ambiguous identity remains a safe transient `External track <id>` with `AUTO
HELD`; it never falls back to a fuzzy realtime guess.
