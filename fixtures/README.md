# Fixtures

This directory owns deterministic, license-safe input and expected-output data
used across engine, client, and end-to-end tests.

Demo deck events and planning scenarios are introduced with their consuming
Epic 1 stories. Audio files and exported proprietary library data do not belong
in this repository.

`demo-session-v1/canonical-e2e.json` is the compact release golden: it records
the edited and locked plan cue, pause-safe 64× execution, semantic output order,
and bounded timeline counts for the complete Epic 1 scenario.
