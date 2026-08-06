# SoundSwitch Autoloops – built-in Output Profile

- Status: **Accepted baseline, demo implementation complete**
- Accepted: **2026-08-04**
- Product language: **English**
- Architecture: [ADR-0015](../../architecture/adr/0015-soundswitch-autoloop-surface-en-virtuele-midi-controller.md)

## Product outcome

`Integrations > Lighting Outputs` presents SoundSwitch Autoloops as a bank-first surface. Lumi is shown
as the virtual MIDI controller at the same level as an optional physical
controller. The UI never suggests that Lumi controls or depends on Control One.

## Views

### Banks & Autoloops

- four clearly numbered and named banks;
- organization prepared for Theme, Genre, Function, and Custom;
- 32 stable AutoLoop positions per bank, 128 total;
- mapped and available states;
- a bank-first mapping surface with four bank selectors and all 32 positions of
  the selected bank visible together;
- direct button selection with a selected bank and AutoLoop inspector;
- editable Bank Name and exact SoundSwitch AutoLoop Name;
- editable Lumi Phrase Type per button;
- explicit separation between logical mapping and later MIDI binding.

Each button is one atomic mapping: `Bank + Button + AutoLoop Name + Phrase Type`.
The same button number may use a different Phrase Type in another bank. Internal
mapping identifiers are never presented as editable Variant names.

The spatial layout is part of the product contract: selecting Bank 1, 2, 3, or
4 replaces the grid with that bank's 32 unique positions. No page, range, or
shared slot state is presented. The inspector augments that surface; it never
replaces it.

### Test Controller

- `Lumi Virtual MIDI Controller` identity;
- the same bank selector and 32 positions for the selected bank;
- the exact same names and Phrase Types as the Banks view;
- no second configuration model;
- explicit learn and runtime-test actions that never run automatically.

### MIDI Status

- active `Lumi Virtual MIDI → SoundSwitch` route and source status;
- CoreMIDI transport and a separate Ableton Link timing route;
- the validated bank-switch delay;
- catalog preflight separate from MIDI-address preflight;
- integration checks including parallel Control One use and DMX output through it;
- explicit publish, stop, learn and trigger controls for diagnostics.

## Model boundary

```text
Library phrase
  -> selected Bank/Theme + Phrase Type
  -> SoundSwitch Output Profile
  -> Bank + Button + exact AutoLoop Name + MIDI sequence
  -> SoundSwitch
```

The built-in profile owns the canonical MIDI addresses and validated sequencing.
Future output profiles persist their provider-specific mapping without changing
the generic controller and status-page model.
