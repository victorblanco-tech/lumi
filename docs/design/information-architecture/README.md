# Lumi desktop information architecture

- Status: **Accepted and implemented baseline**
- Accepted: **2026-08-06**
- Product language: **English**
- Delivery story: [E2A-19 – Reorganize Library, Integrations, and Settings around user tasks](https://github.com/victorblanco-tech/lumi/issues/88)

## Product principle

Lumi groups configuration by the task the DJ is performing, not by the internal
technology that implements it. Provider-specific setup remains close to the
input, source, or output it configures. Global Settings contains only behavior
that applies across providers.

## Primary navigation

```text
Live
Library
  Tracks
  Import & Sources
Plans (later)
Integrations
  Overview
  Pro DJ Link
  Lighting Outputs
  Diagnostics
Settings
  General
  Phrase Model
  Planning Defaults
```

## Placement rules

### Library

`Library > Tracks` contains the persistent Track Lighting Editor and track
browser. `Library > Import & Sources` owns trusted USB discovery, safe synchronization,
source health, changes/conflicts and source-specific initial phrase mapping.

Only mounted USB Device Libraries are product sources. XML and direct local
Rekordbox import are retained solely as migration internals. The deterministic
demo source remains available for local rehearsal and automated verification.

Source phrase mapping is applied only to the initial imported baseline. Edited
Lumi phrases remain independent after import.

### Integrations

`Overview` is a calm operational summary of the signal chain. It shows active
providers and health, then deep-links to the owning screen. It does not contain
duplicate editable configuration or repair actions.

`Pro DJ Link` owns direct player and mixer discovery plus live transport through the
provider-neutral `DeckSource` boundary.

`Lighting Outputs` owns SoundSwitch and future output profiles. The accepted
Banks & AutoLoops, Test Controller, and MIDI Status surfaces remain part of the
built-in SoundSwitch profile.

`Diagnostics` is the technical troubleshooting destination. The initial
version shows transport health and counters. [A later story](https://github.com/victorblanco-tech/lumi/issues/89) adds live traffic,
recent events and errors, logs, restart/reconnect/republish actions, test
messages, and log export. Those actions are deliberately not duplicated on
Overview.

### Settings

Settings contains only cross-provider application behavior:

- `General`: appearance and musical-key notation;
- `Phrase Model`: Lumi-owned phrase types and their stable identities;
- `Planning Defaults`: future default theme-selection and planning policies.

Pro DJ Link, SoundSwitch, MIDI routes, USB synchronization and source mapping
must not return to Settings.

## Deep-link behavior

- Integrations Overview → Pro DJ Link opens `Pro DJ Link`.
- Integrations Overview → SoundSwitch opens `Lighting Outputs`.
- Integrations Overview → USB Sources opens `Library > Import & Sources`.
- The compact technical status in Live will open Diagnostics when the later
  diagnostics story adds actionable detail.

## Verification contract

- Navigation boundaries are covered by local Swift tests.
- Existing provider-specific views are reused rather than reimplemented.
- The native app is tested by clicking every destination and the USB Sources
  cross-workspace deep link.
- The app remains fully offline and all user-facing copy is English.
