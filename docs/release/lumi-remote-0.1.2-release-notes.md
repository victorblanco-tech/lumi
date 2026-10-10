# Lumi Remote 0.1.2 — QR pairing isolation

Focused pairing patch; no Live rendering, controls, timing or lighting changes.

- Production alone registers `lumi://`; existing Production Mac QR codes keep working.
- Dev registers `lumi-dev://` and RC registers `lumi-rc://`.
- The client rejects pairing links belonging to another channel, before beginning pairing.
- The Mac QR generator chooses the matching channel scheme. Invitation contents,
  expiry, approval, Keychain storage, TLS pinning and discovery remain unchanged.

Update both Production and Dev iPhone apps if the older Dev app intercepts a
Production QR code. Existing pairings and app data are preserved. Production works
with Lumi 0.6.3 without a Mac update. Creating new Dev/RC pairings requires the
corresponding Mac QR-generation update; existing Dev/RC pairings keep working.

Validation covers the full three-by-three channel acceptance matrix, invitation
expiry, shared route policy, source registration isolation and Mac QR contents.
Device installation and a fresh camera scan are checked separately; this note
does not imply App Store or TestFlight distribution.
