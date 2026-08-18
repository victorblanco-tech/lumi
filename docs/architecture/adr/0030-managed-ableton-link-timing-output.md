# ADR-0030: Managed Ableton Link timing output

- Status: **Accepted**
- Date: **2026-08-11**
- Refines: ADR-0015, ADR-0022 and ADR-0026
- Execution refinement: ADR-0033 replaces the predictive AutoLoop coordination
  rules below with an exactly-once cue executor. The managed Link provider and
  parallel SoundSwitch path decisions remain accepted.

## Context

Lumi already receives read-only deck, master, effective-tempo and beat facts
directly from Pro DJ Link through the supervised `lumi-prolink-bridge`. Local
Playback provides the same normalized transport facts from Lumi's own player.
SoundSwitch needs a continuous tempo, beat and four-beat bar timeline in
parallel with the sparse MIDI commands which select a Bank and AutoLoop.

Beat Link Trigger proved this end-to-end workflow by connecting its Pro DJ Link
state to Carabiner and publishing an Ableton Link session. Keeping BLT in the
production chain would duplicate deck discovery, master selection, lifecycle
and configuration already owned by Lumi. Implementing the Ableton Link network
protocol independently would create unnecessary interoperability and timing
risk.

The cross-platform Ableton Link library is dual-licensed under GPL-2.0-or-later
and a proprietary license. Carabiner is a small GPL-2.0-or-later executable
which embeds that library and exposes a documented loopback-only protocol.
Lumi is distributed under EPL-2.0.

## Decision

Lumi owns a provider-neutral `TimingOutputProvider`. The first production
adapter is `AbletonLinkTimingOutput` and communicates with a pinned, separately
executed Carabiner helper over its loopback TCP protocol. The Lumi application
supervises, starts and stops the helper; the user does not install or configure
Carabiner and no Beat Link Trigger process or expression is required.

Carabiner remains a separate program with its own license, notices and
corresponding-source location. Its types and protocol do not cross the timing
adapter boundary. A later Lumi-owned native Link helper can replace it without
changing engine, deck-source, planning or UI contracts.

The Rust engine is the single timing authority. It selects exactly one source
timeline and publishes immutable timing anchors containing:

- source and stable deck identity;
- monotonically observed time;
- effective BPM, including deck pitch;
- beat-within-bar and transport generation;
- playing state and discontinuity generation;
- freshness and confidence.

The adapter behaves as a passive follower of Lumi's selected master. It never
appoints Ableton Link as Pro DJ Link tempo master and never sends control
commands to physical players. Local Playback and Pro DJ Link feed the same
output port.

## Parallel SoundSwitch paths

SoundSwitch receives three independent inputs:

1. Ableton Link timing from Lumi: BPM, beat, bar and transport;
2. Lumi Virtual MIDI: Bank and AutoLoop commands on planned phrase boundaries;
3. optional Control One input: direct user overrides.

SoundSwitch remains the lighting engine. Its DMX output may use Control One or
another interface. That downstream interface is not part of Lumi's model.

```text
Pro DJ Link / Local Playback -> Lumi Timing Authority -> Ableton Link --+
                                                                      |
Lumi Lighting Plan ----------> Lumi Virtual MIDI ---------------------+-> SoundSwitch
                                                                      |
Control One manual input ---------------------------------------------+

SoundSwitch -> selected DMX interface (optionally Control One) -> fixtures
```

The two Lumi outputs have different responsibilities and priority. Ableton
Link maintains SoundSwitch tempo and phase after an AutoLoop has been selected.
`Lumi Virtual MIDI` selects the correct Bank and AutoLoop at the musical
deadline. The latter is the release-critical realtime path: a correct BPM with
a late or wrong AutoLoop selection is not considered successful output.

## Realtime AutoLoop execution

Normal phrase playback and discontinuous navigation use one engine-owned,
provider-neutral musical timeline. Local Playback supplies an audio-clock
anchor; Pro DJ Link supplies exact beat packets plus absolute deck-status
position. The local Rekordbox beat grid and Lumi phrases translate those
transport facts into the active and next lighting cue.

- The engine schedules the next cue's Bank close to a known phrase boundary:
  the SoundSwitch settle interval plus one engine-tick safety margin. It then
  emits the AutoLoop note on the exact boundary observation. This short window
  lets Lumi reassert its choice after ordinary manual control.
- Bank settling and AutoLoop emission are separate non-blocking stages. No
  fixed sleep may block deck ingestion, Link publication or another transport
  generation.
- Every scheduled stage carries deck, track-load, plan revision, phrase and
  transport-generation identity. A load, seek, hotcue, beatjump or master
  change invalidates older work before it can reach CoreMIDI.
- A hotcue or beatjump resolves the landing beat against the exact local beat
  grid and immediately selects the landing phrase's planned AutoLoop.
- If its Bank is already safely armed, the landing AutoLoop is emitted on that
  beat. If the Bank cannot satisfy its minimum settling time, Lumi arms it and
  emits the AutoLoop on the first following exact beat. A deterministic
  one-beat fallback is preferred over starting a SoundSwitch loop off-beat.
- Unknown, ambiguous or non-library tracks remain visible but never infer a
  realtime AutoLoop target.
- Lighting Output Offset applies to the sparse AutoLoop deadline only. It does
  not change the deck timeline or Ableton Link phase.

The visual waveform, phrase band and AutoLoop plan may consume the same
timeline through a replaceable display clock, but rendering is never an input
to cue execution. UI frame drops therefore cannot delay or duplicate output.

## Timing behavior

- The engine pumps direct deck input on a dedicated 20 ms cadence. SwiftUI's
  snapshot polling, waveform rendering and database work never establish or
  advance timing.
- Only Pro DJ Link beat packets are treated as beat-exact observations while a
  deck is playing. Asynchronous status packets may update metadata and stopped
  BPM/transport state, but never impersonate a beat boundary.
- Effective BPM changes are applied immediately while preserving phase.
- Normal packet jitter is filtered; Lumi does not force a new Link mapping on
  every beat.
- Play, cue, seek, track replacement and master handoff increment a transport
  generation and establish one new anchor on the first reliable beat.
- A soft phase-error threshold permits smooth correction. A hard threshold or
  explicit discontinuity performs one deterministic re-anchor.
- Four-beat bar phase is derived from the source beat grid. A missing master or
  timing silence of three beats (bounded to 1.25–5 seconds) holds Link
  transport fail-closed and degrades timing readiness.
- Start/stop synchronization mirrors the selected Lumi source; Link remains a
  timing transport and never becomes the operation-state authority.
- Lighting Output Offset advances or delays only the sparse MIDI command at a
  safe phrase boundary. It does not falsify the Link timeline.

The timing worker and helper are isolated from SwiftUI, waveform rendering,
SQLite and library imports. The adapter retains at most the newest unprocessed
anchor and coalesces older continuous observations. A transport generation is
never replaced by an older observation.

The Pro DJ Link bridge is supervised as an independent fault domain. Decode,
pipe or process failure clears the stale deck authority, holds Link transport
and schedules a bounded automatic restart. The first fresh observation after
source or helper recovery re-establishes the same user-enabled Link session;
no app restart or manual toggle is required.

## Lifecycle and diagnostics

The helper starts only after explicit Link enablement or a saved auto-start
preference and is supervised independently from `Lumi Virtual MIDI`. Merely
launching Lumi does not join Link with the safe default. While enabled, a valid
timing anchor supplies tempo and phase; `Off` holds transport without silently
discarding the user's Link choice. Failure of one output does not remove the
other.
Readiness reports at least:

- helper version and process health;
- Link enabled state and peer count;
- selected timing source and master deck;
- current effective BPM, beat and bar phase;
- last beat age, phase error and last re-anchor reason;
- transport state and last actionable error.

Bounded session diagnostics also report received, applied and coalesced timing
anchors; hard re-anchors and soft corrections; maximum observed phase error;
fail-closed and provider-failure counts; and engine-pump ticks, starvation and
maximum lateness. These counters make regressions visible without logging in
the realtime path or allowing diagnostics to grow without bound.

Diagnostics offers an explicit helper self-test while Lumi is `Off`. It runs
the pinned executable's terminating version mode and verifies the exact
expected version without opening a server or joining the Link session. This
keeps the test side-effect free for SoundSwitch; runtime connection recovery
still occurs automatically while Link is enabled and the next valid timing
anchor arrives. It is rejected while Link is enabled and during `Arm`, `Start`
or `Pause`, so recovery cannot interrupt a show in progress.

Ableton Link has an explicit user-owned lifecycle and is Off by default. The
user may enable it from Integrations or Live and may persist an app-start
preference. Disabling it leaves the Link session immediately; changing the
lighting operation state never silently changes that saved integration choice.

Within an enabled Link session, `Off` and `Pause` hold authoritative transport
while the selected source's effective BPM, beat and bar phase remain current.
This lets peers follow the musical authority without opening Lumi's lighting
gate. `Arm` locks and validates timing without sending lighting commands.
`Start` publishes running transport and executes planned MIDI cues. Resumption
re-anchors safely before a new automatic cue.

Live status exposes only Pro DJ Link, Light Output and Ableton Link. An
intentionally disabled provider, Local Playback not using Pro DJ Link, or an
empty deck is informational rather than degraded. Diagnostics retains helper
and transport detail for troubleshooting.

## Acceptance gates

1. Local Playback drives a complete SoundSwitch AutoLoop show without BLT.
2. The USB-backed network simulator proves pitch changes, pause/cue/resume,
   seeks and master changes.
3. A one-hour soak has no cumulative phase drift or timing-worker starvation.
4. Measured phrase-boundary output targets a p95 end-to-end error of at most
   20 ms; the measured result is retained with the release evidence.
5. Control One remains usable in parallel and its selected AutoLoop feedback
   follows Lumi commands.
6. Physical CDJ-1500X, DJM-V5, SoundSwitch, Control One and DMX acceptance
   succeeds before the BLT fallback is removed.
7. Forward and backward hotcue/beatjump landings cancel stale output and select
   the landing phrase's exact configured AutoLoop. An unarmed Bank falls back
   to the next beat rather than emitting off-beat.
8. Realtime AutoLoop evidence records scheduled, actual and delta timestamps,
   stale-generation cancellations and fallback-beat count. Normal pre-armed
   phrase changes target p95 <= 20 ms at the CoreMIDI boundary.

The managed helper gate was first exercised against SoundSwitch as a real Link
peer on 2026-08-11: peer discovery, 130 → 140 BPM, phase synchronization,
start/stop and hold completed without BLT. This is evidence for gates 1–2, but
does not replace the complete-song, one-hour or physical-DMX gates.

The `0.4.0-dev-21` robustness slice added deterministic status-versus-beat
tests, stale-source fail-closed/recovery tests, helper race recovery and
engine-cadence starvation metrics. Its opt-in LAN acceptance test also proved
that bridge frames and Link anchors advance through three seconds with no
client or UI polling. The one-hour and physical-DMX gates remain release
evidence rather than being inferred from this bounded test.

## Implementation correction — `0.4.0-dev-42`

Deck-status packets and precise Beat packets are independent asynchronous
streams. A playing status position is therefore compared with the expected
progress derived from elapsed monotonic time and effective BPM. Ordinary
multi-beat progress is not a seek merely because more than two Beat packets
were delayed or coalesced. A late status packet also cannot rewind the
canonical position established by a newer precise Beat packet. Only a real
out-of-envelope seek, Hot Cue, beatjump or loop wrap advances the transport
generation and permits a new Link anchor.

The managed foreground helper is part of the app-owned process tree. Runtime
drop terminates and waits for that exact child; normal macOS Quit and an
unexpected UI disconnect both tear down the engine first. A real-helper
regression verifies that the control endpoint closes without a separate Stop
command. This prevents SoundSwitch from retaining a ghost Link peer after Lumi
has exited.

## Implementation correction — `0.4.0-dev-43`

The UI window and the channel engine no longer share one ordinary-Quit
lifetime. Every authenticated client disconnect first transitions the engine
to Off, stops local clock publication and leaves Ableton Link. The engine then
continues only as an inactive, reconnectable owner of stable CoreMIDI
endpoints. Link helper command and child-process waits are bounded; a failed
cleanup is retained as diagnostic state and cannot tear down those MIDI
endpoints as a secondary effect.

This resolves two lifecycle hazards at their correct boundaries: Carabiner
cannot survive as a ghost Link peer, while SoundSwitch does not see virtual
MIDI devices removed and recreated for an ordinary Lumi UI session. The final
signed `SMAppService` login/crash lifecycle remains ADR-0003 work.

## Licensing and distribution

- `beat-link` remains a pinned EPL-2.0 dependency of the independent Pro DJ
  Link input bridge.
- Beat Link Trigger is not copied, linked or bundled.
- The pinned Carabiner executable and Ableton Link sources are inventoried with
  their GPL notices and corresponding-source location.
- Lumi does not claim AlphaTheta or Ableton certification or endorsement.
- A future closed-source or proprietary Lumi distribution requires a separate
  Ableton Link licensing decision and legal review.

## Consequences

- BLT can be removed after physical acceptance without losing Link timing.
- Local Playback and physical decks exercise one timing-output implementation.
- The GPL component remains explicit and replaceable instead of being linked
  into the EPL Lumi executable.
- The app bundle grows by one small native helper and release packaging gains a
  source/notices obligation.

## Superseding isolation correction — `0.4.0-dev-48`

ADR-0032 supersedes the parts of this ADR that placed show transport
generation, phrase/Hot Cue discontinuity and lighting operation state in the
Link input. The Link adapter now accepts a clock-only observation and cannot
receive those show-control facts. AutoLoop MIDI and Ableton Link remain
parallel SoundSwitch inputs.

## Rejected alternatives

### Keep Beat Link Trigger as a runtime dependency

Rejected because it duplicates Lumi's source, state, UI and lifecycle and
requires user-managed expressions.

### Reimplement the Ableton Link network protocol

Rejected because the official implementation already solves discovery,
consensus and timeline synchronization and independent compatibility would be
hard to prove.

### Link the GPL Ableton library directly into the Lumi executable

Deferred because it creates a less clear distribution boundary. A native
helper remains possible if its licensing and packaging are handled explicitly.

### Use MIDI Clock as the only timing output

Rejected as the primary route because it has weaker phase/bar semantics and
jitter behavior. `Lumi Clock` remains a separately diagnosed fallback.
