# ADR-0028: Trusted USB sources and monotone analysis promotion

- Status: Accepted
- Date: 2026-08-10

## Context

Lumi can encounter a primary show USB and one or more trusted backups. A backup
may contain older names, beatgrids, cues or analysis files, even when it is
synchronized with Lumi later. Sync time therefore cannot be used as content
freshness, and a hash identifies a revision but does not order revisions.

## Decision

`Library > Import & Sources` exposes USB sources only. Adding a source first
performs a read-only OneLibrary inspection of `exportLibrary.db` and exposes its
playlist hierarchy. Lumi deliberately does not support the older `export.pdb`
format: unsupported media must first be exported again by a current rekordbox
version with OneLibrary enabled.
The user must select one or more playlists before Sync becomes available; an
empty or unknown selection is rejected by the engine. XML and local Rekordbox
import remain migration internals and are not part of the product workflow.
Trusted sources remain visible as one compact status row per physical volume.
Source detail and the playlist chooser are disclosed only for the selected row.
Expanding a mounted row re-inspects OneLibrary, so ejecting and remounting media
does not leave a stale in-memory playlist index.

Synchronization is playlist-scoped. Lumi processes the unique union of tracks
referenced by the selected playlists and archives prior aliases from that USB
which are no longer in scope. Selected playlists are materialized in the Lumi
Library using their full folder path and only contain tracks which resolve
unambiguously to canonical Lumi tracks. Per-source selections are remembered;
the complete USB is never selected implicitly.

Primary and backup media may expose the same logical playlist. Persistence
keeps the source-local playlist rows separate, but the Library presentation
groups equal normalized full paths into one canonical row and returns the
deduplicated union of their canonical tracks. Source status still lists every
USB relation; the UI therefore avoids duplicate playlists without erasing
source provenance or synchronization state.

Every trusted USB uses the filesystem volume UUID as its stable source identity
(with the existing Device Library identity as a compatibility fallback). On the
first sync using that stable identity, Lumi atomically reconciles obsolete
mount-specific records for the same physical USB and OneLibrary revision. Every
matched USB alias is retained independently from the canonical Lumi track. USB
metadata never overwrites Lumi-owned phrases, phrase roles,
AutoLoop choices or plan overrides.
Visible library pages include the active USB alias relations for each canonical
track. These are presentation metadata only and never change match authority.

Before sync, every playlist can be expanded into its tracks. Playlist summaries
and individual tracks show `current`, `USB newer`, `USB outdated`, `new` or
`review`. A stored playlist subscription is restored on reconnect; scanning and
browsing never mutate the Lumi database. Sync remains an explicit action.

Analysis promotion is monotone and fail-closed. OneLibrary master identities,
per-track update counters, export metadata and analysis revisions are retained
as ordering evidence:

1. the same analysis revision is `current`;
2. an initial USB analysis may enrich a canonical track without device
   provenance;
3. a different revision with a later valid Rekordbox analysis date is
   `promoted-newer`;
4. a revision with an earlier date is `protected-older` and is not applied;
5. a different revision with an equal, missing or invalid date is
   `held-conflict` and is not applied.

Beatgrid/waveform provenance and hot-cue provenance are evaluated separately.
The same trusted source may refresh its cue revision without forcing a held
beatgrid conflict; a different or older backup source remains subject to the
same date ordering. Cue enrichment only replaces provider-owned cue rows and
never changes the track analysis revision, waveform, beatgrid or Lumi timeline.

Sync persists provenance and the disposition per USB alias atomically. The UI
shows indexed, matched, current, promoted, protected and conflict counts.
Unmatched or ambiguous tracks remain held and cannot drive automatic output.

A held conflict has an explicit, component-level review. While the exact USB is
connected, Lumi parses its candidate analysis read-only and compares Beatgrid,
Cue Points, file/track metadata, raw Rekordbox Phrases and the RGB waveform
projection with the active Lumi projection. Source dates, active provenance and
revision identities are visible evidence; one combined analysis hash is never
presented as if it explained which component changed.

Review decisions are revision-bound and fail closed:

1. `Ignore This Time` is presentation-only and hides nothing beyond the current
   screen session;
2. `Do Not Sync to Lumi` records `kept-active` for that exact incoming USB
   analysis revision; a later USB revision re-enters normal comparison;
3. `Sync to Lumi & Overwrite` asks for confirmation, rereads the mounted source,
   validates both the incoming and active revisions, and atomically promotes the
   Rekordbox projection. A stale review is rejected and must be refreshed.

Raw Rekordbox phrases are provider evidence. The Lumi-authored phrase timeline,
Phrase Roles, AutoLoop choices and plan overrides are stored separately and are
never replaced by a USB review decision. Refreshing source analysis may
reconcile its beat-coordinate baseline using the existing timeline preservation
rules, but it does not turn Rekordbox phrases back into authored Lumi phrases.

The integration is named **Pro DJ Link** in all product UI. Beat Link Trigger
is a legacy development fallback; the production path is Lumi's supervised
bridge using the upstream `beat-link` library.

## Consequences

- Connecting an older backup can never downgrade the active beatgrid,
  waveform or independently versioned hot-cue set.
- Conflicting counters or identities are held for review rather than guessed.
- Synchronizing a source later does not make its content newer.
- A user can resolve a held conflict without weakening the safe automatic
  promotion policy or guessing from a timestamp alone.
- Primary and backup devices remain distinct even when their volume names or
  exported track IDs overlap.
- Selecting the same track through multiple playlists never analyzes it more
  than once in a synchronization run.
- The Pro DJ Link source contract will expose media-slot identity when the
  player supplies it. Lumi can then associate a USB mounted in a CDJ with the
  same trusted source by OneLibrary identity; player presence never triggers an
  implicit sync and is not required for ordinary USB inspection.
