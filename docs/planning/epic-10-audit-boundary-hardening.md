# Epic 10 — Verified show and data boundaries

Status: **In progress** | Target: **0.6.2 / Remote 0.1.1**

The follow-up audit of `8513ca6` found gaps between the isolation promised in
ADR-0040 and actual runtime behavior. ADR-0042 governs this correction. Existing
waveform rendering, the tempo-only Link relay and user-created Library/MIDI data
must remain unchanged unless a specific regression proves a change necessary.

## Stories and order

| Story | Status | Scope and required evidence |
| --- | --- | --- |
| E10-01 | Implemented; regression-tested | Non-fatal Remote projections; deterministic Live/Next selection with extra Players; bounded cancellation-safe stream reads; fragmented/oversized frame and recovery regressions. |
| E10-02 | In progress | Desktop authentication/writes independent from show pumping; prepared data/plan results outside the pump; process tests with slow clients and SQLite contention, with measured input-to-output timing. |
| E10-03 | In progress | Preserve independent USB identities; source-aware live matching; supported older-backup migration before activation; staged phrase/plan mutations with failure-injection and no user data cleanup. |
| E10-04 | In progress | Effective authentication/command budgets, outcome-preserving idempotency, bounded command lifetime and control revocation; test actual production TLS/IPC paths. |
| E10-05 | In progress | Remote freshness and reconnect state; persistent integration status and disabled stale controls; native client tests and headed macOS/iOS acceptance. |
| E10-06 | In progress | Correct benchmark/app-build/visual gates; simulator user guide and real HQ screenshots; final regression, performance, security and UI evidence. |

## Delivery rules

- Each story records its actual tests and remaining limitations below. A green
  test that reproduces a bug is not completion; regressions assert the desired
  behavior after the fix.
- Commit verified increments to `dev`; use a new numbered development version
  for an installable handoff. Do not publish a Production release implicitly.
- No resetting, re-importing or consolidating the owner's Library to make a
  test pass. Restore/USB tests use temporary fixtures and copies.
- Do not claim hardware acceptance from simulated traffic or screenshots.
- Test real native UI for user-visible changes. Screenshots are original
  resolution captures, visually inspected, with sensible display sizing and no
  fabricated waveform/track content.
- Ask before a product choice would change supported workflows. Implementation
  choices that preserve existing behavior can proceed independently.

## Evidence

Initial state: second audit reproduced six defects using temporary tests against
the production crates. Those findings are being converted to in-repository
regressions. No story is complete merely because its documentation exists.

### 2026-09-05 — first verified implementation increment

- **Show boundary:** Remote projection errors no longer escape into the engine
  event loop. Live/Next selection accepts more than two detected Players and
  display metadata is normalized without rewriting track IDs.
- **Framing:** the shared `lumi-stream` bounded reader preserves partially read
  bytes across cancellation. Production desktop, gateway, TLS, admin and engine
  Remote readers use it. Tests cover fragmentation, unterminated oversized
  frames and recovery through the real gateway projection client.
- **Desktop I/O:** authentication and socket writes have separate owners,
  deadlines and bounded queues. The actual process test held a partial
  authentication open for 300 ms while 61 show-pump ticks continued; maximum
  measured pump lateness was 803 µs in that run. This is not a measurement of
  CDJ-to-MIDI latency under database contention.
- **USB/restore:** identical labels and tracksets no longer merge separate USB
  source identities. Historical schema-13 backups migrate and validate in a
  staging database before activation. A malformed historical backup leaves the
  active database unchanged. Worker-level post-restore hydration is still open.
- **Remote commands:** authentication work is checked against bounded rate
  budgets; mutating command IDs are scoped to device and Controller lease.
  The production relay retains actual engine results, including conflicts,
  without needing a connected phone. Pending/uncertain submissions never earn
  a success acknowledgment and are not automatically resubmitted. In-flight
  ledger entries cannot be evicted to make room for new work. Tests cover
  repeated results, reused IDs with different payloads, lease changes, bounds
  and the real relay socket path. Queued gateway work is checked again before
  sending; engine requests check a monotonic deadline and abandoned receiver.
- **iPhone freshness:** stale/unavailable/reconnecting states disable controls
  until an authoritative projection arrives, retaining the lease for recovery.
  Legacy Duplicate responses and uncertain outcomes require a fresh snapshot.
  The 28 client regressions pass; new physical-iPhone acceptance is not claimed.
- **Gates:** the 10k benchmark now actually runs its ignored test. The fast Apple
  gate includes the Mac app build and 18 safe client-contract/safety tests;
  exclusive CoreMIDI process tests remain a separate requirement.
- **Documentation:** the simulator user guide is linked from the README, site
  and main guide. New images must show 90s Bitch on Player 1 and My Favourite
  Regrets on Player 2, preserving existing Lumi edits. Native HQ capture is
  authorized by the owner but macOS screen-recording permission is pending.

### Remaining before this epic can be called complete

1. Move synchronous SQL, preparation and expensive presentation work out of
   the show pump, with revision-bound staging and real contention tests.
2. Preserve proven USB provenance in live lookup; the current call still drops
   source context. Settle missing-provenance behavior before changing matching.
3. Make complete phrase/plan mutations and post-restore worker hydration atomic,
   including failure injection after SQL succeeds.
4. Enforce Controller revocation at the final engine execution boundary, not
   only before the gateway sends. Test saturated production paths rather than
   treating the separate ProjectionHub tests as production evidence.
5. Complete exclusive native CoreMIDI tests and headed iPhone recovery checks.
   The first Swift process run failed its MIDI checks with Lumi running, so it
   is not acceptance evidence. Permission was requested to stop the app/service
   temporarily; no protected stop was bypassed.
6. Investigate the slow Library accessibility traversal seen during headed
   testing (high UI CPU, engine still running). A sample contains substantial
   SwiftUI/AppKit accessibility and layout work; causality is not yet established.
7. Finish original-resolution screenshots and content-aware visual evidence.
   Existing generated/placeholder images are not evidence of real UI acceptance.
