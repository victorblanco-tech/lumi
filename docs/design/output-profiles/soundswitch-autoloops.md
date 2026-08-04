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
- eight stable AutoLoop buttons per bank, 32 total;
- mapped and available states;
- selected bank and Autoloop inspector;
- editable Bank Name and exact SoundSwitch AutoLoop Name;
- editable Lumi Phrase Type per button;
- explicit separation between logical mapping and later MIDI binding.

Each button is one atomic mapping: `Bank + Button + AutoLoop Name + Phrase Type`.
The same button number may use a different Phrase Type in another bank. Internal
mapping identifiers are never presented as editable Variant names.

### Test Controller

- `Lumi Virtual MIDI Controller` identity;
- four bank columns with eight AutoLoop buttons each;
- the exact same names and Phrase Types as the Banks view;
- no second configuration model;
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
  -> selected Bank/Theme + Phrase Type
  -> SoundSwitch Output Profile
  -> Bank + Button + exact AutoLoop Name + MIDI sequence
  -> SoundSwitch
```

The demo projection does not persist concrete MIDI addresses. The POC determines
real mapping and sequencing before those fields become production configuration.
