# Music-library core

Epic 2A starts with a provider-neutral, local-first library foundation. This
slice has no Rekordbox access and cannot read or modify a Rekordbox database.
Development and CI use only the repository-owned synthetic fixture.

## Ownership boundaries

| Crate | Owns | Must not own |
| --- | --- | --- |
| `lumi-library` | Canonical imported analysis and playlists, Lumi phrase timelines, phrase roles, repository port | SQL, JSON, Rekordbox, UI, process I/O |
| `lumi-library-source` | Capability-based source-provider contract | Provider discovery or database details |
| `lumi-library-demo` | Deterministic curated and scale baselines | Copyrighted audio or proprietary exports |
| `lumi-library-sqlite` | Schema migration and local repository implementation | Source-provider logic or UI state |

A source adapter returns an immutable baseline containing source identity,
analysis revision, metadata, playlists, beatgrid, waveform samples, optional
source color, audio reference, and raw source phrase observations. Source
observations are evidence for a later mapping step; they are not Lumi's editable
phrase model.

Lumi stores its own phrase timeline separately. A timeline covers complete bars
without gaps, references configurable phrase-role identifiers, and is appended
as an immutable revision. Repository writes use an expected head revision to
prevent two editors from silently overwriting each other.

## SQLite persistence

Schema version 3 separates:

- source registrations, immutable import-baseline facts, canonical track and
  playlist identities;
- current imported analysis, beat markers, waveform samples, and raw phrases;
- a revisioned configurable phrase-role catalog, one-time default seed marker,
  provider-scoped initial source mappings, and archive-safe stable IDs;
- immutable Lumi timeline revisions, edit reason, parent/restore provenance,
  per-phrase loop strategy, and their current heads.

The pair `(source_id, source_track_id)` is unique. Reimporting the same analysis
revision is a no-op and preserves the Lumi track ID. A changed source analysis
updates only imported analysis tables; it does not replace Lumi's phrase
timeline. Later reconciliation is therefore explicit and reversible.

All library, playlist, playlist-track, and revision-history reads are bounded to
200 records per page. The automated suite imports and pages a 10,000-track
synthetic baseline to protect this boundary.

## Safe development data

`fixtures/demo-library-v1/library.json` contains only invented titles, artists,
playlists, metadata, colors, and `lumi-demo://` references. The adapter derives
waveform samples, beat markers, and bounded 44.1 kHz mono PCM audio segments
from fixed inputs. This provides audible offline development data without
shipping copied music. No Rekordbox exports or local user-library paths are
used. The demo provider lives in its own adapter crate. During Epic 2A it is
selected explicitly by the development engine so Library and editor stories
remain testable without a Rekordbox installation; production source selection
will replace that explicit composition rather than leaking demo concerns into
the canonical library model.

Repository tests inject a failure during an analysis refresh and prove that the
complete earlier track survives. A separate migration fault test proves that a
partially executed schema migration is rolled back. Restart tests reopen an
on-disk database and verify imported baseline facts, tracks, playlists, roles,
the current timeline head, and all immutable timeline revisions.

Run this slice directly with:

```bash
cargo test \
  -p lumi-library \
  -p lumi-library-source \
  -p lumi-library-demo \
  -p lumi-library-sqlite
```

The repository-wide `./scripts/verify.sh` command also runs these tests and the
dependency-boundary checks.
