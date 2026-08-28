# ADR-0035: Compiled Light Plans and provider-specific modifiers

Status: **Accepted**

Date: **2026-08-19**

## Context

Lumi can already resolve a Lumi Phrase Role through a Theme to one mapped
SoundSwitch AutoLoop and execute that selection exactly once at an authoritative
phrase boundary. The current automatic strategy intentionally selects the first
available variant. This is predictable, but makes consecutive tracks and repeated
Phrase Roles visually repetitive.

SoundSwitch also offers Static Looks and Color Overrides. A Static Look can
override only its included fixtures, while excluded fixtures continue the active
AutoLoop. Both features can therefore add atmosphere and variation without
replacing the provider-neutral AutoLoop model. Their MIDI lifecycle differs from
the already proven bank and AutoLoop selection, however, and must not be guessed
inside a show-critical output lane.

The stable integration architecture from ADR-0033 and ADR-0034 is a hard
constraint: planning policy and UI work may never enter the realtime Pro DJ Link,
Ableton Link or MIDI execution paths.

## Decision

### 1. Light Plans is a separate compile-time subsystem

Lumi introduces a provider-neutral `Light Planning Policy` and `Light Plan
Compiler`. The compiler runs when a track is loaded, before playback, and emits a
complete immutable plan instance. Realtime execution consumes only the compiled
discrete cue addresses.

The compiler never:

- reads the UI during playback;
- makes random choices at a phrase boundary;
- corrects or seeks a SoundSwitch AutoLoop timeline;
- sends Ableton Link state;
- joins or interprets Pro DJ Link traffic.

### 2. Four explicit layers

1. **Output catalog and mapping** — Theme/Bank, Phrase Role/Variant and the
   physical provider address.
2. **Planning policy** — candidate eligibility, Selection Weight, track-color
   affinity and repetition protection.
3. **Track template** — optional `AUTO`, `FIXED_VARIANT` or
   `THEME_SPECIFIC_EXACT` exception per phrase.
4. **Plan instance** — the concrete, reviewable selections for one loaded track,
   including live edits and evidence.

Themes remain late-bound. Track color is an input to selection and does not become
a fixed Theme in the Track Editor.

For the built-in SoundSwitch output profile, physical Banks 1–4 are explicitly
organized as Lumi Themes. The planner selects one base Theme for a complete track
before phrase-level AutoLoop compilation. Automatic phrase changes cannot cross
Themes. A deliberate plan-instance edit may still retheme a future phrase and all
following phrases.

### 3. Deterministic variation

`AUTO` considers mapped variants for the exact Phrase Role and selected Theme.
Every candidate has a **Selection Weight** (`Rare`, `Normal`, `Often`, `Primary`)
and optional track-color behavior:

- `Neutral`: color has no effect;
- `Prefer`: matching colors increase selection weight;
- `Only`: the candidate is eligible only for matching colors; matching `Only`
  candidates form the eligible set before weighted selection.

The weighted result is deterministic for a recorded variation seed. `New
variation` explicitly changes that seed. Reopening the same plan never silently
changes its choices.

Track overrides always take precedence. The compiler never falls back to a
different Phrase Role. If all repeat constraints exclude valid candidates, the
oldest repeat constraint may be relaxed and the evidence states this explicitly.

Rekordbox OneLibrary track colors are read from `content.color_id` and its fixed
named `color` catalog during trusted USB sync. Lumi persists their canonical RGB
projection on the track and exposes the distinct, Library-wide catalog to Light
Plans. Track metadata has provenance independent from analysis and hot cues:
`informationUpdateCount` orders updates from matching Rekordbox identities, with
export date as a conservative fallback. Consequently an information-only color
change can promote without reimporting analysis, while an older backup USB cannot
overwrite newer color metadata.

### 4. Repetition protection

The default policy is:

- Theme cooldown: 1 track;
- AutoLoop cooldown: 2 uses of the same Phrase Role;
- identical whole-plan signature: avoid within 4 tracks.

The history includes the current plan and the reserved next plan. Replacing an
unexecuted next plan releases its reservation; a plan is committed when execution
starts. History is bounded and can be cleared explicitly.

### 4.1 Theme Strategy

Each enabled Theme has Selection Weight and Track Color behavior:

- `Neutral` participates in normal weighted rotation;
- `Prefer` doubles effective weight for a matching Rekordbox color but remains a
  fallback for other colors;
- `Only` is eligible only for configured colors, and matching `Only` Themes take
  precedence over Neutral and Prefer candidates.

The fallback Theme is used when a plan has no prior Theme history and no color
rule takes precedence. Subsequent plans use deterministic weighted rotation after
excluding the configured number of committed and reserved recent Themes. Policy
evaluation remains outside all realtime lanes.

`Only` is a hard eligibility boundary, but Track Color itself is optional. With
no Track Color, non-`Only` Themes remain eligible. Lumi chooses among the
eligible Themes with the best exact phrase coverage; the opening Phrase Role
must be mapped so a track can start deterministically.

A later Phrase Role without a mapping does not invalidate the whole track plan.
That phrase compiles to a provider no-op: Lumi sends no AutoLoop selection and
the current SoundSwitch AutoLoop continues until the next mapped phrase. The UI
shows this as a held plan segment. Lumi still never violates an `Only` rule,
substitutes another Phrase Role, or crosses Themes automatically to hide a
mapping gap.

USB metadata inspection treats a changed metadata revision independently from
an unchanged kept-active analysis revision. This allows a Rekordbox Track Color
change to enter the normal review/sync path without replacing the beatgrid,
waveform, cues or Lumi-owned phrase timeline.

Stored policies from before Theme Strategy retain their exact former selection
behavior until the user saves the explicit strategy. Saving materializes one rule
per existing Bank without changing its ID, name, MIDI mapping or AutoLoops.

### 5. Explainability

Each compiled choice contains evidence: selected candidate, effective weight,
color influence, repeat filters, relaxed constraint if any, policy revision and
variation seed. The UI can explain every automatic choice without reproducing the
selection algorithm.

### 6. Provider-neutral modifiers

A compiled phrase may contain:

- one base AutoLoop;
- zero or one `Atmosphere Modifier`;
- zero or one `Color Modifier`.

The SoundSwitch output profile maps these generic modifiers to a Static Look or a
Color Override and to its MIDI address. Modifier policies use Application Rate,
Selection Weight, cooldown, scope, eligible Phrase Roles and track colors. `No
Override` is always a valid outcome.

Static Looks and Color Overrides are configuration- and preview-ready in this
epic, but automatic output is **fail-closed** until an integration proof has
established activation, release, toggle/exclusivity and reconnect semantics. An
unverified modifier can never emit MIDI from automatic execution. Manual Control
One operation remains parallel.

### 6.1 SoundSwitch Static Look capability proof

The physical SoundSwitch POC on 2026-08-22 established the concrete first-output
contract:

- SoundSwitch exposes one global Static Look surface with 32 slots, ordered as
  four columns of eight (`1–8`, `9–16`, `17–24`, `25–32`);
- a learned note pulse toggles the selected Static Look both on and off;
- at most one Static Look is active; selecting another replaces the current one;
- SoundSwitch exposes no selected-look feedback to Lumi.

The built-in SoundSwitch profile therefore reserves MIDI Channel 12, Notes
64–95 for Static Looks 1–32. This surface is configured under `Integrations →
Lighting Outputs → Static Looks`, next to the existing Banks & AutoLoops
surface. The UI supports guided Learn and a deliberate manual Toggle action.

Because output state is write-only, `activationVerified` and `releaseVerified`
remain separate user confirmations. Merely learning or testing a slot never
enables automatic execution. Planning rules stay in Light Plans; provider
addresses and names stay in Lighting Outputs.

### 6.2 Write-only Static Look runtime contract

After the physical proof, verified Atmosphere Modifiers may enter an immutable
compiled plan. The output lane keeps only the last Static Look that Lumi itself
successfully selected. At a cue it compares that assumed state with the compiled
desired state:

- `none → look`: one activation pulse;
- `look A → look B`: one replacement pulse for B;
- `look → none`: one release pulse for the active look;
- unchanged: no MIDI.

This is a sparse state-transition lane, not a timeline. Pause/resume, UI refresh,
repeated deck packets and equivalent phrase observations never cause a pulse. A
seek or hotcue only emits if the destination's compiled desired state differs.
`Off` and an explicit source stop attempt one release of a Lumi-managed look.

Because SoundSwitch exposes no feedback, Lumi never claims to know changes made
directly on Control One. It also never reasserts the same Static Look: doing so
could toggle it off. Automatic eligibility therefore remains fail-closed per
slot, and diagnostics call the value an `activeAssumption`.

Static Look execution shares the already isolated MIDI worker but never changes
the AutoLoop execution epoch/generation and never sends transport, beat, BPM or
Ableton Link corrections.

### 7. Revisioned persistence

Planning policy, candidate rules and modifier mappings are revisioned and updated
transactionally with optimistic concurrency. Existing plans retain their compiled
policy revision. Database migrations are additive and preserve the existing
Library, phrases and SoundSwitch mapping.

## Consequences

- Variation becomes visible and predictable without adding work to a phrase
  boundary.
- The current proven AutoLoop lane remains unchanged: one optional bank selection
  and one AutoLoop selection per execution epoch. A separate sparse modifier
  transition may add at most one Static Look pulse at a genuine state change.
- Color and repetition logic can evolve independently of SoundSwitch.
- Static Look execution requires per-slot activation and release verification;
  Color Override execution remains unavailable pending its own physical proof.
- A policy change affects newly compiled plans, not a currently executing show.
- Contextual help is part of the contract for non-obvious planning controls;
  terse labels do not require users to infer cooldown or color semantics.

## Sources

- SoundSwitch, [Static Looks Explained](https://support.soundswitch.com/en/support/solutions/articles/69000863339-soundswitch-static-looks-explained)
- SoundSwitch, [Software features](https://www.soundswitch.com/software.html)
- SoundSwitch, [Control One User Guide](https://cdn.inmusicbrands.com/soundswitch/files/User%20Guide%20Control%20One.pdf)
