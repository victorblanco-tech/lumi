# Deterministic next-track planner

`lumi-planner` converts normalized track analysis into a complete semantic
`LightingPlan`. Planning runs when a non-leader track is loaded, outside the
phrase-boundary execution path. The resulting effect re-enters the serialized
domain reducer and is stored before an ensuing leader change.

## Deterministic input

The default Epic 1 configuration has an explicit revision, two themes, and two
scenes in each of five phrase-compatible categories. A stable seed is derived
only from canonical track ID and configuration revision. The choice source is
injected; wall-clock time and provider state never affect selection.

Phrase kinds map to minimal semantic categories:

- Intro and Outro: Ambient
- Verse: Groove
- Build: Build
- Drop: Impact
- Breakdown: Break

Every analyzed phrase receives exactly one cue containing theme, scene, loop,
automatic origin, and a machine-readable `phraseCategoryMatched` reason. The
plan captures its configuration revision and starts at plan revision 1.

## Safe fallback

Missing, empty, partial, non-contiguous, or out-of-range phrase analysis never
panics and never produces a partial creative plan. The planner returns one cue
covering the track with `HoldCurrentLook`, fallback origin, and the explicit
`missingPhraseAnalysis` reason. The UI distinguishes this from a ready plan.

## Evidence

`fixtures/demo-session-v1/next-plan.json` is the reviewed canonical plan for
the simulated Next track. Tests cover byte determinism, complete phrase
coverage, injected choices, missing and partial analysis, and a 50 ms Epic 1
budget for a 200-phrase plan.

Run the focused evidence with:

```bash
cargo test -p lumi-planner
```
