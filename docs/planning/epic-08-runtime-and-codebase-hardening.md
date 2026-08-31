# Epic 8 – Runtime and codebase hardening

Status: **In progress** | Target: **0.5.2** | Priority: **P0/P1**

## Outcome

The public-beta feature set from 0.5.1 keeps identical user behavior while its
control plane, service lifecycle, USB identity, persistence concurrency and
development gates become bounded and predictable. The work creates a safer
base for the later iPhone companion without coupling that client to a large or
ambiguous desktop protocol.

## Architecture boundary

ADR-0039 is authoritative. No refactor may move SwiftUI, SQLite, USB or full
snapshot work into Pro DJ Link ingestion, Ableton Link publication or realtime
SoundSwitch MIDI execution.

## Delivery stories

### E8-01 – Bounded IPC and safe service ownership

- bounded connect, authentication and command deadlines;
- cancellation releases the serialized exchange lease;
- executable identity must match before a stale PID is signalled;
- regression coverage for a silent local peer and stale record handling.

### E8-02 – Authoritative USB identity

- physical identity wins over display labels;
- legacy identities have an explicit, tested migration path;
- secured bookmarks reconnect local fallback identities;
- the isolated worker validates the identity contract before mutation.

### E8-03 – Retired runtime paths

- direct Pro DJ Link is the only production Connected Deck provider;
- remove silent BLT runtime fallback;
- remove Rekordbox XML/direct-database product commands and hidden UI;
- keep only bounded test tooling where it still provides unique evidence.

Progress: direct Pro DJ Link is the sole Connected Deck provider and the
authenticated product command surfaces no longer expose XML or direct local
Rekordbox database ingestion. The unreachable macOS XML discovery and mirror
presentation has also been removed. Mounted OneLibrary USB media remains
supported.

### E8-04 – Data-lane isolation and maintainability

- explicit SQLite busy/durability policy;
- long data commands cannot occupy the integration pump;
- delete permanently disabled timing implementations;
- split the largest files only at existing ownership seams, protected by
  characterization tests.

Progress: USB work has one supervised out-of-process route and is rejected by
the realtime command protocol. SQLite contention and durability policy are
explicit, bounded and covered by a real two-connection lock-release test. The
permanently disabled predictive scheduler has been deleted, leaving one tested
exactly-once AutoLoop execution implementation.

### E8-05 – Quality and release evidence

- affected Engine Client changes run its package tests on `dev`;
- scheduled/manual dependency and licence audit without taxing every push;
- full Rust, Apple, security, functional and technical gates;
- headed macOS smoke test and clean 0.5.2 release artifact.

Progress: the fast native development gate now includes the Engine Client
package. Vulnerability and SPDX/license inventory checks run as a separate
weekly or manual Linux workflow and therefore do not consume macOS minutes or
delay every `dev` push.

## Acceptance

- no local IPC operation can wait indefinitely;
- Lumi never signals an unverified process;
- two independent equal-model USB sources cannot alias by label;
- a Pro DJ Link bridge failure is visible and never changes provider silently;
- all existing regression and performance gates pass without timing regression;
- 0.5.1 Production data opens unchanged in 0.5.2 and Dev data stays isolated.

## Accepted follow-up after the refactor

Live Deck presentation will adopt Pro DJ Link terminology without changing the
transport model: `Player 1`, `Player 2`, and so on, with the detected hardware
model (for example `CDJ-1500X`) shown beneath the stable player number. Missing
model metadata stays blank; Lumi never invents a device type.

Progress: implemented after the runtime cleanup. Player number remains the
stable layout/transport identity and the optional announced model is carried as
presentation-only snapshot metadata.
