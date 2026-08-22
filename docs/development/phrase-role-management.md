# Phrase-role management and initial source mapping

E2A-06 makes the phrase vocabulary a first-class Lumi-owned catalog. A
`PhraseRoleId` is the permanent logical Autoloop Category. The user-facing name
and ordering are editable presentation data; neither operation changes track
timeline references.

## Catalog invariants

- the eleven ADR-0013 defaults are installed exactly once and retain their
  stable IDs;
- custom roles receive monotonic local IDs (`custom-1`, `custom-2`, …) that are
  never derived from the display name;
- display names are non-empty, bounded, and unique case-insensitively;
- ordering is contiguous and independent from identity;
- every role owns one revisioned 24-bit sRGB color used by every phrase-aware
  Lumi surface;
- at least one role remains active;
- archiving is reversible and never deletes an ID, timeline reference, mapping,
  or future matrix reference;
- archived roles remain readable on existing timelines but cannot be assigned
  through the normal track editor;
- every mutation uses an expected catalog revision and stale writes fail with a
  typed `phraseRoleRevisionMismatch` response.

Usage diagnostics count current timeline-head phrases, affected tracks, and
logical Autoloop variants owned by the role. The Settings inspector exposes
these facts before an archive action. There is no hard-delete command.

## Provider mappings

Raw provider phrases are provenance, never planner input. A mapping key is the
pair `providerKind + normalized raw label`; the value is one stable
`PhraseRoleId`. Lumi ships separate initial profiles for the deterministic demo
provider and Rekordbox 7. A wildcard row provides an explicit visible fallback.

Mapping changes apply only when a track has no Lumi timeline yet. Reimport,
engine restart, role rename, and mapping edits never rewrite a user-owned
timeline. The Track Lighting Editor displays overlapping raw source labels in
the inspector while all editing and future planning continue to use only Lumi
role IDs. New mappings may target only active roles; if legacy data ever maps a
new timeline to an archived role, initialization fails closed instead of
silently assigning it.

## Persistence and process boundary

Phrase-role management was introduced by SQLite schema v3, which added
`library_settings` for the one-time defaults marker and catalog revision plus
`source_phrase_mappings`. Schema v15 adds the role-owned `color_rgb` value and
migrates every built-in role to the accepted palette. Role, color and mapping changes
are written in one optimistic-concurrency transaction. The Rust engine remains
the single writer; Swift submits typed add, rename, move, archive/restore,
color and mapping commands and renders only the returned authoritative
snapshot. See ADR-0036.

Verification covers domain invariants, v2→v3 migration, persistence and
restart, color persistence, stale revisions, usage diagnostics, future-only initialization,
Swift decoding, real Swift↔Rust process commands, dark/light visual evidence,
and the exact Terminal-built macOS app.
