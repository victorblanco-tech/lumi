# Live workspace design

Status: **Accepted for native implementation**

## Desktop composition

```text
┌────────────────────────────────────────────────────────────────────┐
│ Lumi Live       Tech status       OFF · ARM · LIVE · PAUSE         │
├───────────────────────────────┬────────────────────────────────────┤
│ PLAYER 1                      │ PLAYER 2                           │
│ CDJ-1500X                     │ CDJ-1500X                          │
│ MASTER · LIVE NOW             │ PLAN READY                         │
│ metadata                      │ metadata                           │
│ RGB waveform + beatgrid       │ RGB waveform + beatgrid            │
│ phrase band + playhead        │ phrase band                        │
│ active + remaining plan       │ complete next-track plan           │
└───────────────────────────────┴────────────────────────────────────┘
```

Player 1 remains left and Player 2 remains right. Only the master role and associated
Live styling move. The non-master deck remains a planning surface.

## Interaction rules

- Active and past Live phrases are locked.
- Future Live phrases are selectable from the waveform or remaining-plan list.
- AutoLoop edits affect that phrase.
- Theme edits are scheduled from that phrase onward.
- Changes become authoritative and lock when the phrase starts.
- Next-track edits remain possible independently before the transition.

## Waveform rules

- RGB is the default style.
- The whole-track preview, beatgrid, playhead and phrase band are separate data
  layers rendered by Lumi.
- Waveform provenance is part of the state: local library, deck provider,
  simulator or unavailable.
- The current position moves over immutable cached waveform data; the UI never
  depends on streamed screenshots.

## iPhone composition

- Portrait: master deck first, planned deck below it.
- Landscape: stable numbered Players side by side when space permits.
- Phrase bands are always directly below their waveform.
- The current/future editing rule is identical to macOS.
