# Editor height persistence — 0.6.2-dev-10

The editor screenshot must use the user's enlarged waveform with the main
navigation collapsed. Do not change waveform rendering, colors or zoom to make
the screenshot. Preserve the user's current editor/browser divider choice.

## Change

AppKit's split-view autosave describes current frames, not necessarily the user's
preferred height. SwiftUI reconstruction and automatic resizing can replace those
frames. Keep a separate preferred editor height, migrate the existing saved
divider on first use, and restore it after the native split view is installed.
Only a deliberate divider drag updates this preference. A small window may clamp
the displayed height without replacing the preference for the next opening.

The normal initial height remains 692.5 points. The user's observed 717-point
height is migrated locally, not shipped as a user-specific application default.
There are no waveform renderer, engine, USB or lighting timing changes.

## Verification

- 61 Library workspace tests passed, including migration, persistence across
  automatic autosave changes, deliberate updates and malformed legacy values.
- Native Dev build, signed package checks and installation succeeded. Dev-10
  launched. Navigation/relaunch/real divider-drag acceptance remains pending:
  Computer Use returned ScreenCaptureKit error -3811 on the fullscreen action
  and again on the following state read. Do not claim headed acceptance.
- Original-resolution screenshot: blocked by native macOS capture permission
  (`CGPreflightScreenCaptureAccess() == false`). Do not publish a compressed
  Computer Use screenshot as a replacement.

## Screenshot follow-up

The earlier working method was recovered from the task history: a temporary
Lumi build exported its own rendered AppKit window at Retina scale. This now
produced and visually verified a 3600 × 2260 original Editor PNG with collapsed
navigation and the user's 717-point pane. See
`../development/retina-documentation-captures.md` for the repeatable procedure.

Normal track loading was observed resetting the pane to 620 points despite the
717-point preference. That remaining layout issue is not solved by exporting
the documentation image at the preferred height. Keep the UI acceptance open.
