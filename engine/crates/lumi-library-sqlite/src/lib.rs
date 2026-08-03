//! SQLite persistence adapter for Lumi's provider-neutral music library.

#![forbid(unsafe_code)]

use std::path::Path;

use lumi_domain::{KeyMode, MusicalKey, PitchClass, TrackId};
use lumi_library::{
    BeatGrid, BeatMarker, ImportResult, ImportedLibraryBaseline, ImportedTrackAnalysis,
    LibraryRepository, LibraryTrackQuery, LumiPhraseTimeline, PhraseInstance, PhraseRole,
    PhraseRoleId, PlaylistId, PlaylistPage, PlaylistSummary, RawPhraseObservation,
    SourcePlaylistId, SourceRevision, SourceTrackId, StoredTrack, TextIdentifierError,
    TimelineRevision, TimelineRevisionOrigin, TimelineRevisionPage, TimelineRevisionSummary,
    TrackColor, TrackPage, TrackPageRequest, TrackSummary, WaveformPoint,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;

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
        let current = self.schema_version()?;
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
                CREATE TABLE timeline_revisions (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    revision INTEGER NOT NULL,
                    baseline_revision TEXT NOT NULL,
                    total_bars INTEGER NOT NULL,
                    origin TEXT NOT NULL,
                    PRIMARY KEY(track_id, revision)
                );
                CREATE TABLE phrase_instances (
                    track_id INTEGER NOT NULL,
                    revision INTEGER NOT NULL,
                    phrase_index INTEGER NOT NULL,
                    start_bar INTEGER NOT NULL,
                    end_bar INTEGER NOT NULL,
                    role_id TEXT NOT NULL,
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
                PRAGMA user_version = 1;
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
                "SELECT baseline_revision, total_bars, origin
                 FROM timeline_revisions WHERE track_id = ?1 AND revision = ?2",
                params![to_i64(track_id.value())?, to_i64(revision.value())?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((baseline_revision, total_bars, origin)) = header else {
            return Ok(None);
        };
        let mut statement = self.connection.prepare(
            "SELECT phrase_index, start_bar, end_bar, role_id
             FROM phrase_instances
             WHERE track_id = ?1 AND revision = ?2 ORDER BY phrase_index",
        )?;
        let mut rows = statement.query(params![
            to_i64(track_id.value())?,
            to_i64(revision.value())?
        ])?;
        let mut phrases = Vec::new();
        while let Some(row) = rows.next()? {
            phrases.push(PhraseInstance::new(
                to_u16(row.get(0)?, "phrase index")?,
                to_u32(row.get(1)?, "start bar")?,
                to_u32(row.get(2)?, "end bar")?,
                PhraseRoleId::try_new(row.get::<_, String>(3)?)?,
            ));
        }
        Ok(Some(LumiPhraseTimeline::try_new(
            track_id,
            revision,
            SourceRevision::try_new(baseline_revision)?,
            to_u32(total_bars, "total bars")?,
            decode_origin(&origin)?,
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

    fn save_phrase_roles(&mut self, roles: &[PhraseRole]) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        for role in roles {
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
        }
        transaction.commit()?;
        Ok(())
    }

    fn phrase_roles(&self) -> Result<Vec<PhraseRole>, Self::Error> {
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
        Ok(roles)
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
        transaction.execute(
            "INSERT INTO timeline_revisions
             (track_id, revision, baseline_revision, total_bars, origin)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                to_i64(timeline.track_id().value())?,
                to_i64(timeline.revision().value())?,
                timeline.baseline_revision().as_str(),
                i64::from(timeline.total_bars()),
                encode_origin(timeline.origin()),
            ],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO phrase_instances
                 (track_id, revision, phrase_index, start_bar, end_bar, role_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for phrase in timeline.phrases() {
                statement.execute(params![
                    to_i64(timeline.track_id().value())?,
                    to_i64(timeline.revision().value())?,
                    i64::from(phrase.index()),
                    i64::from(phrase.start_bar()),
                    i64::from(phrase.end_bar()),
                    phrase.role_id().as_str(),
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
                    COUNT(p.phrase_index)
             FROM timeline_revisions r
             LEFT JOIN phrase_instances p
               ON p.track_id = r.track_id AND p.revision = r.revision
             WHERE r.track_id = ?1
             GROUP BY r.revision, r.baseline_revision, r.total_bars, r.origin
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
                to_u32(row.get(4)?, "timeline phrase count")?,
            ));
        }
        Ok(TimelineRevisionPage::new(
            from_nonnegative_i64(total, "timeline revision count")?,
            request.offset(),
            revisions,
        ))
    }
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
    #[error("invalid persisted phrase role: {0}")]
    InvalidPhraseRole(#[from] lumi_library::TrackPageRequestError),
    #[error("corrupt library data: {0}")]
    CorruptData(String),
    #[error("library arithmetic overflow")]
    ArithmeticOverflow,
    #[error("playlist references missing source track {0}")]
    MissingPlaylistTrack(String),
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
