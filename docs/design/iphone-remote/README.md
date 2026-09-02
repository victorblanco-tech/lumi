# Lumi Remote for iPhone

Status: **Proposed for product validation**  
Target: **0.6.0 – iPhone Remote Beta**

## Product purpose

Lumi Remote is the booth companion for the autonomous Lumi service running on
the Mac. It shows the current Live Player and the prepared next Player, and lets
the DJ safely adjust operational state and future Light Plan choices without
leaving the mixer.

It is deliberately not a mobile copy of the desktop application. The first
version has no Local Playback, Library, USB synchronization, Track Editor,
output-profile configuration or developer diagnostics. Preparation stays on
the Mac; the iPhone is a focused Live surface.

## Global Live controls

The first screen opens directly into Live. Its compact top area contains the
same authoritative controls and terminology as macOS:

- connection to the paired Mac;
- Pro DJ Link, Light Output and Ableton Link health;
- Ableton Link on/off and current master BPM;
- the applied lighting timing offset, including pending-next-phrase state;
- `OFF`, `ARM`, `START` and `PAUSE` operation controls.

Healthy integrations stay visually quiet. A problem changes the compact status
to amber or red; tapping it opens the three user-facing integration states and
one actionable recovery hint. Detailed traffic and developer diagnostics remain
on the Mac.

Operation colors remain identical on both clients:

- Off: white;
- Armed: orange;
- Start: red;
- Pause: blinking orange.

Moving from Start to Off requires confirmation on iPhone to avoid an accidental
show stop. Every other operation command receives immediate haptic feedback but
is only presented as authoritative after the Mac engine accepts its revision.

## Portrait composition

```text
┌─────────────────────────────────────┐
│ Lumi       MacBook Pro · Connected  │
│ ● PDL  ● LIGHT  ● LINK 140.0  −20ms │
│  OFF      ARM     START     PAUSE   │
├─────────────────────────────────────┤
│ PLAYER 1 · CDJ-1500X                │
│ MASTER · LIVE NOW                   │
│ 90s Bitch                           │
│ RGB waveform + fixed playhead       │
│ hot cues                            │
│ phrase band                         │
│ active and upcoming Light Plan      │
├─────────────────────────────────────┤
│ PLAYER 2 · CDJ-1500X                │
│ PLAN READY                          │
│ Favorite Regrets                    │
│ RGB waveform                        │
│ hot cues                            │
│ phrase band                         │
│ complete prepared Light Plan        │
└─────────────────────────────────────┘
```

The master Player is shown first in portrait because the live performance is
the primary mobile task. Each card always retains its real `Player n` identity
and detected hardware model. A clear role transition moves the Live treatment
when the master changes; a Player is never renamed or fabricated.

Each loaded card shows:

- track color, title, artist, effective BPM, key and remaining time;
- the same cached RGB waveform, Rekordbox beatgrid and Hot Cue markers as macOS;
- a fixed Live playhead and phrase band directly below the waveform;
- a proportional Light Plan timeline aligned to the same beat space;
- the active or selected Theme, AutoLoop and applied Static Look.

Pinch zoom and horizontal inspection are visual only. The phone never seeks a
physical Player. A `Follow Live` action returns an inspected Live card to its
fixed-playhead viewport.

## Landscape composition

Landscape keeps the compact global controls in one row and shows numbered
Players side by side. Player 1 stays left and Player 2 stays right; only the
Master and Live treatment moves. A phrase selection opens a compact bottom
sheet without resizing or shifting either Player surface.

## Future-phrase editing

Tapping a not-yet-started phrase in either phrase band selects that exact plan
cue and opens a bottom sheet. It exposes the safe controls already owned by the
Live engine contract:

- Theme from this phrase onward;
- AutoLoop for this phrase;
- lock or unlock this choice;
- effective Static Look as plan information in the first beta.

Active and completed phrases are read-only. Phrase boundaries and Phrase Roles
cannot be edited from the iPhone. A pending choice is shown immediately, but it
is discarded and refreshed if the engine reports a plan revision conflict.

## Connection and safety states

- **Connected:** authoritative state and controls are live.
- **Reconnecting:** the last state is retained with its age, dimmed, and all
  mutations are disabled. Nothing is queued for later execution.
- **Mac found, not paired:** only the pairing flow is available.
- **Mac unavailable:** the app explains local-network requirements and can retry
  discovery; it never substitutes demo state in the production Live screen.
- **App backgrounded or phone locked:** Lumi on the Mac continues the show. On
  foreground the phone reconnects and requests one complete current snapshot.

## Mac companion surface

`Integrations > iPhone Remote` on macOS contains:

- Remote Gateway enabled/disabled and connection health;
- `Pair New iPhone`, presenting a QR code and short confirmation code;
- paired device name, last seen time and Controller/View-only permission;
- revoke access;
- a clear statement that the remote works only on the local network and is
  never part of lighting execution.

The first beta permits one active Controller lease. Additional paired clients
are view-only until control is explicitly transferred on the Mac. This prevents
two phones from issuing conflicting booth actions.

## Accessibility and booth ergonomics

- minimum 44-point touch targets;
- Dynamic Type without hiding operation state or Player identity;
- color is always paired with text and shape;
- haptics distinguish accepted, rejected and destructive actions;
- an optional keep-awake setting applies only while the Remote Live screen is
  foregrounded;
- no essential action depends on a hover gesture or tiny waveform marker.

