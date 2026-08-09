# ADR-0026: Direct Pro DJ Link bridge and separate timing authority

- Status: **Accepted**
- Date: **2026-08-09**
- Supersedes: the production role of ADR-0018; the BLT MIDI adapter remains a
  temporary fallback and diagnostic reference

## Context

The BLT MIDI v4 proof of concept established that Lumi can consume two deck
transports, follow master changes and operate SoundSwitch beside a physical
Control One. Its seven-bit MIDI frames intentionally expose only a small
projection of Beat Link Trigger state. They cannot provide the complete media
identity, detailed waveform, beat grid and track signature needed for robust
matching.

Beat Link Trigger and the underlying `beat-link` Java library are separate
projects. Forking or embedding the BLT application would also bring its UI,
trigger model and application lifecycle into Lumi. Lumi only needs the
provider capabilities offered by `beat-link`.

SoundSwitch timing is a separate concern from Autoloop selection. MIDI notes
select banks and Autoloops; a continuous timing provider supplies effective
tempo, beat phase, bar phase and transport state.

## Decision

Lumi builds an application-owned `lumi-prolink-bridge` helper process. The
helper uses a pinned official `beat-link` release as a Maven dependency. Lumi
does not fork Beat Link Trigger and does not fork `beat-link` unless a required
upstream change cannot be implemented outside that library.

The helper is Java 21 and is supervised by `lumi-engine`. Production packages
bundle a minimal compatible Java runtime, so users do not install Java or
Maven. The helper reserves standard output for a versioned NDJSON protocol,
writes diagnostics to standard error and receives commands through standard
input. A process boundary isolates JVM, network and protocol failures from the
lighting execution hot path.

The bridge publishes source facts only:

- device discovery and loss;
- deck status, play/cue/on-air/master and freshness;
- original and effective BPM, beat number and beat within bar;
- playback position and transport discontinuities;
- Rekordbox media slot and track identity;
- metadata, artwork references, cue points, beat grid and waveforms;
- Beat Link `SignatureFinder` signatures when available.

The Rust adapter translates bridge messages into Lumi-owned deck observations.
No Beat Link, Java or Pioneer-specific type crosses the adapter boundary.

The engine remains timing authority. A separate `TimingOutputProvider`
receives the selected source timeline and publishes it to SoundSwitch using
Ableton Link. The existing MIDI Clock route remains a fallback. Pro DJ Link
input is never wired directly to Ableton Link output inside the source adapter;
this allows Local Playback to use the same SoundSwitch timing output.

Track reconciliation is deterministic and fail closed:

1. exact mounted-media identity plus Rekordbox track ID;
2. exact detailed-waveform/beat-grid track signature;
3. a persisted user-confirmed alias;
4. a unique scored metadata candidate;
5. otherwise `Unknown Track`, with automatic lighting held.

## Licensing and dependency policy

The checked-in `beat-link` repository license is EPL-2.0. Its Maven metadata
still mentions EPL-1.0; the exact source and binary license files of every
pinned release must therefore be captured and verified before a public Lumi
package is distributed. Required notices and corresponding-source locations
are included in Lumi's third-party notice and release artifacts.

Dependencies are consumed at an immutable version. Lumi maintains no permanent
vendor copy. If an upstream patch becomes necessary, VB Tech may create a
minimal fork, document the delta and offer the change upstream. Product code
must not depend on fork-only behavior without an explicit follow-up ADR.

## Operational safety

- Exactly one authoritative Pro DJ Link source process runs per Lumi engine.
- BLT and the direct bridge are never selected as simultaneous authoritative
  sources.
- Missing heartbeats and process termination degrade the source and close the
  automatic-output gate.
- A malformed or unsupported bridge message cannot mutate runtime state.
- UI rendering and metadata hydration never execute on the phrase-boundary
  lighting lane.
- The first release is read-only on the Pro DJ Link network; player remote
  control is outside scope.
- Development-simulator controls are a separate authenticated HTTP test surface
  defined by ADR-0027 and never target physical Pro DJ Link devices.

## Consequences

- Lumi no longer needs BLT expressions or MIDI framing in production.
- Rich loaded-track identity enables reliable USB/export-independent matching.
- The same normalized Live workspace and planner remain in use.
- The app package grows because it contains a minimal Java runtime and helper.
- Physical-player compatibility and timing still require hardware validation.
- The BLT adapter stays available until the direct bridge passes equivalent
  replay, simulator and physical-network acceptance tests.

## Rejected alternatives

### Fork and embed Beat Link Trigger

Rejected because Lumi would inherit an unrelated UI, trigger runtime and
configuration model instead of using the narrower library API.

### Copy Beat Link source into the Lumi repository

Rejected because it obscures provenance, upgrades and upstream contributions.

### Send only a BPM number to SoundSwitch

Rejected because Autoloops also require beat phase, bar alignment and transport
state for deterministic synchronization.
