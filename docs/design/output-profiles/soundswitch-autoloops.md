# SoundSwitch Autoloops – built-in Output Profile

- Status: **Accepted baseline, demo implementation complete**
- Accepted: **2026-08-04**
- Product language: **English**
- Architecture: [ADR-0015](../../architecture/adr/0015-soundswitch-autoloop-surface-en-virtuele-midi-controller.md)

## Product outcome

Settings presents SoundSwitch Autoloops as a bank-first surface. Lumi is shown
as the virtual MIDI controller at the same level as an optional physical
controller. The UI never suggests that Lumi controls or depends on Control One.

## Views

### Banks & Autoloops

- four clearly numbered and named banks;
- organization prepared for Theme, Genre, Function, and Custom;
- 32 stable Autoloop positions per bank;
- mapped, incomplete, and available states;
- selected bank and Autoloop inspector;
- logical Group, Phrase Role, and Variant identity;
- editable bank, Variant, and logical Autoloop names where supported;
- explicit separation between logical mapping and later MIDI binding.

The demo preset organizes every bank as a Theme and projects the same logical
role/variant row onto the same numbered position in every bank. A missing matrix
cell therefore stays visible at a stable position.

### Virtual Controller

- `Lumi Virtual MIDI Controller` identity;
- four bank controls;
- four pages of eight Autoloop buttons;
- the same selected position and logical binding as the Banks view;
- utility controls represented for layout completeness;
- live test disabled until the CoreMIDI POC.

### MIDI & POC

- planned `Lumi Virtual MIDI → SoundSwitch` route;
- CoreMIDI transport and a separate Ableton Link timing route;
- bank-switch delay explicitly marked as a measurement;
- catalog preflight separate from MIDI-address preflight;
- POC acceptance including parallel Control One use and DMX output through it;
- no live MIDI send before the POC gate passes.

## Model boundary

```text
Library phrase
  -> logical Group/Theme + Phrase Role + Variant
  -> SoundSwitch Output Profile
  -> Bank + Autoloop position + MIDI sequence
  -> SoundSwitch
```

The demo projection does not persist concrete MIDI addresses. The POC determines
real mapping and sequencing before those fields become production configuration.
