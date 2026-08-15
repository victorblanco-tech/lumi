# Story E4-03A: Regression baseline and security floor

- Status: **Done for Dev — RC/lab evidence remains at epic level**
- Priority: **P0 Critical**
- Effort: **5**
- Components: Delivery, Engine, macOS, Pro DJ Link, Security
- GitHub tracking: [#100](https://github.com/victorblanco-tech/lumi/issues/100)

## User outcome

As the owner of Lumi, I can make and test changes locally without consuming
GitHub Actions minutes and can tell whether product behavior, timing,
robustness or dependency safety regressed before installing a Dev build.

## Scope

### A1 — Layered local gates

- provide explicit local entry points for functional, technical, complete,
  security and show/lab verification;
- document prerequisites, expected side effects and when hardware is required;
- keep normal functional and technical gates deterministic and offline after
  dependencies are cached;
- fail early with an actionable message when an installed Lumi instance owns
  the fixed CoreMIDI endpoints needed by an Apple integration test;
- keep expensive GitHub Actions manual/main-only during local-first development.

### A2 — Characterization fixtures

Capture accepted behavior before scheduler refactoring for:

- Off → Arm → Start → Pause → Start transitions;
- starting Lumi while a Master deck is already playing;
- stopped/cued deck → play and current-phrase reassertion;
- backward/forward seek and transport-generation changes;
- master handoff between two loaded planned decks;
- unknown and ambiguous tracks failing closed;
- navigation away from and back to Live without output loss;
- exact Bank/AutoLoop choice from saved user configuration.

### A3 — Technical baseline

Record a reproducible baseline for:

- input-to-CoreMIDI timing distribution (p50/p95/p99/max);
- scheduler lateness/starvation;
- received, applied, coalesced and dropped source observations;
- stale-generation cancellations and fallback-beat count;
- UI snapshot payload size and construction time;
- idle and active CPU, memory and queue high-water marks;
- 10,000-track library and 200-phrase planner budgets.

The baseline is test input, not proof of acceptance. Later phases must compare
against it and explain any regression.

### A4 — Security floor

- upgrade the directly pinned Jackson line to a non-vulnerable supported patch;
- add Maven dependency update coverage beside Cargo and GitHub Actions;
- add a local RustSec plus OSV audit command;
- define a documented, expiring exception format for an unavoidable advisory;
- inventory production helper boundaries and inherited credentials before D.

## Acceptance criteria

- `./scripts/verify-local.sh functional` passes with no hardware;
- `./scripts/verify-local.sh technical` passes with no hardware;
- `./scripts/verify-local.sh full` either passes or stops before testing with a
  precise instruction to close installed Lumi instances;
- `./scripts/verify-local.sh security` reports package, version, advisory and
  source, and exits non-zero for an unexcepted finding;
- `./scripts/verify-local.sh lab` never pretends to pass when simulator, synced
  database or helper prerequisites are absent;
- accepted Local Playback and Live Deck sequences above have deterministic
  automated coverage before E4-03B changes runtime scheduling;
- no secrets, user database, mounted-media content or copyrighted audio enter
  a test artifact.

## Verification

Implemented in `0.4.0-dev-34`:

- `functional`, `technical`, `full`, `security` and `lab` local gate entry
  points with a CoreMIDI ownership preflight;
- deterministic coverage for operational resume, already-playing Master,
  cued-to-play, seek generations, stale execution, overload and exact saved
  Bank/AutoLoop resolution;
- RustSec and Maven OSV scanning, Maven Dependabot coverage and Jackson 2.18.9;
- helper credential inheritance removed for the Pro DJ Link and Ableton Link
  child processes;
- retained Apple Silicon baseline from 2026-08-15:
  - 10,000 tracks: import 214.6 ms, paging 23.1 ms, search 6.7 ms;
  - full snapshot: p95 1.178 ms, 50,409 bytes;
  - lean Live snapshot: p95 0.221 ms, 20,799 bytes;
  - real Dev UI idle after accessibility traversal: 7.7% CPU, 51 MB resident;
  - engine runtime during the UI acceptance: one startup lateness of 60.1 ms
    over 28,925 ticks, zero fail-closed holds and zero provider failures.

Completed in `0.4.0-dev-35`:

- functional, technical, security and Apple application gates pass locally;
- the real UI was exercised through Integrations, Live operation state and an
  engine-owned backup using accessibility automation;
- concurrent monitor and interactive IPC exchanges have a real-process framing
  regression test;
- the packaged artifact contains an SPDX 2.3 SBOM.

The one-hour soak and physical-show evidence remain release gates owned by the
epic, not missing Dev implementation in this story.
