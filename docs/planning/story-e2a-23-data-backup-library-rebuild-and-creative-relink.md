# Story E2A-09: Data backup, Library rebuild and creative relinking

## User outcome

As a Lumi user, I can periodically start again with a cleaned or reorganized
Rekordbox USB workflow while keeping the phrase and lighting work that belongs
to my music.

## Functional slice

- `Settings > Data & Backups` shows complete Lumi backups and their restore
  actions.
- A backup covers Library & Phrases, Lumi Configuration, Lighting Output and
  App Preferences in one `.lumibackup` package.
- Rebuild shows exact impact and automatically creates a mandatory pre-reset
  backup.
- User-edited tracks are offered as optional immediate keeps; for the current
  Dev data this includes the authored `90s Bitch - Extended Mix` timeline.
- Unkept authored timelines move to Creative Archive rather than being
  deleted.
- A later selected-playlist USB sync automatically relinks exact compatible
  creative work and visibly reports `pending`, `review`, `preserved` or
  `restored` state.
- All destructive operations require Lumi `Off` and use reviewed, token-bound,
  transactional commands.

## Acceptance criteria

- The real user database is never used by automated reset tests.
- A temporary database test proves archive, reset, USB-style reimport and exact
  phrase restoration.
- A second test proves that selected authored tracks remain directly available
  while playlists are removed.
- An incompatible beat count does not auto-apply archived phrase points.
- A failed or stale Apply leaves no partial reset.
- Restore creates a safety backup before replacing data.
- Dev, RC and production keep their existing isolated Application Support
  locations and backup folders.

## Status

Implemented for `0.4.0-dev-15`; macOS build and end-user validation remain the
release gate before promotion.
