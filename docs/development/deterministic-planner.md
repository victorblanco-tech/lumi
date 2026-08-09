# Deterministic next-track planner

`lumi-planner` converts normalized track analysis into a complete semantic
`LightingPlan`. Planning runs when a non-leader track is loaded, outside the
phrase-boundary execution path. The resulting effect re-enters the serialized
domain reducer and is stored before an ensuing leader change.

## Deterministic input

The default configuration has an explicit revision, four late-bound Themes,
and two scenes in each of five phrase-compatible categories. A stable seed is
derived only from canonical track ID and configuration revision. The choice
source is injected; wall-clock time and provider state never affect selection.

Track color crosses the source-adapter boundary as one normalized 24-bit sRGB
value (`0xRRGGBB`). Provider-specific labels and color IDs never enter the
planner. The selected Theme is recorded on the plan as decision evidence with
the chosen Theme identity, machine-readable reason, and matched normalized
color when a color rule applied.

Phrase kinds map to minimal semantic categories:

- Intro and Outro: Ambient
- Verse: Groove
- Build: Build
- Drop: Impact
- Breakdown: Break

Every analyzed phrase receives exactly one cue containing theme, scene, loop,
automatic origin, and a machine-readable `phraseCategoryMatched` reason. The
plan captures its configuration revision and starts at plan revision 1.

## Late-bound Theme precedence

Theme selection is deterministic and follows one explicit precedence chain:

1. global Theme Lock;
2. a user choice for the current plan instance;
3. a matching `FORCE` color rule;
4. a weighted matching `PREFER` color rule;
5. rotation/no-repeat using bounded recent Theme history;
6. the configured default Theme.

The planner generates the complete Next plan with the winning Theme. Changing
the Next Theme creates one new plan revision and re-themes every concrete cue,
including locked cues; cue locks protect creative cue edits, not plan-level
Theme selection. The underlying library timeline and loop strategies are not
mutated.

Activation atomically freezes the current plan revision. Later Next-preview
changes therefore cannot alter the already active output. A future explicit
live-Theme command must be a separate safe-boundary operation.

## Safe fallback

Missing, empty, partial, non-contiguous, or out-of-range phrase analysis never
panics and never produces a partial creative plan. The planner returns one cue
covering the track with `HoldCurrentLook`, fallback origin, and the explicit
`missingPhraseAnalysis` reason. The UI distinguishes this from a ready plan.

## Evidence

`fixtures/demo-session-v1/next-plan.json` is the reviewed canonical plan for
the simulated Next track; `next-plan-theme-override.json` is its reviewed
Ultraviolet plan-instance override. Tests cover byte determinism, complete phrase
coverage, Theme precedence, weighted preference, rotation/no-repeat,
plan-instance override, frozen activation, missing and partial analysis, and a
50 ms budget for a 200-phrase plan.

Run the focused evidence with:

```bash
cargo test -p lumi-planner
```
