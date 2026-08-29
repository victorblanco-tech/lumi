# E7-01 – Track workflow foundation and USB-change inbox

Status: **Done in 0.5.0-dev-34** | Priority: **P0** | Effort: **8**

## User value

As a DJ, I can prepare tracks through clear statuses and immediately find tracks
that need another check after a trusted USB sync, without losing my Lumi phrase
work or affecting a running show.

## Acceptance criteria

- Tracks default to `Not Started` and can be changed to `In Progress` or `Ready
  for Show` from the editor.
- Workflow state is separate from technical Library readiness.
- Status mutations use optimistic revisions and survive engine/app restarts.
- Trusted USB promotions create exact, durable attention reasons for changed
  metadata, waveform, beatgrid, hot cues and source phrases.
- A manually ready track with attention is not counted in Ready for Show until
  the review is explicitly cleared.
- The editor explains that Lumi phrases remain on authored beats and offers one
  explicit `Mark Reviewed` action.
- The Library offers paged fixed queues and displays status/attention in the
  track table.
- Empty libraries, migrations from schema 1/2/3/13/15 and stale UI revisions
  fail safely.
- Existing live integration and output regression transcripts remain unchanged.

## Verification

- SQLite migration, persistence, conflict, filtering and attention tests;
- USB promotion regression proving source-phrase attention;
- engine command decode/encode tests;
- Swift snapshot and workspace tests;
- full local functional and technical gates;
- headed Lumi test: switch to Workflow, change a status, verify queue movement,
  restart and verify persistence.

## Acceptance evidence

- The full local functional and technical gates pass on Apple Silicon.
- The 10,000-track workflow query and summary complete in approximately 11 ms
  in the release-profile regression fixture.
- The installed Dev DMG opens the real preserved Dev Library, renders the
  preparation status separately from readiness and exposes the Workflow
  browser, status menu and keyboard-accessible navigation.
- Engine-process and Swift interaction regressions cover status mutation,
  queue movement, stale-revision rejection and persistence after restart
  without changing the user's real preparation values during acceptance.
