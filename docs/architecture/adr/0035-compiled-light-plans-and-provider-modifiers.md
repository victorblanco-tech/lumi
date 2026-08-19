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

### 7. Revisioned persistence

Planning policy, candidate rules and modifier mappings are revisioned and updated
transactionally with optimistic concurrency. Existing plans retain their compiled
policy revision. Database migrations are additive and preserve the existing
Library, phrases and SoundSwitch mapping.

## Consequences

- Variation becomes visible and predictable without adding work to a phrase
  boundary.
- The current proven output lane remains unchanged: one optional bank selection
  and one AutoLoop selection per execution epoch.
- Color and repetition logic can evolve independently of SoundSwitch.
- Modifier mappings can be prepared now; their runtime execution requires a
  separate physical POC and explicit capability verification.
- A policy change affects newly compiled plans, not a currently executing show.
- Contextual help is part of the contract for non-obvious planning controls;
  terse labels do not require users to infer cooldown or color semantics.

## Sources

- SoundSwitch, [Static Looks Explained](https://support.soundswitch.com/en/support/solutions/articles/69000863339-soundswitch-static-looks-explained)
- SoundSwitch, [Software features](https://www.soundswitch.com/software.html)
- SoundSwitch, [Control One User Guide](https://cdn.inmusicbrands.com/soundswitch/files/User%20Guide%20Control%20One.pdf)
