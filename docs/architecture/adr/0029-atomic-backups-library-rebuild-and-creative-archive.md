# ADR-0029: Atomic backups, library rebuilds and Creative Archive

- Status: Accepted
- Date: 2026-08-11

## Context

Lumi's track collection follows selected playlists on trusted Rekordbox
OneLibrary USB media. A DJ may periodically replace that entire playlist
workflow, clean an existing USB or start with new USB devices. Deleting the old
Library must not also delete hours of Lumi-owned phrase editing, track-level
AutoLoop choices, SoundSwitch mappings or application configuration.

A normal database copy alone is insufficient for this workflow: it can restore
the old collection, but it cannot carry selected creative work forward into a
new USB organization. Conversely, phrase data must never be guessed onto a
different recording or an incompatible beat structure.

## Decision

Lumi exposes `Settings > Data & Backups` as an end-user workflow. A complete
`.lumibackup` package contains one consistent SQLite snapshot, saved app
preferences and a versioned manifest. Its modules are presented as:

1. Library & Phrases;
2. Lumi Configuration;
3. Lighting Output;
4. App Preferences.

Backup and restore are only available while Lumi is `Off`. The local engine is
stopped briefly before copying SQLite so the package represents one committed
state. Restore first creates an automatic safety backup and only accepts a
validated Lumi package from the active release channel.

`Rebuild Library Content` is a separate operation. Before confirmation Lumi:

- creates a mandatory complete backup;
- shows exact track, playlist, preserved-track and Creative Archive impact;
- lets the user keep selected authored tracks immediately available;
- pins Apply to the reviewed state by a reset token.

Apply runs in one SQLite transaction. It archives the latest authored phrase
timeline for every edited track, removes obsolete tracks, playlists, source
mirrors and import baselines, and retains trusted USB identities, global phrase
roles, AutoLoop catalog/matrix, SoundSwitch/MIDI configuration and app
preferences. Demo data is suppressed after an intentional rebuild.

Creative Archive is independent of playlists and USB devices. It stores the
latest authored phrase points, loop choices, source metadata, beat count and,
where available, a content-derived audio signature. A later USB synchronization
automatically restores the timeline only when it finds exactly one identity and
the target beat count is compatible. Exact audio identity is preferred; strict
normalized title, artist, BPM and duration form a bounded fallback. Ambiguous
matches remain pending and beat-structure differences become `review`; neither
case silently modifies a track.

The complete backup retains all revision history. Creative Archive deliberately
retains the authored head needed to carry work forward; a future review UI may
offer explicit reconciliation for incompatible beat structures.

## Consequences

- A DJ can rebuild a USB-driven Library without losing authored lighting work.
- Playlist reorganizations do not affect phrase ownership or matching.
- Keeping a critical track such as a fully edited show track is an explicit,
  visible reset choice.
- Automatic relinking is deterministic and fail-closed.
- Backups remain isolated between Dev, RC and production data directories.
- Reset and restore briefly interrupt the local engine and therefore require
  `Off`; they cannot disturb a running show.
