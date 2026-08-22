# E6-03 – Light Plans workspace

Status: **Done** | Priority: **P0** | Effort: **8**

## User value

As a user I can understand, configure and preview automatic planning in one clean
workspace.

## Acceptance criteria

- `Light Plans` replaces the Plans placeholder in primary navigation.
- `Theme Strategy` explicitly maps SoundSwitch Banks to coherent Lumi Themes and
  configures fallback, weight, Track Color affinity and Theme cooldown.
- `AutoLoop Rules` shows Phrase Roles, mapped candidates, Selection Weight,
  track-color behavior and cooldown status.
- The physical mapping is read-only here and links to Lighting Outputs.
- `Plan Preview` selects a real Library track and defaults to automatic Theme
  selection through the same compiler as runtime. A manual Theme is an explicit
  Preview override; both paths show phrases, AutoLoops, addresses and reasons.
- `New variation` changes the recorded seed; reloading does not.
- Empty/error/loading states are clear and do not shift the surrounding layout.
- Controls have full hit targets, keyboard accessibility and responsive scrolling.
- Contextual `i` help explains compile-time behavior, Selection Weight, Track
  Color and every Repeat Protection setting without making the workspace noisy.
- Track Color shows the named Rekordbox color catalog actually present across
  the complete Lumi Library, with a per-color track count; it is not derived
  from the currently visible 50-row page.
- A trusted OneLibrary USB resync imports information-only color changes for new
  and existing tracks while monotone metadata provenance prevents an older
  backup USB from downgrading the active color.
