# Editor Theme acceptance — 2026-10-10

Local production-channel build 463, source `e4c543a`, installed through the native
installer after user approval. Public release assets were not replaced.
Pre-migration database backup retained locally (not committed).

## Automated checks

- Engine library: 167 passed, 4 ignored. Snapshot timing passed in a quiet serial
  run; an earlier parallel build/test run exceeded its 25 ms p95 threshold.
- SQLite: 61 passed, 1 ignored, including schema migration and stale-write guards.
- Swift workspace: 79 passed; Swift engine client: 32 passed with a real test
  engine and production stopped. Running MIDI tests with production open first
  caused endpoint conflicts; the isolated rerun passed.
- Library and Light Plans regressions passed; targeted persistence test proves
  saved Theme selection overrides automatic color eligibility and survives
  reopening, with a Theme-specific phrase choice in the compiled plan.
- Full macOS build and targeted Clippy checks passed.

## Real desktop UI

- Opened Toca's Miracle vs Retrograde in Track Editor.
- Selected user-requested BLUE PINK through Track Theme; save confirmed.
- BD START's Phrase AutoLoop picker contained only Automatic within BLUE PINK
  and button 10, BD START BLUE PINK.
- Selected button 10 and observed saved timeline revision 59.
- Quit and reopened Lumi, reopened the track: BLUE PINK and button 10 remained.
- Returned the temporary phrase choice to Automatic within BLUE PINK (revision
  60). Track Theme remains BLUE PINK. No phrase boundaries or roles were changed.

This is editor/persistence acceptance, not a fresh hardware light-output timing
test. The existing all-Theme coverage warning remains separate from the selected
Theme's available mapping; missing audio preview for this track was pre-existing.
