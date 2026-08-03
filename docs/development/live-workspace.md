# Native Live workspace

E1-08 presents the authoritative engine snapshot as the native macOS `Live`
workspace. SwiftUI does not select the lighting leader, invent a next deck, or
create plan decisions.

## Boundaries

`LumiLiveWorkspace` is split by responsibility:

- `Mapping` validates protocol DTOs and produces immutable engine snapshots;
- `Presentation` maps lifecycle and provider health to explicit UI states;
- `Views` renders only the supplied `Live`, `Next`, and next-plan presentation;
- `Previews` contains deterministic Ready, Loading, Fallback, Stale,
  Disconnected, and Degraded fixtures;
- `LumiVisualEvidence` renders those fixtures without opening an app window.

The macOS app owns process supervision and supplies the resulting
`LiveWorkspaceState`. Key notation remains a presentation preference; canonical
key data is not mutated.

## Headless visual evidence

Generate the review matrix with:

```bash
./scripts/render-visual-evidence.sh
```

The command writes six 1280×960 PNGs to the ignored
`build/VisualEvidence/` directory. It uses fixed content, dimensions, locale,
appearance, and key notation. Rendering therefore works while the login session
is locked and does not require an active app window.

The full repository verification also renders the matrix and fails unless all
six non-empty PNGs are produced. Visual review remains necessary for layout,
contrast, truncation, and semantic state use.

## Verification coverage

Presentation tests read the canonical recorded snapshot contract and prove:

- the engine leader becomes `Live` and the other loaded deck becomes `Next`;
- the next plan belongs to the presented Next track-load instance;
- mismatched plan/deck snapshots fail at the mapping boundary;
- fallback and stale states retain their authoritative content;
- disconnected state never fabricates deck or plan data.

All controls and primary workspace regions expose stable accessibility
identifiers. App content remains scrollable at supported window sizes; the
headless evidence renderer uses an equivalent static layout because offscreen
`ScrollView` rendering requires a live window server.
