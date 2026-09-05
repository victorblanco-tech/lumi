# Native Retina documentation captures

## Working method

Lumi can export **its own rendered AppKit window content**, at the window's
native backing scale, from an opt-in local documentation build. This was the
successful capture method on 2026-08-30 and was verified again on 2026-09-05.
It is not an external desktop screenshot and does not capture other apps.

Build with `LUMI_DOCUMENTATION_CAPTURE` added to
`SWIFT_ACTIVE_COMPILATION_CONDITIONS`, preserving the inherited conditions. Use
a temporary copy of the existing development app and its packaged resources;
replace and ad-hoc sign only that temporary bundle. Do not replace the user's
installed application. Only one app with the Dev bundle identity should run at
a time. Normal builds do not enable the capture condition or export menu.

In the temporary build:

1. Open the real prepared Library track in the actual Editor.
2. Collapse the main navigation and make the window full screen.
3. Select **File → Export Retina Documentation Image**.
4. Inspect `/tmp/lumi-editor-retina.png` and verify its actual pixel dimensions.
5. Copy the inspected original PNG to the documentation assets. Never upscale
   a Computer Use JPEG or use synthetic waveform data as a product screenshot.
6. Close the temporary build and leave the normal installed Dev app open.

The editor export uses the user's stored preferred editor height. This is a
documentation-only layout step, **not proof that normal track loading restores
that preference correctly**. Track-load/layout persistence remains a separate
UI acceptance item.

## Verified output

2026-09-05: 90s Bitch, actual Dev Library revision 38, collapsed navigation,
717-point editor pane, 3600 × 2260 PNG, approximately 940 KiB. Text and the
accepted RGB waveform were inspected. No waveform renderer/color code changed.

## Why the other routes failed

- `screencapture -l` could not capture the known window in this tool process.
- Native capture preflight returned false despite Computer Use being available.
- The current Computer Use image was confirmed to be a 1223 × 768 JPEG, not a
  Retina original merely resized for display.
- Keyboard and Screenshot-app launch attempts did not yield an original file.

Do not repeat those routes indefinitely or alter privacy settings. The in-app
export avoids depending on external screen recording in the first place.
