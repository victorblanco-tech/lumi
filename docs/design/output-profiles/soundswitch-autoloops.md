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
replaces it. Slot numbering follows SoundSwitch/Control One column-major order:
1–8 run top-to-bottom in the first column, followed by 9–16, 17–24 and 25–32.
The mapping surface, Learn surface, Test Controller and MIDI adapter must all
use this exact same coordinate system.

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

## Version 4 slot migration

The initial 4×32 mapping view accidentally stored the four-column screen
position in row-major order while Learn and Test Controller already used the
SoundSwitch column-major order. Catalog defaults version 4 applies a one-time
bijection from the old screen positions to the physical slots and preserves
every user-authored Bank name, Phrase Type and exact AutoLoop name. Generated
placeholder cells are removed rather than treated as executable mappings.
The SQLite catalog replacement persists the upgraded defaults version in the
same transaction, making the migration restart-safe and idempotent.
Tracks using automatic Phrase Type resolution then select only a Theme whose
phrases are fully mapped; an unconfigured Bank cannot be chosen at random.
