# Lumi Remote 0.1.0 TestFlight readiness

This checklist separates source readiness from Apple distribution. A GitHub tag
can create a Simulator validation release without signing; a physical beta must
be uploaded to App Store Connect and distributed through TestFlight.

## Product freeze

- [x] Independent version and tag contract: `apps/ios/VERSION` and
      `lumi-remote-vX.Y.Z`.
- [x] Production bundle identifier reserved in source as
      `co.victorblan.tech.lumi.remote`.
- [x] iOS 18 deployment target, iPhone-only product and Production Bonjour
      channel are explicit.
- [x] High-resolution opaque App Store icon and all required iPhone icon sizes
      are part of the asset catalog and verified in local/CI builds.
- [x] Local Network purpose text and exempt-encryption declaration are present.
- [x] Public privacy information, user guide and beta release notes are present.
- [x] Freeze the exact Lumi macOS 0.6.0 and Remote 0.1.0 compatibility pair.

## Local verification

- [x] Remote Client and Remote Feature package tests pass with warnings as
      errors.
- [x] Generic iOS Simulator Dev build succeeds and contains `AppIcon`.
- [x] Physical iPhone discovery, pinned-TLS pairing and saved-credential
      reconnect are proven.
- [x] Physical waveform, BPM, Hot Cue, master handoff and future-phrase control
      acceptance are proven on the reference setup.
- [x] A physical Animation Hitches trace demonstrated sustained near-native
      display cadence with no interaction delay above 33 ms.
- [ ] Repeat rotation, background/foreground, Wi-Fi interruption and Mac gateway
      restart on the exact release build.
- [ ] Run a two-Player plus SoundSwitch/Ableton Link soak with the Remote
      connected, disconnected and foregrounded again.
- [ ] Verify a second paired phone remains Viewer until Controller transfer.

## App Store Connect setup

- [ ] Enroll the release owner in the Apple Developer Program.
- [ ] Sign in to the release Apple account in Xcode and regenerate the
      development/distribution provisioning profiles. The local source build
      currently succeeds unsigned, but Xcode reports no active account/profile
      for the selected local development team when a new physical-device build
      is requested.
- [ ] Register the production App ID `co.victorblan.tech.lumi.remote` and enable
      only the entitlements used by the target.
- [ ] Create the App Store Connect app record as **Lumi Remote**.
- [ ] Add the privacy-policy URL from the published GitHub Pages site.
- [ ] Complete App Privacy using the audited local-only data behavior.
- [ ] Create Internal and External TestFlight groups.
- [ ] Add beta review contact details, the same-LAN requirement and a complete
      pairing/demo path for review.
- [ ] Provide representative iPhone screenshots and beta feedback contact.

## Archive and publish

1. Set `apps/ios/VERSION`, `LUMI_REMOTE_PRODUCT_VERSION` and the Apple build
   number to the release values.
2. Select the release team in Xcode without committing personal signing
   material.
3. Archive the `LumiRemote` scheme using the **Release** configuration for
   `Any iOS Device (arm64)`.
4. Validate the archive, confirm the production bundle ID and upload it to App
   Store Connect.
5. Install the processed build through TestFlight on a clean physical iPhone.
6. Repeat the physical release matrix with the production Lumi Mac build.
7. Submit the build to External TestFlight review.
8. Tag the exact source commit `lumi-remote-v0.1.0`; review and publish the
   controlled GitHub draft with the matching release notes.

The source-distributed 0.1.0 Public Beta intentionally stops before the App
Store Connect steps above. Testers build the immutable tag with their own Apple
Account; TestFlight remains a later distribution upgrade rather than a runtime
dependency.

Never commit a signing certificate, provisioning profile, App Store Connect API
key or Apple account credential. CI signing is a later hardening step after the
first manual TestFlight path is proven.
