# Fixtures

This directory owns deterministic, license-safe input and expected-output data
used across engine, client, and end-to-end tests.

Demo deck events and planning scenarios are introduced with their consuming
Epic 1 stories. Audio files and exported proprietary library data do not belong
in this repository.

`demo-library-v1/library.json` describes three fully synthetic tracks and two
playlists for the library workflow. Waveform points, beat markers, and bounded
PCM audio segments are generated deterministically from the fixture;
`lumi-demo://` references never resolve to copied music files. The demo adapter
can also generate up to 10,000 synthetic tracks for local scale and pagination
tests without copyrighted audio or proprietary database content.

`demo-session-v1/canonical-e2e.json` is the compact release golden: it records
the edited and locked plan cue, pause-safe 64× execution, semantic output order,
and bounded timeline counts for the complete Epic 1 scenario.

`epic-2a-v1/library-editor-e2e.json` is the Epic 2A library golden. It proves a
demo track can be browsed, edited on whole bars, assigned a Settings-owned
stable role and fixed logical variant, refreshed to a new source revision,
reopened after a worker restart, and resolved through all four Theme targets.
`demo-library-v1/simulator-e2e.json` continues that exact Lumi-owned track into
Next, Active and exactly-once dry-run output.
