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
| [E10-07](story-e10-07-stable-remote-controller.md) | Complete; automated and native UI verified | Durable single-Controller ownership; two-client reconnect/transfer regression tests; separate connection/role presentation and client versions. |

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
- Editor screenshots always have the left main navigation collapsed to its
  icon rail. Keep the enlarged waveform and a useful phrase boundary visible;
  use the owner's prepared 90s Bitch track, not an unprepared placeholder.
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
  authorized by the owner. Codex Computer Use screen-recording access is enabled;
  a negative preflight from a separate Swift subprocess did not establish a
  missing Computer Use permission. Native Lumi selection/capture currently
  times out and headed acceptance is still pending.

### 2026-09-05 — atomic Live phrase/plan mutation increment

- Live Phrase Type edits now prepare the timeline and replacement Library
  context without a database write. AutoLoop overrides are likewise staged,
  never written into the active context before validation.
- Plan materialization is side-effect-free. Variation reservations, compiled
  modifiers and output plans are published only after the runtime has accepted
  the candidate plan and SQLite has committed the optimistic timeline revision.
- Runtime reduction happens against a bounded state copy; a duplicate/stale
  effect is a rejection, not an acknowledgment of a change that never applied.
  Only the edited track's context is prepared; other Players' waveform contexts
  are not copied. Rendering and the tempo/MIDI algorithms are unchanged.
- Regressions inject compiler failure, effect-sequence overflow, reducer
  rejection and a real SQLite trigger failure during phrase insertion. They
  assert no partial revision, changed context, variation history, compiled plan,
  output modifier or MIDI generation. Each scenario then successfully retries
  the same expected plan revision. Separate regressions cover a no-change
  AutoLoop selection and protection/stale-head checks at durable commit.
- This is code-level verification through the shared Mac/Remote command path,
  not headed acceptance. It does not complete asynchronous SQL isolation or
  post-restore worker hydration. Installed Lumi has not been replaced while
  exclusive-stop approval and native UI access remain unresolved.
- Verification: 408 Rust tests passed (14 broad-run tests intentionally ignored).
  The explicitly run 200-edit release benchmark measured p50 381 µs, p95 445 µs,
  p99 515 µs and max 662 µs on the small in-memory fixture. This does not claim
  realtime latency under concurrent Library work or physical lighting acceptance.

### Remaining before this epic can be called complete

The dev-5 follow-up installed and exercised the Mac app, completed all 25
exclusive engine-client tests and fixed two false-positive acceptance gaps:
the combined soak now requires real CoreMIDI dispatch and non-empty latency
samples, and iOS Simulator builds now retain their own Keychain entitlements.
The stricter 120-second soak completed 28 AutoLoops/29 pulses with 68 µs p95
local dispatch lateness. Native Remote Observer disconnect/reconnect recovered
without affecting the running Mac show. See the dated
[progress report](../release/0.6.2-audit-progress.md) for bounds and limitations.

1. Move synchronous SQL, preparation and expensive presentation work out of
   the show pump, with revision-bound staging and real contention tests.
2. Preserve proven USB provenance in live lookup; the current call still drops
   source context. Settle missing-provenance behavior before changing matching.
3. Complete atomic post-restore worker hydration. Live phrase/plan edits now
   prepare all fallible state work before the final transactional SQL append;
   asynchronous preparation will also need revision revalidation before publish.
4. Enforce Controller revocation at the final engine execution boundary, not
   only before the gateway sends. Test saturated production paths rather than
   treating the separate ProjectionHub tests as production evidence.
5. Complete headed Controller mutation and physical-iPhone recovery checks.
   Native iPhone Simulator Observer recovery now passes. The owner authorized
   stopping Lumi Dev and its Dev services, and the exclusive native CoreMIDI
   rerun passed all 25 engine-client tests (205 Swift tests in the full Apple
   gate). The earlier concurrent failure is not counted as acceptance evidence.
6. Investigate the slow Library accessibility traversal seen during headed
   testing (high UI CPU, engine still running). A sample contains substantial
   SwiftUI/AppKit accessibility and layout work; causality is not yet established.
7. Finish original-resolution screenshots and content-aware visual evidence.
   Existing generated/placeholder images are not evidence of real UI acceptance.
