# Lumi Design System

`LumiDesignSystem` is the single owner of Lumi's native presentation tokens,
reusable controls, component states, global appearance preference, and musical
key formatting. It is a local Swift package with no external UI or state
management dependency.

## Rules

- Feature views use semantic `LumiColor`, `LumiTypography`, `LumiSpacing`,
  `LumiRadius`, and `LumiControlMetric` roles instead of defining visual values.
- Status uses a label and an SF Symbol in addition to semantic color.
- User-facing labels are English String Catalog keys; dynamic track, provider,
  and protocol values remain verbatim data.
- Canonical musical data is pitch class plus mode. Only
  `KeyNotationFormatter` creates Camelot or Classic display strings.
- `LumiPreferences` owns persisted appearance and key-notation choices. First
  launch defaults to Dark and Camelot; Light and System remain supported.
- The macOS app applies appearance through `NSApplication.appearance`. Setting
  System clears the override so AppKit updates background and foreground
  semantics together and continues following the macOS setting.
- The application canvas uses a semantic near-black/dark-gray palette and a
  Lumi-owned cyan interaction accent rather than the user's mutable macOS
  accent color. Semantic colors remain adaptive so Light and System appearances
  continue to work outside fixed-dark media surfaces.
- Waveform/media editing surfaces may force Dark for stable RGB contrast, but
  must still consume `LumiColor` tokens instead of defining a parallel palette.
- Components use native controls, keyboard semantics, accessibility labels, and
  San Francisco system typography.

The initial component set is `DeckCard`, `StatusBadge`, `PhraseRow`,
`InspectorField`, `ProviderStatus`, and `OperationControl`. Loading, empty,
ready, stale, degraded, and error are the shared component states.

## Verification

Run package tests directly with:

```bash
swift test --package-path apps/macos/Packages/LumiDesignSystem
```

The tests cover all 24 major/minor Camelot mappings, Classic formatting,
presentation-only notation changes, first-launch defaults, and persistence.
`./scripts/verify.sh` also runs these tests and builds the integrated app.

The package contains dark and light `#Preview` component galleries. The
development app exposes the same components with clearly marked sample data so
appearance, notation, selection, state, and accessibility behavior can be
reviewed without DJ hardware.
