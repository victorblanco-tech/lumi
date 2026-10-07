# ADR 0043 — USB identity and verified synchronization

Status: Accepted for staged implementation (E10-08).

## Decision

Keep USB operations in disposable workers, separate from playback and lighting.
Independent physical media remain independent sources even when model, playlists
and contents match. Existing source links and all Lumi-owned preparation survive.

The owner has authorized a small `.lumi-media.json` at the volume root. It contains
only schema version, random media UUID and the existing Lumi source ID. This
supersedes ADR 0028's blanket prohibition on identity metadata writes; PIONEER,
music, analysis and other Rekordbox-owned data remain read-only. Registration has
its own short deadline and failure does not turn a successful read into failure.
Malformed markers are not overwritten. Duplicate connected markers are ambiguous,
not evidence that two volumes are one device. A marker is identity evidence, not
authentication, not proof of freshness and not a remotely visible CDJ identifier.

For OneLibrary, compare analysis/cue counters only within the same nonzero master
track/database identity. Monotone increments may advance analysis; older counters
cannot replace it just because the containing export database has a later date.
Equal counters with different analysis, crossed counters and incomparable master
identities require review. Hashes establish difference, not ordering.

Automatic audio equivalence uses a complete container digest, not file ends or
title plus size. Preserve historical digests so a replaced path does not redefine
the previously imported track. Different edits must not silently inherit identity.
This deliberately prefers a duplicate/review over merging unrelated audio.

Sync requires the database revision from the scan. Recheck selected analysis and
the database before the existing atomic transaction. Report actual stages and
track progress in a reserved per-source UI area; no success until commit. Rebuild
playlist selection from stored full paths, not recyclable USB numeric IDs. Moved
playlists require explicit reselection rather than guessing from a leaf name.

## Synced playlist hierarchy

Lumi 0.6.4-dev-13 retains the native OneLibrary folder components separately
from each synced playlist's full display path. SQLite schema 21 adds the nullable
`playlists.folder_names_json` column; existing tracks, memberships, IDs, phrases
and output settings are not rewritten by the migration. New syncs preserve the
components in the existing playlist transaction.

A source scan can enrich an already synced playlist only when its trusted source,
numeric playlist ID and full stored path all match. This updates folder metadata
on the Library worker, not track data, selection or live preparation. Unknown old
paths remain literal until that USB is scanned; splitting on `/` is unsafe because
both folder and playlist names can contain it. Conflicting native hierarchies for
a logically grouped path remain unknown rather than selecting one arbitrarily.

Mac Tracks/Editor and Local Playback share one collapsible hierarchy renderer.
Folder actions change expansion only; leaf selection retains the authoritative
playlist ID and existing query/load commands. The active playlist's ancestors
are revealed, while other folders begin collapsed. No live timing, MIDI, waveform
or Remote rendering code changes are required.

Before migration, retain a checked database backup. Older binaries that reject
schema 21 require that pre-upgrade backup for rollback; do not point them at the
upgraded file. After one scan, native folders remain available offline and after
restart without further USB reads.

## Consequences and remaining boundaries

Full hashing costs USB I/O and must never run on the realtime or UI thread. A
container metadata rewrite can change the digest without changing samples; this
is conservatively a separate edition until explicitly reconciled. Verification is
not a filesystem-wide snapshot: do not export from Rekordbox during Lumi sync.

Last-known compatible plans for the same track remain preferable to no lighting.
Source-aware live lookup, compatible local audio selection and multi-stick/CDJ
acceptance remain separate E10-08 work; this ADR does not claim that those paths
can read a USB marker over Pro DJ Link or that all acceptance is complete.
