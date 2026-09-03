# Lumi Remote for iPhone

Lumi Remote is the native booth companion for Lumi on macOS. It intentionally
contains only Live Players, compact integration health and revision-safe booth
controls. It does not contain Local Playback, Library, USB Sync or Track Edit.

## Current development status

Version 0.1.0 is available as a source-installed public beta for iOS 18 or
newer. Bonjour discovery, release-channel isolation, pinned TLS, native
Camera deep-link pairing, Keychain credential storage, Mac approval and
revocation, scoped Remote decoding and revision-safe booth controls are
implemented. The independently packaged, opt-in Mac Remote Gateway receives
only the path-free Live projection from the engine and cannot sit in the Pro DJ
Link, SoundSwitch or Ableton Link paths.

The complete connection and moving Live presentation have been exercised on a
physical iPhone as well as the Simulator. The app never substitutes demo show
state. Multi-phone transfer, broader device coverage and extended combined
show soaks remain public-beta work rather than reasons to weaken the isolation
boundary.

## Local validation

```bash
swift test -Xswiftc -warnings-as-errors \
  --package-path apps/ios/Packages/LumiRemoteClient
swift test -Xswiftc -warnings-as-errors \
  --package-path apps/ios/Packages/LumiRemoteFeature
xcodebuild \
  -project apps/ios/LumiRemote.xcodeproj \
  -scheme LumiRemote \
  -configuration Dev \
  -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO build
```

Physical-iPhone testers currently build and sign the Dev configuration with
their own Apple Account in Xcode. The step-by-step route and seven-day Personal
Team limitation are documented in `docs/user-guide/iphone-remote.md`.

## Version and release

The independent product version is in `apps/ios/VERSION`. Tags use
`lumi-remote-vX.Y.Z`; that tag validates and creates a controlled draft GitHub
Release with an iOS Simulator validation artifact. It does not imply a signed
physical-iPhone build. TestFlight remains a future, separate Apple signing and
App Store Connect gate. Until then, testers install the app from source with
Xcode; the Simulator archive remains unsuitable for physical iPhones.

Architecture and product details:

- `docs/design/iphone-remote/README.md`
- `docs/user-guide/iphone-remote.md`
- `docs/architecture/adr/0040-isolated-local-remote-gateway.md`
- `docs/architecture/adr/0041-independent-monorepo-product-releases.md`
- `docs/planning/epic-09-iphone-remote-beta.md`
