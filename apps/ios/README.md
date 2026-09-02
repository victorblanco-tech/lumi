# Lumi Remote for iPhone

Lumi Remote is the native booth companion for Lumi on macOS. It intentionally
contains only Live Players, compact integration health and revision-safe booth
controls. It does not contain Local Playback, Library, USB Sync or Track Edit.

## Current development status

The target and shared Live presentation compile for iOS 18. Bonjour discovery,
release-channel isolation, Keychain credential storage, scoped Remote protocol
decoding and safe command construction are implemented. The gateway remains
fail-closed until pinned TLS, Mac approval, persistent trust and the independent
engine projection feed are complete. Consequently this target is not yet a
usable physical-iPhone beta and never substitutes demo show state.

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
