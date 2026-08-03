//! SQLite persistence adapter for Lumi's provider-neutral music library.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;

use lumi_domain::{KeyMode, MusicalKey, PitchClass, ThemeId, TrackId};
use lumi_library::{
    AutoloopCatalog, AutoloopCatalogError, AutoloopEntryId, AutoloopMatrixCell, AutoloopTheme,
    AutoloopVariant, BeatGrid, BeatMarker, ImportResult, ImportedLibraryBaseline,
    ImportedTrackAnalysis, LibraryRepository, LibraryTrackQuery, LumiPhraseTimeline,
    PhraseInstance, PhraseLoopStrategy, PhraseRole, PhraseRoleCatalog, PhraseRoleCatalogError,
    PhraseRoleId, PhraseRoleTrackUsage, PhraseRoleUsage, PlaylistId, PlaylistPage, PlaylistSummary,
    RawPhraseObservation, SourcePhraseMapping, SourcePlaylistId, SourceRevision, SourceTrackId,
    StoredTrack, TextIdentifierError, ThemeSpecificVariant, TimelineRevision,
    TimelineRevisionOrigin, TimelineRevisionPage, TimelineRevisionReason, TimelineRevisionSummary,
    TrackColor, TrackPage, TrackPageRequest, TrackSummary, VariantId, WaveformPoint,
    normalize_source_label,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 4;
const DEFAULTS_VERSION_KEY: &str = "phrase-role-defaults-version";
const CATALOG_REVISION_KEY: &str = "phrase-role-catalog-revision";
const AUTOLOOP_DEFAULTS_VERSION_KEY: &str = "autoloop-catalog-defaults-version";
const AUTOLOOP_CATALOG_REVISION_KEY: &str = "autoloop-catalog-revision";

pub struct SqliteLibraryRepository {
    connection: Connection,
}

impl SqliteLibraryRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteLibraryError> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, SqliteLibraryError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteLibraryError> {
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let mut repository = Self { connection };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&mut self) -> Result<(), SqliteLibraryError> {
        let mut current = self.schema_version()?;
        if current > SCHEMA_VERSION {
            return Err(SqliteLibraryError::UnsupportedSchema(current));
        }
        if current == 0 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE library_sources (
                    source_id TEXT PRIMARY KEY,
                    source_kind TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    source_revision TEXT NOT NULL
                );
                CREATE TABLE import_baselines (
                    source_id TEXT NOT NULL,
                    source_revision TEXT NOT NULL,
                    source_kind TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    track_count INTEGER NOT NULL,
                    playlist_count INTEGER NOT NULL,
                    PRIMARY KEY(source_id, source_revision)
                );
                CREATE TABLE tracks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_id TEXT NOT NULL REFERENCES library_sources(source_id),
                    source_track_id TEXT NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    bpm_milli INTEGER NOT NULL,
                    key_pitch TEXT NOT NULL,
                    key_mode TEXT NOT NULL,
                    duration_millis INTEGER NOT NULL,
                    color_rgb INTEGER,
                    audio_uri TEXT NOT NULL,
                    UNIQUE(source_id, source_track_id)
                );
                CREATE INDEX tracks_title_artist ON tracks(title, artist, id);
                CREATE TABLE playlists (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_id TEXT NOT NULL REFERENCES library_sources(source_id),
                    source_playlist_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    UNIQUE(source_id, source_playlist_id)
                );
                CREATE INDEX playlists_name ON playlists(name, id);
                CREATE TABLE playlist_tracks (
                    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    position INTEGER NOT NULL,
                    PRIMARY KEY(playlist_id, position),
                    UNIQUE(playlist_id, track_id)
                );
                CREATE TABLE beat_grids (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    beats_per_bar INTEGER NOT NULL
                );
                CREATE TABLE beat_markers (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    beat_index INTEGER NOT NULL,
                    time_millis INTEGER NOT NULL,
                    bar_index INTEGER NOT NULL,
                    beat_in_bar INTEGER NOT NULL,
                    PRIMARY KEY(track_id, beat_index)
                );
                CREATE TABLE waveform_points (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    point_index INTEGER NOT NULL,
                    low INTEGER NOT NULL,
                    mid INTEGER NOT NULL,
                    high INTEGER NOT NULL,
                    PRIMARY KEY(track_id, point_index)
                );
                CREATE TABLE raw_phrases (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    phrase_index INTEGER NOT NULL,
                    start_beat INTEGER NOT NULL,
                    end_beat INTEGER NOT NULL,
                    source_label TEXT NOT NULL,
                    PRIMARY KEY(track_id, phrase_index)
                );
                CREATE TABLE phrase_roles (
                    role_id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    sort_order INTEGER NOT NULL,
                    archived INTEGER NOT NULL
                );
                CREATE TABLE library_settings (
                    key TEXT PRIMARY KEY,
                    value INTEGER NOT NULL
                );
                INSERT INTO library_settings(key, value)
                    VALUES ('phrase-role-defaults-version', 0),
                           ('phrase-role-catalog-revision', 0),
                           ('autoloop-catalog-defaults-version', 0),
                           ('autoloop-catalog-revision', 0);
                CREATE TABLE source_phrase_mappings (
                    provider_kind TEXT NOT NULL,
                    normalized_label TEXT NOT NULL,
                    raw_label TEXT NOT NULL,
                    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                    PRIMARY KEY(provider_kind, normalized_label)
                );
                CREATE TABLE autoloop_themes (
                    theme_id INTEGER PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    sort_order INTEGER NOT NULL UNIQUE
                );
                CREATE TABLE autoloop_variants (
                    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                    variant_id TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    sort_order INTEGER NOT NULL,
                    archived INTEGER NOT NULL,
                    PRIMARY KEY(role_id, variant_id),
                    UNIQUE(role_id, sort_order)
                );
                CREATE TABLE autoloop_matrix_cells (
                    theme_id INTEGER NOT NULL REFERENCES autoloop_themes(theme_id),
                    role_id TEXT NOT NULL,
                    variant_id TEXT NOT NULL,
                    entry_id TEXT NOT NULL UNIQUE,
                    display_name TEXT NOT NULL,
                    PRIMARY KEY(theme_id, role_id, variant_id),
                    FOREIGN KEY(role_id, variant_id)
                        REFERENCES autoloop_variants(role_id, variant_id)
                );
                CREATE TABLE timeline_revisions (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    revision INTEGER NOT NULL,
                    baseline_revision TEXT NOT NULL,
                    total_bars INTEGER NOT NULL,
                    origin TEXT NOT NULL,
                    reason TEXT NOT NULL,
                    parent_revision INTEGER,
                    restored_from_revision INTEGER,
                    PRIMARY KEY(track_id, revision)
                );
                CREATE TABLE phrase_instances (
                    track_id INTEGER NOT NULL,
                    revision INTEGER NOT NULL,
                    phrase_index INTEGER NOT NULL,
                    start_bar INTEGER NOT NULL,
                    end_bar INTEGER NOT NULL,
                    role_id TEXT NOT NULL,
                    loop_strategy TEXT NOT NULL,
                    PRIMARY KEY(track_id, revision, phrase_index),
                    FOREIGN KEY(track_id, revision)
                        REFERENCES timeline_revisions(track_id, revision) ON DELETE CASCADE
                );
                CREATE TABLE timeline_heads (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    revision INTEGER NOT NULL,
                    FOREIGN KEY(track_id, revision)
                        REFERENCES timeline_revisions(track_id, revision)
                );
                PRAGMA user_version = 4;
                COMMIT;
                ",
            )?;
        }
        if current == 1 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                ALTER TABLE timeline_revisions ADD COLUMN reason TEXT NOT NULL DEFAULT 'change-role';
                ALTER TABLE timeline_revisions ADD COLUMN parent_revision INTEGER;
                ALTER TABLE timeline_revisions ADD COLUMN restored_from_revision INTEGER;
                UPDATE timeline_revisions
                   SET reason = CASE origin
                       WHEN 'source-import' THEN 'initial-source-mapping'
                       WHEN 'source-reconcile' THEN 'source-reconcile'
                       WHEN 'revision-restore' THEN 'restore-revision'
                       ELSE 'change-role'
                   END,
                       parent_revision = CASE WHEN revision > 1 THEN revision - 1 ELSE NULL END;
                ALTER TABLE phrase_instances ADD COLUMN loop_strategy TEXT NOT NULL DEFAULT 'auto';
                PRAGMA user_version = 2;
                COMMIT;
                ",
            )?;
            current = 2;
        }
        if current == 2 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE library_settings (
                    key TEXT PRIMARY KEY,
                    value INTEGER NOT NULL
                );
                INSERT INTO library_settings(key, value)
                    VALUES ('phrase-role-defaults-version', 0),
                           ('phrase-role-catalog-revision', 0);
                CREATE TABLE source_phrase_mappings (
                    provider_kind TEXT NOT NULL,
                    normalized_label TEXT NOT NULL,
                    raw_label TEXT NOT NULL,
                    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                    PRIMARY KEY(provider_kind, normalized_label)
                );
                PRAGMA user_version = 3;
                COMMIT;
                ",
            )?;
            current = 3;
        }
        if current == 3 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                INSERT INTO library_settings(key, value)
                    VALUES ('autoloop-catalog-defaults-version', 0),
                           ('autoloop-catalog-revision', 0);
                CREATE TABLE autoloop_themes (
                    theme_id INTEGER PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    sort_order INTEGER NOT NULL UNIQUE
                );
                CREATE TABLE autoloop_variants (
                    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                    variant_id TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    sort_order INTEGER NOT NULL,
                    archived INTEGER NOT NULL,
                    PRIMARY KEY(role_id, variant_id),
                    UNIQUE(role_id, sort_order)
                );
                CREATE TABLE autoloop_matrix_cells (
                    theme_id INTEGER NOT NULL REFERENCES autoloop_themes(theme_id),
                    role_id TEXT NOT NULL,
                    variant_id TEXT NOT NULL,
                    entry_id TEXT NOT NULL UNIQUE,
                    display_name TEXT NOT NULL,
                    PRIMARY KEY(theme_id, role_id, variant_id),
                    FOREIGN KEY(role_id, variant_id)
                        REFERENCES autoloop_variants(role_id, variant_id)
                );
                PRAGMA user_version = 4;
                COMMIT;
                ",
            )?;
        }
        Ok(())
    }

    fn store_analysis(
        transaction: &Transaction<'_>,
        track_id: i64,
        track: &ImportedTrackAnalysis,
    ) -> Result<(), SqliteLibraryError> {
        transaction.execute(
            "INSERT INTO beat_grids(track_id, beats_per_bar) VALUES (?1, ?2)",
            params![track_id, i64::from(track.beat_grid().beats_per_bar())],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO beat_markers
                 (track_id, beat_index, time_millis, bar_index, beat_in_bar)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for marker in track.beat_grid().markers() {
                statement.execute(params![
                    track_id,
                    i64::from(marker.beat_index()),
                    to_i64(marker.time_millis())?,
                    i64::from(marker.bar_index()),
                    i64::from(marker.beat_in_bar()),
                ])?;
            }
        }
        {
            let mut statement = transaction.prepare(
                "INSERT INTO waveform_points(track_id, point_index, low, mid, high)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (index, point) in track.waveform().iter().enumerate() {
                statement.execute(params![
                    track_id,
                    usize_to_i64(index)?,
                    i64::from(point.low()),
                    i64::from(point.mid()),
                    i64::from(point.high()),
                ])?;
            }
        }
        {
            let mut statement = transaction.prepare(
                "INSERT INTO raw_phrases
                 (track_id, phrase_index, start_beat, end_beat, source_label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (index, phrase) in track.raw_phrases().iter().enumerate() {
                statement.execute(params![
                    track_id,
                    usize_to_i64(index)?,
                    i64::from(phrase.start_beat()),
                    i64::from(phrase.end_beat()),
                    phrase.source_label(),
                ])?;
            }
        }
        Ok(())
    }

    fn delete_analysis(
        transaction: &Transaction<'_>,
        track_id: i64,
    ) -> Result<(), SqliteLibraryError> {
        transaction.execute("DELETE FROM beat_grids WHERE track_id = ?1", [track_id])?;
        transaction.execute("DELETE FROM beat_markers WHERE track_id = ?1", [track_id])?;
        transaction.execute(
            "DELETE FROM waveform_points WHERE track_id = ?1",
            [track_id],
        )?;
        transaction.execute("DELETE FROM raw_phrases WHERE track_id = ?1", [track_id])?;
        Ok(())
    }

    fn load_summary(row: &Row<'_>) -> Result<TrackSummary, SqliteLibraryError> {
        let id = from_positive_i64(row.get(0)?, "track id")?;
        let source_track_id = SourceTrackId::try_new(row.get::<_, String>(1)?)?;
        let pitch = decode_pitch(&row.get::<_, String>(5)?)?;
        let mode = decode_mode(&row.get::<_, String>(6)?)?;
        let color = row
            .get::<_, Option<i64>>(8)?
            .map(|value| to_u32(value, "track color").map(TrackColor::from_rgb_u32))
            .transpose()?;
        let timeline_revision = row
            .get::<_, Option<i64>>(10)?
            .map(|value| timeline_revision(value, "timeline revision"))
            .transpose()?;
        Ok(TrackSummary::new(
            TrackId::new(id),
            source_track_id,
            row.get(2)?,
            row.get(3)?,
            to_u32(row.get(4)?, "BPM")?,
            MusicalKey::new(pitch, mode),
            from_nonnegative_i64(row.get(7)?, "duration")?,
            color,
            SourceRevision::try_new(row.get::<_, String>(9)?)?,
            timeline_revision,
        ))
    }

    fn timeline_at_revision(
        &self,
        track_id: TrackId,
        revision: TimelineRevision,
    ) -> Result<Option<LumiPhraseTimeline>, SqliteLibraryError> {
        let header = self
            .connection
            .query_row(
                "SELECT baseline_revision, total_bars, origin, reason,
                        parent_revision, restored_from_revision
                 FROM timeline_revisions WHERE track_id = ?1 AND revision = ?2",
                params![to_i64(track_id.value())?, to_i64(revision.value())?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((baseline_revision, total_bars, origin, reason, parent_revision, restored_from)) =
            header
        else {
            return Ok(None);
        };
        let mut statement = self.connection.prepare(
            "SELECT phrase_index, start_bar, end_bar, role_id, loop_strategy
             FROM phrase_instances
             WHERE track_id = ?1 AND revision = ?2 ORDER BY phrase_index",
        )?;
        let mut rows = statement.query(params![
            to_i64(track_id.value())?,
            to_i64(revision.value())?
        ])?;
        let mut phrases = Vec::new();
        while let Some(row) = rows.next()? {
            phrases.push(
                PhraseInstance::new(
                    to_u16(row.get(0)?, "phrase index")?,
                    to_u32(row.get(1)?, "start bar")?,
                    to_u32(row.get(2)?, "end bar")?,
                    PhraseRoleId::try_new(row.get::<_, String>(3)?)?,
                )
                .with_loop_strategy(decode_loop_strategy(&row.get::<_, String>(4)?)?),
            );
        }
        Ok(Some(LumiPhraseTimeline::try_new_with_history(
            track_id,
            revision,
            SourceRevision::try_new(baseline_revision)?,
            to_u32(total_bars, "total bars")?,
            decode_origin(&origin)?,
            decode_reason(&reason)?,
            parent_revision
                .map(|value| timeline_revision(value, "timeline parent revision"))
                .transpose()?,
            restored_from
                .map(|value| timeline_revision(value, "timeline restored revision"))
                .transpose()?,
            phrases,
        )?))
    }
}

impl LibraryRepository for SqliteLibraryRepository {
    type Error = SqliteLibraryError;

    fn schema_version(&self) -> Result<u32, Self::Error> {
        let value = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
        to_u32(value, "schema version")
    }

    fn import_baseline(
        &mut self,
        baseline: &ImportedLibraryBaseline,
    ) -> Result<ImportResult, Self::Error> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO library_sources(source_id, source_kind, display_name, source_revision)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id) DO UPDATE SET
               source_kind = excluded.source_kind,
               display_name = excluded.display_name,
               source_revision = excluded.source_revision",
            params![
                baseline.source_id().as_str(),
                baseline.source_kind(),
                baseline.display_name(),
                baseline.source_revision().as_str(),
            ],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO import_baselines
             (source_id, source_revision, source_kind, display_name, track_count, playlist_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                baseline.source_id().as_str(),
                baseline.source_revision().as_str(),
                baseline.source_kind(),
                baseline.display_name(),
                usize_to_i64(baseline.tracks().len())?,
                usize_to_i64(baseline.playlists().len())?,
            ],
        )?;
        let mut inserted = 0_u32;
        let mut updated = 0_u32;
        let mut unchanged = 0_u32;
        for track in baseline.tracks() {
            let current = transaction
                .query_row(
                    "SELECT id, analysis_revision FROM tracks
                     WHERE source_id = ?1 AND source_track_id = ?2",
                    params![
                        baseline.source_id().as_str(),
                        track.source_track_id().as_str()
                    ],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            match current {
                Some((_track_id, revision)) if revision == track.analysis_revision().as_str() => {
                    unchanged = unchanged
                        .checked_add(1)
                        .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
                }
                Some((track_id, _revision)) => {
                    update_track(&transaction, track_id, track)?;
                    Self::delete_analysis(&transaction, track_id)?;
                    Self::store_analysis(&transaction, track_id, track)?;
                    updated = updated
                        .checked_add(1)
                        .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
                }
                None => {
                    insert_track(&transaction, baseline.source_id().as_str(), track)?;
                    let track_id = transaction.last_insert_rowid();
                    Self::store_analysis(&transaction, track_id, track)?;
                    inserted = inserted
                        .checked_add(1)
                        .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
                }
            }
        }
        for playlist in baseline.playlists() {
            transaction.execute(
                "INSERT INTO playlists(source_id, source_playlist_id, name)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(source_id, source_playlist_id) DO UPDATE SET name = excluded.name",
                params![
                    baseline.source_id().as_str(),
                    playlist.source_playlist_id().as_str(),
                    playlist.name(),
                ],
            )?;
            let playlist_id = transaction.query_row(
                "SELECT id FROM playlists WHERE source_id = ?1 AND source_playlist_id = ?2",
                params![
                    baseline.source_id().as_str(),
                    playlist.source_playlist_id().as_str()
                ],
                |row| row.get::<_, i64>(0),
            )?;
            transaction.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist_id],
            )?;
            let mut statement = transaction.prepare(
                "INSERT INTO playlist_tracks(playlist_id, track_id, position)
                 SELECT ?1, id, ?2 FROM tracks
                 WHERE source_id = ?3 AND source_track_id = ?4",
            )?;
            for (position, source_track_id) in playlist.track_ids().iter().enumerate() {
                let affected = statement.execute(params![
                    playlist_id,
                    usize_to_i64(position)?,
                    baseline.source_id().as_str(),
                    source_track_id.as_str(),
                ])?;
                if affected != 1 {
                    return Err(SqliteLibraryError::MissingPlaylistTrack(
                        source_track_id.as_str().to_owned(),
                    ));
                }
            }
        }
        transaction.commit()?;
        Ok(ImportResult {
            inserted,
            updated,
            unchanged,
        })
    }

    fn reconcile_track(
        &mut self,
        baseline: &ImportedLibraryBaseline,
        incoming: &ImportedTrackAnalysis,
        timeline: &LumiPhraseTimeline,
        expected_head: TimelineRevision,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let track_id = transaction
            .query_row(
                "SELECT id FROM tracks WHERE source_id = ?1 AND source_track_id = ?2",
                params![
                    baseline.source_id().as_str(),
                    incoming.source_track_id().as_str()
                ],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or_else(|| {
                SqliteLibraryError::MissingReconcileTrack(
                    incoming.source_track_id().as_str().to_owned(),
                )
            })?;
        if to_i64(timeline.track_id().value())? != track_id {
            return Err(SqliteLibraryError::ReconcileTrackIdentityMismatch);
        }
        let actual_head = transaction
            .query_row(
                "SELECT revision FROM timeline_heads WHERE track_id = ?1",
                [track_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| timeline_revision(value, "timeline head"))
            .transpose()?;
        if actual_head != Some(expected_head) {
            return Err(SqliteLibraryError::RevisionConflict {
                expected: Some(expected_head),
                actual: actual_head,
            });
        }
        let required_revision = expected_head
            .checked_next()
            .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        if timeline.revision() != required_revision {
            return Err(SqliteLibraryError::InvalidNextRevision {
                required: required_revision,
                received: timeline.revision(),
            });
        }

        transaction.execute(
            "INSERT INTO import_baselines
             (source_id, source_revision, source_kind, display_name, track_count, playlist_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_id, source_revision) DO NOTHING",
            params![
                baseline.source_id().as_str(),
                baseline.source_revision().as_str(),
                baseline.source_kind(),
                baseline.display_name(),
                usize_to_i64(baseline.tracks().len())?,
                usize_to_i64(baseline.playlists().len())?,
            ],
        )?;
        transaction.execute(
            "UPDATE library_sources SET source_kind = ?1, display_name = ?2,
                    source_revision = ?3 WHERE source_id = ?4",
            params![
                baseline.source_kind(),
                baseline.display_name(),
                baseline.source_revision().as_str(),
                baseline.source_id().as_str(),
            ],
        )?;
        update_track(&transaction, track_id, incoming)?;
        Self::delete_analysis(&transaction, track_id)?;
        Self::store_analysis(&transaction, track_id, incoming)?;

        let parent_revision = timeline
            .parent_revision()
            .map(|value| to_i64(value.value()))
            .transpose()?;
        let restored_from_revision = timeline
            .restored_from()
            .map(|value| to_i64(value.value()))
            .transpose()?;
        transaction.execute(
            "INSERT INTO timeline_revisions
             (track_id, revision, baseline_revision, total_bars, origin, reason,
              parent_revision, restored_from_revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                track_id,
                to_i64(timeline.revision().value())?,
                timeline.baseline_revision().as_str(),
                i64::from(timeline.total_bars()),
                encode_origin(timeline.origin()),
                encode_reason(timeline.reason()),
                parent_revision,
                restored_from_revision,
            ],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO phrase_instances
                 (track_id, revision, phrase_index, start_bar, end_bar, role_id, loop_strategy)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for phrase in timeline.phrases() {
                statement.execute(params![
                    track_id,
                    to_i64(timeline.revision().value())?,
                    i64::from(phrase.index()),
                    i64::from(phrase.start_bar()),
                    i64::from(phrase.end_bar()),
                    phrase.role_id().as_str(),
                    encode_loop_strategy(phrase.loop_strategy())?,
                ])?;
            }
        }
        transaction.execute(
            "UPDATE timeline_heads SET revision = ?1 WHERE track_id = ?2",
            params![to_i64(timeline.revision().value())?, track_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn refresh_track_without_timeline(
        &mut self,
        baseline: &ImportedLibraryBaseline,
        incoming: &ImportedTrackAnalysis,
        expected_analysis_revision: &SourceRevision,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let (track_id, actual_revision) = transaction
            .query_row(
                "SELECT id, analysis_revision FROM tracks
                 WHERE source_id = ?1 AND source_track_id = ?2",
                params![
                    baseline.source_id().as_str(),
                    incoming.source_track_id().as_str()
                ],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                SqliteLibraryError::MissingReconcileTrack(
                    incoming.source_track_id().as_str().to_owned(),
                )
            })?;
        if actual_revision != expected_analysis_revision.as_str() {
            return Err(SqliteLibraryError::AnalysisRevisionConflict {
                expected: expected_analysis_revision.as_str().to_owned(),
                actual: actual_revision,
            });
        }
        transaction.execute(
            "INSERT INTO import_baselines
             (source_id, source_revision, source_kind, display_name, track_count, playlist_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_id, source_revision) DO NOTHING",
            params![
                baseline.source_id().as_str(),
                baseline.source_revision().as_str(),
                baseline.source_kind(),
                baseline.display_name(),
                usize_to_i64(baseline.tracks().len())?,
                usize_to_i64(baseline.playlists().len())?,
            ],
        )?;
        transaction.execute(
            "UPDATE library_sources SET source_kind = ?1, display_name = ?2,
                    source_revision = ?3 WHERE source_id = ?4",
            params![
                baseline.source_kind(),
                baseline.display_name(),
                baseline.source_revision().as_str(),
                baseline.source_id().as_str(),
            ],
        )?;
        update_track(&transaction, track_id, incoming)?;
        Self::delete_analysis(&transaction, track_id)?;
        Self::store_analysis(&transaction, track_id, incoming)?;
        transaction.commit()?;
        Ok(())
    }

    fn page_tracks(&self, request: TrackPageRequest) -> Result<TrackPage, Self::Error> {
        let total = self
            .connection
            .query_row("SELECT COUNT(*) FROM tracks", [], |row| {
                row.get::<_, i64>(0)
            })?;
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                    t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                    t.analysis_revision, h.revision
             FROM tracks t LEFT JOIN timeline_heads h ON h.track_id = t.id
             ORDER BY t.title COLLATE NOCASE, t.artist COLLATE NOCASE, t.id
             LIMIT ?1 OFFSET ?2",
        )?;
        let mut rows = statement.query(params![
            i64::from(request.limit()),
            i64::from(request.offset())
        ])?;
        let mut tracks = Vec::with_capacity(usize::from(request.limit()));
        while let Some(row) = rows.next()? {
            tracks.push(Self::load_summary(row)?);
        }
        Ok(TrackPage::new(
            from_nonnegative_i64(total, "track count")?,
            request.offset(),
            tracks,
        ))
    }

    fn query_tracks(&self, query: &LibraryTrackQuery) -> Result<TrackPage, Self::Error> {
        let pattern = search_pattern(query.search());
        let page = query.page();
        match query.playlist_id() {
            Some(playlist_id) => {
                let total = self.connection.query_row(
                    "SELECT COUNT(*)
                     FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id
                     WHERE pt.playlist_id = ?1 AND (
                       LOWER(t.title) LIKE LOWER(?2) ESCAPE '\\' OR
                       LOWER(t.artist) LIKE LOWER(?2) ESCAPE '\\' OR
                       LOWER(t.source_track_id) LIKE LOWER(?2) ESCAPE '\\'
                     )",
                    params![to_i64(playlist_id.value())?, pattern],
                    |row| row.get::<_, i64>(0),
                )?;
                let mut statement = self.connection.prepare(
                    "SELECT t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                            t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                            t.analysis_revision, h.revision
                     FROM playlist_tracks pt
                     JOIN tracks t ON t.id = pt.track_id
                     LEFT JOIN timeline_heads h ON h.track_id = t.id
                     WHERE pt.playlist_id = ?1 AND (
                       LOWER(t.title) LIKE LOWER(?2) ESCAPE '\\' OR
                       LOWER(t.artist) LIKE LOWER(?2) ESCAPE '\\' OR
                       LOWER(t.source_track_id) LIKE LOWER(?2) ESCAPE '\\'
                     )
                     ORDER BY pt.position
                     LIMIT ?3 OFFSET ?4",
                )?;
                let mut rows = statement.query(params![
                    to_i64(playlist_id.value())?,
                    pattern,
                    i64::from(page.limit()),
                    i64::from(page.offset())
                ])?;
                let mut tracks = Vec::with_capacity(usize::from(page.limit()));
                while let Some(row) = rows.next()? {
                    tracks.push(Self::load_summary(row)?);
                }
                Ok(TrackPage::new(
                    from_nonnegative_i64(total, "track query count")?,
                    page.offset(),
                    tracks,
                ))
            }
            None => {
                let total = self.connection.query_row(
                    "SELECT COUNT(*) FROM tracks t WHERE
                       LOWER(t.title) LIKE LOWER(?1) ESCAPE '\\' OR
                       LOWER(t.artist) LIKE LOWER(?1) ESCAPE '\\' OR
                       LOWER(t.source_track_id) LIKE LOWER(?1) ESCAPE '\\'",
                    [&pattern],
                    |row| row.get::<_, i64>(0),
                )?;
                let mut statement = self.connection.prepare(
                    "SELECT t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                            t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                            t.analysis_revision, h.revision
                     FROM tracks t LEFT JOIN timeline_heads h ON h.track_id = t.id
                     WHERE LOWER(t.title) LIKE LOWER(?1) ESCAPE '\\' OR
                           LOWER(t.artist) LIKE LOWER(?1) ESCAPE '\\' OR
                           LOWER(t.source_track_id) LIKE LOWER(?1) ESCAPE '\\'
                     ORDER BY t.title COLLATE NOCASE, t.artist COLLATE NOCASE, t.id
                     LIMIT ?2 OFFSET ?3",
                )?;
                let mut rows = statement.query(params![
                    pattern,
                    i64::from(page.limit()),
                    i64::from(page.offset())
                ])?;
                let mut tracks = Vec::with_capacity(usize::from(page.limit()));
                while let Some(row) = rows.next()? {
                    tracks.push(Self::load_summary(row)?);
                }
                Ok(TrackPage::new(
                    from_nonnegative_i64(total, "track query count")?,
                    page.offset(),
                    tracks,
                ))
            }
        }
    }

    fn page_playlists(&self, request: TrackPageRequest) -> Result<PlaylistPage, Self::Error> {
        let total = self
            .connection
            .query_row("SELECT COUNT(*) FROM playlists", [], |row| {
                row.get::<_, i64>(0)
            })?;
        let mut statement = self.connection.prepare(
            "SELECT p.id, p.source_playlist_id, p.name, COUNT(pt.track_id)
             FROM playlists p
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
             GROUP BY p.id, p.source_playlist_id, p.name
             ORDER BY p.name COLLATE NOCASE, p.id
             LIMIT ?1 OFFSET ?2",
        )?;
        let mut rows = statement.query(params![
            i64::from(request.limit()),
            i64::from(request.offset())
        ])?;
        let mut playlists = Vec::with_capacity(usize::from(request.limit()));
        while let Some(row) = rows.next()? {
            playlists.push(PlaylistSummary::new(
                PlaylistId::new(from_positive_i64(row.get(0)?, "playlist id")?),
                SourcePlaylistId::try_new(row.get::<_, String>(1)?)?,
                row.get(2)?,
                from_nonnegative_i64(row.get(3)?, "playlist track count")?,
            ));
        }
        Ok(PlaylistPage::new(
            from_nonnegative_i64(total, "playlist count")?,
            request.offset(),
            playlists,
        ))
    }

    fn page_playlist_tracks(
        &self,
        playlist_id: PlaylistId,
        request: TrackPageRequest,
    ) -> Result<TrackPage, Self::Error> {
        let total = self.connection.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1",
            [to_i64(playlist_id.value())?],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                    t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                    t.analysis_revision, h.revision
             FROM playlist_tracks pt
             JOIN tracks t ON t.id = pt.track_id
             LEFT JOIN timeline_heads h ON h.track_id = t.id
             WHERE pt.playlist_id = ?1
             ORDER BY pt.position
             LIMIT ?2 OFFSET ?3",
        )?;
        let mut rows = statement.query(params![
            to_i64(playlist_id.value())?,
            i64::from(request.limit()),
            i64::from(request.offset())
        ])?;
        let mut tracks = Vec::with_capacity(usize::from(request.limit()));
        while let Some(row) = rows.next()? {
            tracks.push(Self::load_summary(row)?);
        }
        Ok(TrackPage::new(
            from_nonnegative_i64(total, "playlist track count")?,
            request.offset(),
            tracks,
        ))
    }

    fn track(&self, id: TrackId) -> Result<Option<StoredTrack>, Self::Error> {
        let summary = self
            .connection
            .query_row(
                "SELECT t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                        t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                        t.analysis_revision, h.revision
                 FROM tracks t LEFT JOIN timeline_heads h ON h.track_id = t.id
                 WHERE t.id = ?1",
                [to_i64(id.value())?],
                |row| {
                    let values = (
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<i64>>(10)?,
                    );
                    Ok(values)
                },
            )
            .optional()?;
        let Some(values) = summary else {
            return Ok(None);
        };
        let summary = summary_from_values(values)?;
        let audio_uri = self.connection.query_row(
            "SELECT audio_uri FROM tracks WHERE id = ?1",
            [to_i64(id.value())?],
            |row| row.get::<_, String>(0),
        )?;
        let beats_per_bar = self.connection.query_row(
            "SELECT beats_per_bar FROM beat_grids WHERE track_id = ?1",
            [to_i64(id.value())?],
            |row| row.get::<_, i64>(0),
        )?;
        let mut beat_statement = self.connection.prepare(
            "SELECT beat_index, time_millis, bar_index, beat_in_bar
             FROM beat_markers WHERE track_id = ?1 ORDER BY beat_index",
        )?;
        let mut beat_rows = beat_statement.query([to_i64(id.value())?])?;
        let mut markers = Vec::new();
        while let Some(row) = beat_rows.next()? {
            markers.push(BeatMarker::new(
                to_u32(row.get(0)?, "beat index")?,
                from_nonnegative_i64(row.get(1)?, "beat time")?,
                to_u32(row.get(2)?, "bar index")?,
                to_u8(row.get(3)?, "beat in bar")?,
            ));
        }
        let beat_grid = BeatGrid::try_new(to_u8(beats_per_bar, "beats per bar")?, markers)?;
        let mut waveform_statement = self.connection.prepare(
            "SELECT low, mid, high FROM waveform_points
             WHERE track_id = ?1 ORDER BY point_index",
        )?;
        let mut waveform_rows = waveform_statement.query([to_i64(id.value())?])?;
        let mut waveform = Vec::new();
        while let Some(row) = waveform_rows.next()? {
            waveform.push(WaveformPoint::new(
                to_u8(row.get(0)?, "waveform low")?,
                to_u8(row.get(1)?, "waveform mid")?,
                to_u8(row.get(2)?, "waveform high")?,
            ));
        }
        let mut phrase_statement = self.connection.prepare(
            "SELECT start_beat, end_beat, source_label FROM raw_phrases
             WHERE track_id = ?1 ORDER BY phrase_index",
        )?;
        let mut phrase_rows = phrase_statement.query([to_i64(id.value())?])?;
        let mut raw_phrases = Vec::new();
        while let Some(row) = phrase_rows.next()? {
            raw_phrases.push(RawPhraseObservation::try_new(
                to_u32(row.get(0)?, "raw phrase start")?,
                to_u32(row.get(1)?, "raw phrase end")?,
                row.get::<_, String>(2)?,
            )?);
        }
        Ok(Some(StoredTrack::new(
            summary,
            audio_uri,
            beat_grid,
            waveform,
            raw_phrases,
        )))
    }

    fn phrase_role_catalog(&self) -> Result<PhraseRoleCatalog, Self::Error> {
        let revision = setting_u64(&self.connection, CATALOG_REVISION_KEY)?;
        let defaults_version = u16::try_from(setting_u64(&self.connection, DEFAULTS_VERSION_KEY)?)
            .map_err(|_| SqliteLibraryError::ArithmeticOverflow)?;
        let mut statement = self.connection.prepare(
            "SELECT role_id, display_name, sort_order, archived
             FROM phrase_roles ORDER BY sort_order, display_name COLLATE NOCASE, role_id",
        )?;
        let mut rows = statement.query([])?;
        let mut roles = Vec::new();
        while let Some(row) = rows.next()? {
            roles.push(PhraseRole::try_new(
                PhraseRoleId::try_new(row.get::<_, String>(0)?)?,
                row.get::<_, String>(1)?,
                to_u16(row.get(2)?, "phrase role sort order")?,
                row.get(3)?,
            )?);
        }
        let mut statement = self.connection.prepare(
            "SELECT provider_kind, raw_label, role_id
             FROM source_phrase_mappings ORDER BY provider_kind, normalized_label",
        )?;
        let mut rows = statement.query([])?;
        let mut mappings = Vec::new();
        while let Some(row) = rows.next()? {
            mappings.push(SourcePhraseMapping::try_new(
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                PhraseRoleId::try_new(row.get::<_, String>(2)?)?,
            )?);
        }
        Ok(PhraseRoleCatalog::try_new(
            revision,
            defaults_version,
            roles,
            mappings,
        )?)
    }

    fn initialize_phrase_role_catalog(
        &mut self,
        catalog: &PhraseRoleCatalog,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let current_defaults = setting_u64(&transaction, DEFAULTS_VERSION_KEY)?;
        if current_defaults != 0 {
            return Ok(());
        }
        if catalog.revision() != 1 || catalog.defaults_version() == 0 {
            return Err(SqliteLibraryError::InvalidPhraseRoleCatalog(
                PhraseRoleCatalogError::InvalidRevision,
            ));
        }
        for role in catalog.roles() {
            upsert_phrase_role(&transaction, role)?;
        }
        replace_mapping_rows(&transaction, catalog.mappings())?;
        set_setting(
            &transaction,
            DEFAULTS_VERSION_KEY,
            u64::from(catalog.defaults_version()),
        )?;
        set_setting(&transaction, CATALOG_REVISION_KEY, catalog.revision())?;
        transaction.commit()?;
        Ok(())
    }

    fn replace_phrase_role_catalog(
        &mut self,
        catalog: &PhraseRoleCatalog,
        expected_revision: u64,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let actual_revision = setting_u64(&transaction, CATALOG_REVISION_KEY)?;
        if actual_revision != expected_revision {
            return Err(SqliteLibraryError::PhraseRoleCatalogRevisionConflict {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
        let required = expected_revision
            .checked_add(1)
            .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        if catalog.revision() != required {
            return Err(SqliteLibraryError::InvalidPhraseRoleCatalog(
                PhraseRoleCatalogError::InvalidRevision,
            ));
        }
        for role in catalog.roles() {
            upsert_phrase_role(&transaction, role)?;
        }
        replace_mapping_rows(&transaction, catalog.mappings())?;
        set_setting(&transaction, CATALOG_REVISION_KEY, catalog.revision())?;
        transaction.commit()?;
        Ok(())
    }

    fn phrase_role_usages(&self) -> Result<Vec<PhraseRoleUsage>, Self::Error> {
        let mut statement = self.connection.prepare(
            "SELECT phrase_instances.role_id, tracks.id, tracks.title, COUNT(*)
             FROM phrase_instances
             JOIN timeline_heads
               ON timeline_heads.track_id = phrase_instances.track_id
              AND timeline_heads.revision = phrase_instances.revision
             JOIN tracks ON tracks.id = phrase_instances.track_id
             GROUP BY phrase_instances.role_id, tracks.id, tracks.title
             ORDER BY phrase_instances.role_id, tracks.title COLLATE NOCASE, tracks.id",
        )?;
        let mut rows = statement.query([])?;
        let mut grouped = BTreeMap::<PhraseRoleId, (u64, Vec<PhraseRoleTrackUsage>)>::new();
        while let Some(row) = rows.next()? {
            let role_id = PhraseRoleId::try_new(row.get::<_, String>(0)?)?;
            let count = from_nonnegative_i64(row.get(3)?, "phrase-role usage")?;
            let entry = grouped.entry(role_id).or_default();
            entry.0 = entry
                .0
                .checked_add(count)
                .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
            entry.1.push(PhraseRoleTrackUsage::new(
                TrackId::new(from_positive_i64(row.get(1)?, "track id")?),
                row.get::<_, String>(2)?,
                count,
            ));
        }
        Ok(grouped
            .into_iter()
            .map(|(role_id, (phrase_count, tracks))| {
                PhraseRoleUsage::new(role_id, phrase_count, tracks, 0)
            })
            .collect())
    }

    fn autoloop_catalog(&self) -> Result<AutoloopCatalog, Self::Error> {
        let revision = setting_u64(&self.connection, AUTOLOOP_CATALOG_REVISION_KEY)?;
        let defaults_version = u16::try_from(setting_u64(
            &self.connection,
            AUTOLOOP_DEFAULTS_VERSION_KEY,
        )?)
        .map_err(|_| SqliteLibraryError::ArithmeticOverflow)?;
        let mut theme_statement = self.connection.prepare(
            "SELECT theme_id, display_name, sort_order
             FROM autoloop_themes ORDER BY sort_order, theme_id",
        )?;
        let themes = theme_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .map(|row| {
                let (theme_id, display_name, sort_order) = row?;
                Ok(AutoloopTheme::try_new(
                    ThemeId::new(from_positive_i64(theme_id, "Autoloop Theme ID")?),
                    display_name,
                    to_u16(sort_order, "Autoloop Theme sort order")?,
                )?)
            })
            .collect::<Result<Vec<_>, SqliteLibraryError>>()?;
        let mut variant_statement = self.connection.prepare(
            "SELECT role_id, variant_id, display_name, sort_order, archived
             FROM autoloop_variants ORDER BY role_id, sort_order, variant_id",
        )?;
        let variants = variant_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, bool>(4)?,
                ))
            })?
            .map(|row| {
                let (role_id, variant_id, display_name, sort_order, archived) = row?;
                Ok(AutoloopVariant::try_new(
                    PhraseRoleId::try_new(role_id)?,
                    VariantId::try_new(variant_id)?,
                    display_name,
                    to_u16(sort_order, "Autoloop Variant sort order")?,
                    archived,
                )?)
            })
            .collect::<Result<Vec<_>, SqliteLibraryError>>()?;
        let mut cell_statement = self.connection.prepare(
            "SELECT theme_id, role_id, variant_id, entry_id, display_name
             FROM autoloop_matrix_cells ORDER BY theme_id, role_id, variant_id",
        )?;
        let cells = cell_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .map(|row| {
                let (theme_id, role_id, variant_id, entry_id, display_name) = row?;
                Ok(AutoloopMatrixCell::try_new(
                    ThemeId::new(from_positive_i64(theme_id, "Autoloop cell Theme ID")?),
                    PhraseRoleId::try_new(role_id)?,
                    VariantId::try_new(variant_id)?,
                    AutoloopEntryId::try_new(entry_id)?,
                    display_name,
                )?)
            })
            .collect::<Result<Vec<_>, SqliteLibraryError>>()?;
        Ok(AutoloopCatalog::try_new(
            revision,
            defaults_version,
            themes,
            variants,
            cells,
        )?)
    }

    fn initialize_autoloop_catalog(
        &mut self,
        catalog: &AutoloopCatalog,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let current_defaults = setting_u64(&transaction, AUTOLOOP_DEFAULTS_VERSION_KEY)?;
        if current_defaults != 0 {
            return Ok(());
        }
        if catalog.revision() != 1 || catalog.defaults_version() == 0 {
            return Err(SqliteLibraryError::InvalidAutoloopCatalog(
                AutoloopCatalogError::InvalidRevision,
            ));
        }
        replace_autoloop_rows(&transaction, catalog)?;
        set_setting(
            &transaction,
            AUTOLOOP_DEFAULTS_VERSION_KEY,
            u64::from(catalog.defaults_version()),
        )?;
        set_setting(
            &transaction,
            AUTOLOOP_CATALOG_REVISION_KEY,
            catalog.revision(),
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn replace_autoloop_catalog(
        &mut self,
        catalog: &AutoloopCatalog,
        expected_revision: u64,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let actual_revision = setting_u64(&transaction, AUTOLOOP_CATALOG_REVISION_KEY)?;
        if actual_revision != expected_revision {
            return Err(SqliteLibraryError::AutoloopCatalogRevisionConflict {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
        let required = expected_revision
            .checked_add(1)
            .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        if catalog.revision() != required {
            return Err(SqliteLibraryError::InvalidAutoloopCatalog(
                AutoloopCatalogError::InvalidRevision,
            ));
        }
        replace_autoloop_rows(&transaction, catalog)?;
        set_setting(
            &transaction,
            AUTOLOOP_CATALOG_REVISION_KEY,
            catalog.revision(),
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn append_timeline_revision(
        &mut self,
        timeline: &LumiPhraseTimeline,
        expected_head: Option<TimelineRevision>,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let actual_head = transaction
            .query_row(
                "SELECT revision FROM timeline_heads WHERE track_id = ?1",
                [to_i64(timeline.track_id().value())?],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| timeline_revision(value, "timeline head"))
            .transpose()?;
        if actual_head != expected_head {
            return Err(SqliteLibraryError::RevisionConflict {
                expected: expected_head,
                actual: actual_head,
            });
        }
        let required_revision = match actual_head {
            Some(revision) => revision
                .checked_next()
                .ok_or(SqliteLibraryError::ArithmeticOverflow)?,
            None => TimelineRevision::initial(),
        };
        if timeline.revision() != required_revision {
            return Err(SqliteLibraryError::InvalidNextRevision {
                required: required_revision,
                received: timeline.revision(),
            });
        }
        let parent_revision = timeline
            .parent_revision()
            .map(|value| to_i64(value.value()))
            .transpose()?;
        let restored_from_revision = timeline
            .restored_from()
            .map(|value| to_i64(value.value()))
            .transpose()?;
        transaction.execute(
            "INSERT INTO timeline_revisions
             (track_id, revision, baseline_revision, total_bars, origin, reason,
              parent_revision, restored_from_revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                to_i64(timeline.track_id().value())?,
                to_i64(timeline.revision().value())?,
                timeline.baseline_revision().as_str(),
                i64::from(timeline.total_bars()),
                encode_origin(timeline.origin()),
                encode_reason(timeline.reason()),
                parent_revision,
                restored_from_revision,
            ],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO phrase_instances
                 (track_id, revision, phrase_index, start_bar, end_bar, role_id, loop_strategy)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for phrase in timeline.phrases() {
                statement.execute(params![
                    to_i64(timeline.track_id().value())?,
                    to_i64(timeline.revision().value())?,
                    i64::from(phrase.index()),
                    i64::from(phrase.start_bar()),
                    i64::from(phrase.end_bar()),
                    phrase.role_id().as_str(),
                    encode_loop_strategy(phrase.loop_strategy())?,
                ])?;
            }
        }
        transaction.execute(
            "INSERT INTO timeline_heads(track_id, revision) VALUES (?1, ?2)
             ON CONFLICT(track_id) DO UPDATE SET revision = excluded.revision",
            params![
                to_i64(timeline.track_id().value())?,
                to_i64(timeline.revision().value())?
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn timeline_head(&self, track_id: TrackId) -> Result<Option<LumiPhraseTimeline>, Self::Error> {
        let revision = self
            .connection
            .query_row(
                "SELECT revision FROM timeline_heads WHERE track_id = ?1",
                [to_i64(track_id.value())?],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| timeline_revision(value, "timeline head"))
            .transpose()?;
        match revision {
            Some(revision) => self.timeline_at_revision(track_id, revision),
            None => Ok(None),
        }
    }

    fn timeline_revision(
        &self,
        track_id: TrackId,
        revision: TimelineRevision,
    ) -> Result<Option<LumiPhraseTimeline>, Self::Error> {
        self.timeline_at_revision(track_id, revision)
    }

    fn timeline_revisions(
        &self,
        track_id: TrackId,
        request: TrackPageRequest,
    ) -> Result<TimelineRevisionPage, Self::Error> {
        let total = self.connection.query_row(
            "SELECT COUNT(*) FROM timeline_revisions WHERE track_id = ?1",
            [to_i64(track_id.value())?],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection.prepare(
            "SELECT r.revision, r.baseline_revision, r.total_bars, r.origin,
                    r.reason, r.parent_revision, r.restored_from_revision,
                    COUNT(p.phrase_index)
             FROM timeline_revisions r
             LEFT JOIN phrase_instances p
               ON p.track_id = r.track_id AND p.revision = r.revision
             WHERE r.track_id = ?1
             GROUP BY r.revision, r.baseline_revision, r.total_bars, r.origin,
                      r.reason, r.parent_revision, r.restored_from_revision
             ORDER BY r.revision DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let mut rows = statement.query(params![
            to_i64(track_id.value())?,
            i64::from(request.limit()),
            i64::from(request.offset())
        ])?;
        let mut revisions = Vec::with_capacity(usize::from(request.limit()));
        while let Some(row) = rows.next()? {
            revisions.push(TimelineRevisionSummary::new(
                timeline_revision(row.get(0)?, "timeline revision")?,
                SourceRevision::try_new(row.get::<_, String>(1)?)?,
                to_u32(row.get(2)?, "timeline total bars")?,
                decode_origin(&row.get::<_, String>(3)?)?,
                decode_reason(&row.get::<_, String>(4)?)?,
                row.get::<_, Option<i64>>(5)?
                    .map(|value| timeline_revision(value, "timeline parent revision"))
                    .transpose()?,
                row.get::<_, Option<i64>>(6)?
                    .map(|value| timeline_revision(value, "timeline restored revision"))
                    .transpose()?,
                to_u32(row.get(7)?, "timeline phrase count")?,
            ));
        }
        Ok(TimelineRevisionPage::new(
            from_nonnegative_i64(total, "timeline revision count")?,
            request.offset(),
            revisions,
        ))
    }
}

fn setting_u64(connection: &Connection, key: &str) -> Result<u64, SqliteLibraryError> {
    let value = connection.query_row(
        "SELECT value FROM library_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, i64>(0),
    )?;
    from_nonnegative_i64(value, "library setting")
}

fn set_setting(
    transaction: &Transaction<'_>,
    key: &str,
    value: u64,
) -> Result<(), SqliteLibraryError> {
    transaction.execute(
        "INSERT INTO library_settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, to_i64(value)?],
    )?;
    Ok(())
}

fn upsert_phrase_role(
    transaction: &Transaction<'_>,
    role: &PhraseRole,
) -> Result<(), SqliteLibraryError> {
    transaction.execute(
        "INSERT INTO phrase_roles(role_id, display_name, sort_order, archived)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(role_id) DO UPDATE SET
           display_name = excluded.display_name,
           sort_order = excluded.sort_order,
           archived = excluded.archived",
        params![
            role.id().as_str(),
            role.display_name(),
            i64::from(role.sort_order()),
            role.is_archived(),
        ],
    )?;
    Ok(())
}

fn replace_mapping_rows(
    transaction: &Transaction<'_>,
    mappings: &[SourcePhraseMapping],
) -> Result<(), SqliteLibraryError> {
    transaction.execute("DELETE FROM source_phrase_mappings", [])?;
    let mut statement = transaction.prepare(
        "INSERT INTO source_phrase_mappings
         (provider_kind, normalized_label, raw_label, role_id)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for mapping in mappings {
        statement.execute(params![
            mapping.provider_kind(),
            normalize_source_label(mapping.raw_label()),
            mapping.raw_label(),
            mapping.role_id().as_str(),
        ])?;
    }
    Ok(())
}

fn replace_autoloop_rows(
    transaction: &Transaction<'_>,
    catalog: &AutoloopCatalog,
) -> Result<(), SqliteLibraryError> {
    transaction.execute("DELETE FROM autoloop_matrix_cells", [])?;
    transaction.execute("DELETE FROM autoloop_variants", [])?;
    transaction.execute("DELETE FROM autoloop_themes", [])?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO autoloop_themes(theme_id, display_name, sort_order)
             VALUES (?1, ?2, ?3)",
        )?;
        for theme in catalog.themes() {
            statement.execute(params![
                to_i64(theme.id().value())?,
                theme.display_name(),
                i64::from(theme.sort_order()),
            ])?;
        }
    }
    {
        let mut statement = transaction.prepare(
            "INSERT INTO autoloop_variants
             (role_id, variant_id, display_name, sort_order, archived)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for variant in catalog.variants() {
            statement.execute(params![
                variant.role_id().as_str(),
                variant.id().as_str(),
                variant.display_name(),
                i64::from(variant.sort_order()),
                variant.is_archived(),
            ])?;
        }
    }
    {
        let mut statement = transaction.prepare(
            "INSERT INTO autoloop_matrix_cells
             (theme_id, role_id, variant_id, entry_id, display_name)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for cell in catalog.cells() {
            statement.execute(params![
                to_i64(cell.theme_id().value())?,
                cell.role_id().as_str(),
                cell.variant_id().as_str(),
                cell.entry_id().as_str(),
                cell.display_name(),
            ])?;
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum SqliteLibraryError {
    #[error("SQLite library error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("library schema version {0} is newer than this Lumi build supports")]
    UnsupportedSchema(u32),
    #[error("invalid library identifier: {0}")]
    InvalidIdentifier(#[from] TextIdentifierError),
    #[error("invalid persisted beat grid: {0}")]
    InvalidBeatGrid(#[from] lumi_library::BeatGridValidationError),
    #[error("invalid persisted track: {0}")]
    InvalidTrack(#[from] lumi_library::TrackValidationError),
    #[error("invalid persisted timeline: {0}")]
    InvalidTimeline(#[from] lumi_library::TimelineValidationError),
    #[error("invalid persisted phrase-role catalog: {0}")]
    InvalidPhraseRoleCatalog(#[from] PhraseRoleCatalogError),
    #[error("invalid persisted Autoloop catalog: {0}")]
    InvalidAutoloopCatalog(#[from] AutoloopCatalogError),
    #[error("corrupt library data: {0}")]
    CorruptData(String),
    #[error("library arithmetic overflow")]
    ArithmeticOverflow,
    #[error("playlist references missing source track {0}")]
    MissingPlaylistTrack(String),
    #[error("source refresh references missing track {0}")]
    MissingReconcileTrack(String),
    #[error("source refresh track identity does not match the timeline")]
    ReconcileTrackIdentityMismatch,
    #[error("analysis revision changed; expected {expected}, actual {actual}")]
    AnalysisRevisionConflict { expected: String, actual: String },
    #[error("timeline head changed; expected {expected:?}, actual {actual:?}")]
    RevisionConflict {
        expected: Option<TimelineRevision>,
        actual: Option<TimelineRevision>,
    },
    #[error("timeline revision must be {required:?}, received {received:?}")]
    InvalidNextRevision {
        required: TimelineRevision,
        received: TimelineRevision,
    },
    #[error("phrase-role catalog changed; expected revision {expected}, actual {actual}")]
    PhraseRoleCatalogRevisionConflict { expected: u64, actual: u64 },
    #[error("Autoloop catalog changed; expected revision {expected}, actual {actual}")]
    AutoloopCatalogRevisionConflict { expected: u64, actual: u64 },
}

#[allow(clippy::too_many_arguments)]
fn insert_track(
    transaction: &Transaction<'_>,
    source_id: &str,
    track: &ImportedTrackAnalysis,
) -> Result<(), SqliteLibraryError> {
    transaction.execute(
        "INSERT INTO tracks
         (source_id, source_track_id, analysis_revision, title, artist, bpm_milli,
          key_pitch, key_mode, duration_millis, color_rgb, audio_uri)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            source_id,
            track.source_track_id().as_str(),
            track.analysis_revision().as_str(),
            track.title(),
            track.artist(),
            i64::from(track.bpm_milli()),
            encode_pitch(track.musical_key().pitch_class()),
            encode_mode(track.musical_key().mode()),
            to_i64(track.duration_millis())?,
            track.color().map(|color| i64::from(color.rgb_u32())),
            track.audio_uri(),
        ],
    )?;
    Ok(())
}

fn update_track(
    transaction: &Transaction<'_>,
    track_id: i64,
    track: &ImportedTrackAnalysis,
) -> Result<(), SqliteLibraryError> {
    transaction.execute(
        "UPDATE tracks SET
           analysis_revision = ?2, title = ?3, artist = ?4, bpm_milli = ?5,
           key_pitch = ?6, key_mode = ?7, duration_millis = ?8,
           color_rgb = ?9, audio_uri = ?10
         WHERE id = ?1",
        params![
            track_id,
            track.analysis_revision().as_str(),
            track.title(),
            track.artist(),
            i64::from(track.bpm_milli()),
            encode_pitch(track.musical_key().pitch_class()),
            encode_mode(track.musical_key().mode()),
            to_i64(track.duration_millis())?,
            track.color().map(|color| i64::from(color.rgb_u32())),
            track.audio_uri(),
        ],
    )?;
    Ok(())
}

type SummaryValues = (
    i64,
    String,
    String,
    String,
    i64,
    String,
    String,
    i64,
    Option<i64>,
    String,
    Option<i64>,
);

fn summary_from_values(values: SummaryValues) -> Result<TrackSummary, SqliteLibraryError> {
    let (
        id,
        source_track_id,
        title,
        artist,
        bpm_milli,
        pitch,
        mode,
        duration_millis,
        color,
        source_revision,
        timeline,
    ) = values;
    Ok(TrackSummary::new(
        TrackId::new(from_positive_i64(id, "track id")?),
        SourceTrackId::try_new(source_track_id)?,
        title,
        artist,
        to_u32(bpm_milli, "BPM")?,
        MusicalKey::new(decode_pitch(&pitch)?, decode_mode(&mode)?),
        from_nonnegative_i64(duration_millis, "duration")?,
        color
            .map(|value| to_u32(value, "track color").map(TrackColor::from_rgb_u32))
            .transpose()?,
        SourceRevision::try_new(source_revision)?,
        timeline
            .map(|value| timeline_revision(value, "timeline revision"))
            .transpose()?,
    ))
}

fn encode_pitch(value: PitchClass) -> &'static str {
    match value {
        PitchClass::C => "c",
        PitchClass::CSharp => "c-sharp",
        PitchClass::D => "d",
        PitchClass::DSharp => "d-sharp",
        PitchClass::E => "e",
        PitchClass::F => "f",
        PitchClass::FSharp => "f-sharp",
        PitchClass::G => "g",
        PitchClass::GSharp => "g-sharp",
        PitchClass::A => "a",
        PitchClass::ASharp => "a-sharp",
        PitchClass::B => "b",
    }
}

fn decode_pitch(value: &str) -> Result<PitchClass, SqliteLibraryError> {
    match value {
        "c" => Ok(PitchClass::C),
        "c-sharp" => Ok(PitchClass::CSharp),
        "d" => Ok(PitchClass::D),
        "d-sharp" => Ok(PitchClass::DSharp),
        "e" => Ok(PitchClass::E),
        "f" => Ok(PitchClass::F),
        "f-sharp" => Ok(PitchClass::FSharp),
        "g" => Ok(PitchClass::G),
        "g-sharp" => Ok(PitchClass::GSharp),
        "a" => Ok(PitchClass::A),
        "a-sharp" => Ok(PitchClass::ASharp),
        "b" => Ok(PitchClass::B),
        _ => Err(corrupt("musical key pitch", value)),
    }
}

fn encode_mode(value: KeyMode) -> &'static str {
    match value {
        KeyMode::Major => "major",
        KeyMode::Minor => "minor",
    }
}

fn decode_mode(value: &str) -> Result<KeyMode, SqliteLibraryError> {
    match value {
        "major" => Ok(KeyMode::Major),
        "minor" => Ok(KeyMode::Minor),
        _ => Err(corrupt("musical key mode", value)),
    }
}

fn encode_origin(value: TimelineRevisionOrigin) -> &'static str {
    match value {
        TimelineRevisionOrigin::SourceImport => "source-import",
        TimelineRevisionOrigin::UserEdit => "user-edit",
        TimelineRevisionOrigin::SourceReconcile => "source-reconcile",
        TimelineRevisionOrigin::RevisionRestore => "revision-restore",
    }
}

fn decode_origin(value: &str) -> Result<TimelineRevisionOrigin, SqliteLibraryError> {
    match value {
        "source-import" => Ok(TimelineRevisionOrigin::SourceImport),
        "user-edit" => Ok(TimelineRevisionOrigin::UserEdit),
        "source-reconcile" => Ok(TimelineRevisionOrigin::SourceReconcile),
        "revision-restore" => Ok(TimelineRevisionOrigin::RevisionRestore),
        _ => Err(corrupt("timeline origin", value)),
    }
}

fn encode_reason(value: TimelineRevisionReason) -> &'static str {
    match value {
        TimelineRevisionReason::InitialSourceMapping => "initial-source-mapping",
        TimelineRevisionReason::CreatePhrase => "create-phrase",
        TimelineRevisionReason::SplitPhrase => "split-phrase",
        TimelineRevisionReason::MergePrevious => "merge-previous",
        TimelineRevisionReason::MergeNext => "merge-next",
        TimelineRevisionReason::MoveBoundary => "move-boundary",
        TimelineRevisionReason::AbsorbPrevious => "absorb-previous",
        TimelineRevisionReason::AbsorbNext => "absorb-next",
        TimelineRevisionReason::ChangeRole => "change-role",
        TimelineRevisionReason::ChangeLoopStrategy => "change-loop-strategy",
        TimelineRevisionReason::Undo => "undo",
        TimelineRevisionReason::Redo => "redo",
        TimelineRevisionReason::RestoreRevision => "restore-revision",
        TimelineRevisionReason::SourceReconcile => "source-reconcile",
    }
}

fn decode_reason(value: &str) -> Result<TimelineRevisionReason, SqliteLibraryError> {
    match value {
        "initial-source-mapping" => Ok(TimelineRevisionReason::InitialSourceMapping),
        "create-phrase" => Ok(TimelineRevisionReason::CreatePhrase),
        "split-phrase" => Ok(TimelineRevisionReason::SplitPhrase),
        "merge-previous" => Ok(TimelineRevisionReason::MergePrevious),
        "merge-next" => Ok(TimelineRevisionReason::MergeNext),
        "move-boundary" => Ok(TimelineRevisionReason::MoveBoundary),
        "absorb-previous" => Ok(TimelineRevisionReason::AbsorbPrevious),
        "absorb-next" => Ok(TimelineRevisionReason::AbsorbNext),
        "change-role" => Ok(TimelineRevisionReason::ChangeRole),
        "change-loop-strategy" => Ok(TimelineRevisionReason::ChangeLoopStrategy),
        "undo" => Ok(TimelineRevisionReason::Undo),
        "redo" => Ok(TimelineRevisionReason::Redo),
        "restore-revision" => Ok(TimelineRevisionReason::RestoreRevision),
        "source-reconcile" => Ok(TimelineRevisionReason::SourceReconcile),
        _ => Err(corrupt("timeline revision reason", value)),
    }
}

fn encode_loop_strategy(value: &PhraseLoopStrategy) -> Result<String, SqliteLibraryError> {
    let json = match value {
        PhraseLoopStrategy::Auto => serde_json::json!({ "kind": "auto" }),
        PhraseLoopStrategy::FixedVariant(variant) => serde_json::json!({
            "kind": "fixedVariant",
            "variantId": variant.as_str(),
        }),
        PhraseLoopStrategy::ThemeSpecificExact(overrides) => serde_json::json!({
            "kind": "themeSpecificExact",
            "overrides": overrides.iter().map(|value| serde_json::json!({
                "themeId": value.theme_id().value(),
                "variantId": value.variant_id().as_str(),
            })).collect::<Vec<_>>(),
        }),
    };
    serde_json::to_string(&json).map_err(|error| SqliteLibraryError::CorruptData(error.to_string()))
}

fn decode_loop_strategy(value: &str) -> Result<PhraseLoopStrategy, SqliteLibraryError> {
    if value == "auto" {
        return Ok(PhraseLoopStrategy::Auto);
    }
    let json: serde_json::Value = serde_json::from_str(value)
        .map_err(|error| SqliteLibraryError::CorruptData(error.to_string()))?;
    let kind = json
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| corrupt("loop strategy kind", value))?;
    match kind {
        "auto" => Ok(PhraseLoopStrategy::Auto),
        "fixedVariant" => Ok(PhraseLoopStrategy::FixedVariant(VariantId::try_new(
            json.get("variantId")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| corrupt("fixed variant id", value))?,
        )?)),
        "themeSpecificExact" => {
            let values = json
                .get("overrides")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| corrupt("theme-specific overrides", value))?;
            let mut overrides = Vec::with_capacity(values.len());
            for item in values {
                let theme_id = item
                    .get("themeId")
                    .and_then(serde_json::Value::as_u64)
                    .filter(|value| *value > 0)
                    .ok_or_else(|| corrupt("theme-specific theme id", value))?;
                let variant_id = item
                    .get("variantId")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| corrupt("theme-specific variant id", value))?;
                overrides.push(ThemeSpecificVariant::new(
                    ThemeId::new(theme_id),
                    VariantId::try_new(variant_id)?,
                ));
            }
            Ok(PhraseLoopStrategy::ThemeSpecificExact(overrides))
        }
        _ => Err(corrupt("loop strategy kind", kind)),
    }
}

fn timeline_revision(value: i64, label: &str) -> Result<TimelineRevision, SqliteLibraryError> {
    TimelineRevision::try_new(from_positive_i64(value, label)?).map_err(Into::into)
}

fn to_i64(value: u64) -> Result<i64, SqliteLibraryError> {
    i64::try_from(value).map_err(|_| SqliteLibraryError::ArithmeticOverflow)
}

fn usize_to_i64(value: usize) -> Result<i64, SqliteLibraryError> {
    i64::try_from(value).map_err(|_| SqliteLibraryError::ArithmeticOverflow)
}

fn from_positive_i64(value: i64, label: &str) -> Result<u64, SqliteLibraryError> {
    if value <= 0 {
        return Err(corrupt(label, value));
    }
    u64::try_from(value).map_err(|_| corrupt(label, value))
}

fn from_nonnegative_i64(value: i64, label: &str) -> Result<u64, SqliteLibraryError> {
    u64::try_from(value).map_err(|_| corrupt(label, value))
}

fn to_u32(value: i64, label: &str) -> Result<u32, SqliteLibraryError> {
    u32::try_from(value).map_err(|_| corrupt(label, value))
}

fn to_u16(value: i64, label: &str) -> Result<u16, SqliteLibraryError> {
    u16::try_from(value).map_err(|_| corrupt(label, value))
}

fn to_u8(value: i64, label: &str) -> Result<u8, SqliteLibraryError> {
    u8::try_from(value).map_err(|_| corrupt(label, value))
}

fn corrupt(label: &str, value: impl std::fmt::Display) -> SqliteLibraryError {
    SqliteLibraryError::CorruptData(format!("invalid {label}: {value}"))
}

fn search_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[cfg(test)]
mod fault_tests {
    use lumi_library::{
        ImportedLibraryBaseline, ImportedTrackAnalysis, LibraryRepository, SourceRevision,
        TrackPageRequest,
    };
    use lumi_library_demo::DemoLibrarySourceProvider;
    use lumi_library_source::MusicLibrarySourceProvider;

    use super::SqliteLibraryRepository;

    #[test]
    fn failed_analysis_refresh_rolls_back_the_complete_previous_track()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let source_track = &baseline.tracks()[0];
        let track_id = repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .tracks()
            .iter()
            .find(|track| track.source_track_id() == source_track.source_track_id())
            .ok_or("stored source track not found")?
            .id();
        let before = repository
            .track(track_id)?
            .ok_or("stored track not found before fault")?;

        let changed_track = ImportedTrackAnalysis::try_new(
            source_track.source_track_id().clone(),
            SourceRevision::try_new("forced-failure-v2")?,
            "This update must roll back",
            source_track.artist(),
            source_track.bpm_milli(),
            source_track.musical_key(),
            source_track.duration_millis(),
            source_track.color(),
            source_track.audio_uri(),
            source_track.beat_grid().clone(),
            source_track.waveform().to_vec(),
            source_track.raw_phrases().to_vec(),
        )?;
        let mut changed_tracks = baseline.tracks().to_vec();
        changed_tracks[0] = changed_track;
        let changed_baseline = ImportedLibraryBaseline::try_new(
            baseline.source_id().clone(),
            baseline.source_kind(),
            baseline.display_name(),
            SourceRevision::try_new("forced-failure-v2")?,
            changed_tracks,
            baseline.playlists().to_vec(),
        )?;
        repository.connection.execute_batch(
            "CREATE TRIGGER force_waveform_failure
             BEFORE INSERT ON waveform_points
             BEGIN SELECT RAISE(ABORT, 'injected waveform failure'); END;",
        )?;

        assert!(repository.import_baseline(&changed_baseline).is_err());
        let after = repository
            .track(track_id)?
            .ok_or("stored track not found after rollback")?;
        assert_eq!(after, before);
        Ok(())
    }
}
