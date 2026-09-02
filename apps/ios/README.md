# Lumi Remote for iPhone

Lumi Remote is the native booth companion for Lumi on macOS. It intentionally
contains only Live Players, compact integration health and revision-safe booth
controls. It does not contain Local Playback, Library, USB Sync or Track Edit.

## Current development status

The native target builds for iOS 18 and has been exercised in portrait and
landscape on the iOS Simulator. Bonjour discovery, release-channel isolation,
pinned TLS, native Camera deep-link pairing, Keychain credential storage,
Mac approval and revocation, scoped Remote decoding and revision-safe booth
controls are implemented. The independently packaged, opt-in Mac Remote Gateway
receives only the path-free Live projection from the engine and cannot sit in
the Pro DJ Link, SoundSwitch or Ableton Link paths.

This is not yet a physical-iPhone beta: real Local Network permission,
pairing/reconnect, two-phone Controller transfer and combined show-soak evidence
remain mandatory. The app never substitutes demo show state.

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

A physical iPhone, Bonjour permission, pairing and lifecycle acceptance remain
mandatory before external beta distribution.

## Version and release

The independent product version is in `apps/ios/VERSION`. Tags use
`lumi-remote-vX.Y.Z`; that tag validates and creates a controlled draft GitHub
Release with an iOS Simulator artifact. It does not imply a signed physical
iPhone build. TestFlight remains a separate Apple signing gate.

Architecture and product details:

- `docs/design/iphone-remote/README.md`
- `docs/architecture/adr/0040-isolated-local-remote-gateway.md`
- `docs/architecture/adr/0041-independent-monorepo-product-releases.md`
- `docs/planning/epic-09-iphone-remote-beta.md`
