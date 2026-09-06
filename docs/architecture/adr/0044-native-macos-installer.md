# ADR 0044 — Fixed-location native macOS installer

Status: accepted; release-artifact acceptance testing in progress.

## Problem

The DMG linked to `/Applications/Lumi` (or its Dev/RC subfolder), which does
not necessarily exist on the destination Mac. Drag installation depended on
manual folder setup. Technical documents also crowded the DMG's main window.

## Decision

Ship a native macOS distribution package, `Install Lumi.pkg`, inside the DMG.
Keep legal notices, SBOM and complete corresponding-source archives accessible
in one `Licenses & Sources` folder. No dangling destination shortcut or loose app.

The package contains only the verified app bundle. Installer creates its exact
channel destination. Disable relocation and enforce the bundle identifier and
version checks. Use bundle upgrade semantics to remove stale files **inside the
app**, never replace the whole Lumi channel folder or Application Support.

Production has a stable package receipt. Version-named Dev/RC apps use a receipt
per version so a new version does not remove another version as obsolete payload.
The app's existing channel identifiers and database paths remain unchanged.

Use Installer's `must-close` for the corresponding app identity. Do not inject
root shell scripts, kill processes, auto-launch as root, reset databases, change
system security settings or touch sibling channels. Users must end their show
before installing. Administrator authorization is handled by macOS itself.

## Verification and release

Portable tests cover channel identity, destinations, receipts and malformed
inputs. Each produced DMG is remounted; its package is expanded and checked for
an app-only payload, fixed destination, no relocation or executable installer
scripts, version/identity enforcement and valid embedded app signature.

Native Installer UI acceptance must cover introduction, destination, permissions
and completed installation. Distinguish package inspection from actual install
evidence. First install and update preservation need verification before release.

This is a Mac packaging change, not a new Remote or simulator version. Publish
as a new Mac patch release; do not silently replace previous release artifacts.

The fixed two-icon Finder layout is checked in as a path-free `.dsstore` asset.
Its optional generator uses the MIT-licensed `ds-store` authoring library; release
builds copy the asset and need no added Python dependency or Finder automation.

Local packaging-prototype evidence (2026-09-06): native Installer upgraded the
existing 0.5.2 Production app to the unchanged, signed 0.6.2 payload. Installer
reported success; the resulting receipt points to `Applications/Lumi`. Production
and Dev SQLite files retained identical SHA-256 values before and after. The
new two-item DMG and shorter, non-scrolling Installer introduction were inspected
in Finder/Installer. This is upgrade evidence, not a clean-Mac/Gatekeeper test.

## Tradeoffs

An unsigned package may require the same explicit Privacy & Security approval as
the unsigned app, plus administrator authorization for the system installation.
The benefit is a supported installation transaction and automatic folder creation.
Signing/notarization remains a separate distribution decision.
