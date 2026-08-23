//! SQLite persistence adapter for Lumi's provider-neutral music library.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lumi_domain::{KeyMode, MusicalKey, PitchClass, ThemeId, TrackId};
use lumi_library::{
    AutoloopCatalog, AutoloopCatalogError, AutoloopEntryId, AutoloopMatrixCell, AutoloopTheme,
    AutoloopVariant, BeatGrid, BeatMarker, HotCue, ImportResult, ImportedLibraryBaseline,
    ImportedTrackAnalysis, LibraryRepository, LibraryTrackQuery, LumiPhraseTimeline,
    PhraseInstance, PhraseLoopStrategy, PhraseRole, PhraseRoleCatalog, PhraseRoleCatalogError,
    PhraseRoleId, PhraseRoleTrackUsage, PhraseRoleUsage, PlaylistId, PlaylistPage, PlaylistSummary,
    RawPhraseObservation, SourceMirrorDiff, SourceMirrorSnapshot, SourceMirrorSummary,
    SourcePhraseMapping, SourcePlaylistId, SourceRevision, SourceTrackId, StoredTrack,
    TextIdentifierError, ThemeSpecificVariant, TimelineRevision, TimelineRevisionOrigin,
    TimelineRevisionPage, TimelineRevisionReason, TimelineRevisionSummary, TrackColor, TrackPage,
    TrackPageRequest, TrackSummary, VariantId, WaveformPoint, normalize_source_label,
};
use lumi_light_plans::LightPlanningPolicy;
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 15;
const DEFAULTS_VERSION_KEY: &str = "phrase-role-defaults-version";
const CATALOG_REVISION_KEY: &str = "phrase-role-catalog-revision";
const AUTOLOOP_DEFAULTS_VERSION_KEY: &str = "autoloop-catalog-defaults-version";
const AUTOLOOP_CATALOG_REVISION_KEY: &str = "autoloop-catalog-revision";

pub struct SqliteLibraryRepository {
    connection: Connection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceMatchCandidate {
    pub track_id: TrackId,
    pub source_id: String,
    pub source_kind: String,
    pub has_user_timeline_edits: bool,
    pub title: String,
    pub artist: String,
    pub bpm_milli: u32,
    pub duration_millis: u64,
    pub audio_uri: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredDevicePlaylistSummary {
    pub playlist_id: PlaylistId,
    pub source_id: String,
    pub device_playlist_id: u32,
    pub name: String,
    pub track_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceAliasUpsert {
    pub device_track_id: u32,
    pub simulator_signature: u32,
    pub canonical_track_id: Option<TrackId>,
    pub match_kind: String,
    pub title: String,
    pub artist: String,
    pub bpm_milli: u32,
    pub duration_millis: u64,
    pub file_size: u32,
    pub audio_uri: String,
    pub metadata_revision: String,
    pub color_rgb: Option<u32>,
    pub master_database_id: u32,
    pub master_content_id: u32,
    pub information_update_count: u32,
    pub analysis_revision: String,
    pub audio_signature: String,
    pub analyzed_at: String,
    pub sync_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlaylistUpsert {
    pub device_playlist_id: u32,
    pub path: String,
    pub device_track_ids: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceAliasResolution {
    pub source_id: String,
    pub display_name: String,
    pub device_track_id: u32,
    pub canonical_track_id: TrackId,
    pub analysis_revision: String,
    pub match_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceTrackSourceRelation {
    pub track_id: TrackId,
    pub source_id: String,
    pub display_name: String,
    pub sync_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredDeviceAliasState {
    pub device_track_id: u32,
    pub canonical_track_id: Option<TrackId>,
    pub match_kind: String,
    pub metadata_revision: String,
    pub analysis_revision: String,
    pub sync_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceAnalysisUpsert {
    pub track_id: TrackId,
    pub source_id: String,
    pub device_track_id: u32,
    pub analysis_revision: String,
    pub source_analysis_revision: String,
    pub analyzed_at: String,
    pub duration_millis: u64,
    pub beat_grid: BeatGrid,
    pub waveform: Vec<WaveformPoint>,
    pub raw_phrases: Vec<RawPhraseObservation>,
    pub hot_cues: Vec<HotCue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceHotCueUpsert {
    pub track_id: TrackId,
    pub source_id: String,
    pub device_track_id: u32,
    pub source_analysis_revision: String,
    pub analyzed_at: String,
    pub hot_cues: Vec<HotCue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceTrackImport {
    pub device_track_id: u32,
    pub source_analysis_revision: String,
    pub analyzed_at: String,
    pub analysis: ImportedTrackAnalysis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceAnalysisDecision {
    Current,
    KeepActive,
    PromoteInitial,
    PromoteNewer,
    ProtectOlder,
    HoldConflict,
}

impl DeviceAnalysisDecision {
    #[must_use]
    pub const fn disposition(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::KeepActive => "kept-active",
            Self::PromoteInitial => "promoted-initial",
            Self::PromoteNewer => "promoted-newer",
            Self::ProtectOlder => "protected-older",
            Self::HoldConflict => "held-conflict",
        }
    }

    #[must_use]
    pub const fn promotes(self) -> bool {
        matches!(self, Self::PromoteInitial | Self::PromoteNewer)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceSourceSummary {
    pub source_id: String,
    pub display_name: String,
    pub database_revision: String,
    pub active_tracks: u64,
    pub matched_tracks: u64,
    pub synced_at: String,
    pub current_tracks: u64,
    pub promoted_tracks: u64,
    pub protected_tracks: u64,
    pub conflict_tracks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceReviewTrackSummary {
    pub source_id: String,
    pub device_track_id: u32,
    pub canonical_track_id: Option<TrackId>,
    pub title: String,
    pub artist: String,
    pub bpm_milli: u32,
    pub duration_millis: u64,
    pub incoming_analyzed_at: String,
    pub incoming_analysis_revision: String,
    pub incoming_metadata_revision: String,
    pub incoming_file_size: u32,
    pub active_source_name: Option<String>,
    pub active_analyzed_at: Option<String>,
    pub active_analysis_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryDataSummary {
    pub track_count: u64,
    pub playlist_count: u64,
    pub user_edited_track_count: u64,
    pub creative_archive_count: u64,
    pub pending_archive_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackColorSummary {
    pub color_rgb: u32,
    pub track_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreativeArchiveSummary {
    pub archive_id: u64,
    pub title: String,
    pub artist: String,
    pub phrase_count: u64,
    pub total_beats: u32,
    pub state: String,
    pub restored_track_id: Option<TrackId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetPreservableTrackSummary {
    pub track_id: TrackId,
    pub title: String,
    pub artist: String,
    pub timeline_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryResetImpact {
    pub track_count: u64,
    pub playlist_count: u64,
    pub preserved_track_count: u64,
    pub removed_track_count: u64,
    pub archived_creative_track_count: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CreativeRelinkResult {
    pub restored: u64,
    pub pending_review: u64,
}

impl SqliteLibraryRepository {
    /// Loads the complete revisioned planning policy. A missing row is a valid
    /// pre-feature database and receives safe defaults.
    pub fn light_planning_policy(&self) -> Result<LightPlanningPolicy, SqliteLibraryError> {
        let encoded = self
            .connection
            .query_row(
                "SELECT policy_json FROM light_planning_policy WHERE singleton_id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let policy = encoded.map_or_else(
            || Ok(LightPlanningPolicy::default()),
            |value| serde_json::from_str(&value),
        )?;
        policy.validate().map_err(|error| {
            SqliteLibraryError::CorruptData(format!("invalid Light Planning Policy: {error}"))
        })?;
        Ok(policy)
    }

    /// Atomically replaces the policy when the caller still owns the expected
    /// revision. Existing plans retain their compiled revision.
    pub fn replace_light_planning_policy(
        &mut self,
        expected_revision: u64,
        mut policy: LightPlanningPolicy,
    ) -> Result<LightPlanningPolicy, SqliteLibraryError> {
        let current = self.light_planning_policy()?;
        if current.revision != expected_revision {
            return Err(SqliteLibraryError::LightPlanningRevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        policy.revision = expected_revision
            .checked_add(1)
            .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        policy.validate().map_err(|error| {
            SqliteLibraryError::CorruptData(format!("invalid Light Planning Policy: {error}"))
        })?;
        let encoded = serde_json::to_string(&policy)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO light_planning_policy(singleton_id, revision, policy_json, updated_at)
             VALUES (1, ?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(singleton_id) DO UPDATE SET
                revision = excluded.revision,
                policy_json = excluded.policy_json,
                updated_at = CURRENT_TIMESTAMP",
            params![to_i64(policy.revision)?, encoded],
        )?;
        transaction.commit()?;
        Ok(policy)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteLibraryError> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, SqliteLibraryError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Creates a coherent SQLite snapshot while this repository remains the
    /// sole database owner. WAL pages are copied through SQLite's backup API;
    /// callers never need to copy `-wal` or `-shm` files.
    pub fn create_consistent_backup(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<(), SqliteLibraryError> {
        let destination = destination.as_ref();
        let staging = backup_staging_path(destination);
        if staging.exists() {
            fs::remove_file(&staging)?;
        }
        if destination.exists() {
            return Err(SqliteLibraryError::BackupDestinationExists(
                destination.display().to_string(),
            ));
        }
        let result = (|| {
            let mut target = Connection::open(&staging)?;
            Backup::new(&self.connection, &mut target)?.run_to_completion(
                128,
                Duration::from_millis(2),
                None,
            )?;
            validate_backup_connection(&target)?;
            drop(target);
            fs::rename(&staging, destination)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&staging);
        }
        result
    }

    pub fn validate_backup(path: impl AsRef<Path>) -> Result<(), SqliteLibraryError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        validate_backup_connection(&connection)
    }

    /// Restores a validated backup into the live owning connection. A coherent
    /// rollback snapshot is created first and is replayed automatically if the
    /// incoming copy or its post-restore integrity check fails.
    pub fn restore_consistent_backup(
        &mut self,
        source: impl AsRef<Path>,
        rollback: impl AsRef<Path>,
    ) -> Result<(), SqliteLibraryError> {
        let source = source.as_ref();
        let rollback = rollback.as_ref();
        Self::validate_backup(source)?;
        self.create_consistent_backup(rollback)?;
        let incoming = Connection::open_with_flags(
            source,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let restore_result = (|| {
            Backup::new(&incoming, &mut self.connection)?.run_to_completion(
                128,
                Duration::from_millis(2),
                None,
            )?;
            validate_backup_connection(&self.connection)
        })();
        if let Err(error) = restore_result {
            let safety = Connection::open_with_flags(
                rollback,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            Backup::new(&safety, &mut self.connection)?.run_to_completion(
                128,
                Duration::from_millis(2),
                None,
            )?;
            validate_backup_connection(&self.connection)?;
            return Err(error);
        }
        Ok(())
    }

    /// Returns imported tracks that have source phrases but no Lumi timeline.
    /// The query deliberately avoids materializing waveform blobs.
    pub fn track_ids_missing_timelines(
        &self,
        limit: u16,
    ) -> Result<Vec<TrackId>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT t.id
               FROM tracks t
              WHERE EXISTS (
                        SELECT 1 FROM raw_phrases r WHERE r.track_id = t.id
                    )
                AND NOT EXISTS (
                        SELECT 1 FROM timeline_heads h WHERE h.track_id = t.id
                    )
              ORDER BY t.id
              LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::from(limit)], |row| row.get::<_, i64>(0))?;
        let mut track_ids = Vec::new();
        for row in rows {
            track_ids.push(TrackId::new(from_positive_i64(row?, "track id")?));
        }
        Ok(track_ids)
    }

    pub fn device_match_candidates(&self) -> Result<Vec<DeviceMatchCandidate>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.source_id, s.source_kind,
                    EXISTS(
                        SELECT 1 FROM timeline_revisions r
                         WHERE r.track_id = t.id
                           AND (r.revision <> 1 OR r.origin <> 'source-import')
                    ),
                    t.title, t.artist, t.bpm_milli, t.duration_millis, t.audio_uri
               FROM tracks t
               JOIN library_sources s ON s.source_id = t.source_id
              ORDER BY t.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;
        let mut candidates = Vec::new();
        for row in rows {
            let (
                track_id,
                source_id,
                source_kind,
                has_user_timeline_edits,
                title,
                artist,
                bpm,
                duration,
                audio_uri,
            ) = row?;
            candidates.push(DeviceMatchCandidate {
                track_id: TrackId::new(from_positive_i64(track_id, "track id")?),
                source_id,
                source_kind,
                has_user_timeline_edits,
                title,
                artist,
                bpm_milli: i64_to_u32(bpm, "track BPM")?,
                duration_millis: from_positive_i64(duration, "track duration")?,
                audio_uri,
            });
        }
        Ok(candidates)
    }

    pub fn stored_device_playlists(
        &self,
    ) -> Result<BTreeMap<String, Vec<StoredDevicePlaylistSummary>>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT p.id, p.source_id, p.source_playlist_id, p.name, COUNT(pt.track_id)
               FROM playlists p
               JOIN device_library_sources s ON s.source_id = p.source_id
               LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
              GROUP BY p.id, p.source_id, p.source_playlist_id, p.name
              ORDER BY p.source_id, p.name COLLATE NOCASE, p.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut playlists = BTreeMap::<String, Vec<StoredDevicePlaylistSummary>>::new();
        for row in rows {
            let (playlist_id, source_id, source_playlist_id, name, track_count) = row?;
            let Some(device_playlist_id) = source_playlist_id
                .strip_prefix("onelibrary:")
                .or_else(|| source_playlist_id.strip_prefix("devicesql:"))
                .and_then(|value| value.parse::<u32>().ok())
            else {
                continue;
            };
            playlists
                .entry(source_id.clone())
                .or_default()
                .push(StoredDevicePlaylistSummary {
                    playlist_id: PlaylistId::new(from_positive_i64(playlist_id, "playlist id")?),
                    source_id,
                    device_playlist_id,
                    name,
                    track_count: from_nonnegative_i64(track_count, "playlist track count")?,
                });
        }
        Ok(playlists)
    }

    pub fn device_alias_states(
        &self,
        source_id: &str,
    ) -> Result<BTreeMap<u32, StoredDeviceAliasState>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT device_track_id, canonical_track_id, match_kind, metadata_revision,
                    analysis_revision, sync_disposition
               FROM device_library_track_aliases
              WHERE source_id = ?1 AND archived = 0
              ORDER BY device_track_id",
        )?;
        let rows = statement.query_map([source_id], |row| {
            let raw_id = row.get::<_, i64>(0)?;
            let device_track_id = u32::try_from(raw_id)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, raw_id))?;
            let canonical_track_id = row
                .get::<_, Option<i64>>(1)?
                .map(|value| {
                    u64::try_from(value)
                        .map(TrackId::new)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, value))
                })
                .transpose()?;
            Ok(StoredDeviceAliasState {
                device_track_id,
                canonical_track_id,
                match_kind: row.get(2)?,
                metadata_revision: row.get(3)?,
                analysis_revision: row.get(4)?,
                sync_disposition: row.get(5)?,
            })
        })?;
        let mut states = BTreeMap::new();
        for row in rows {
            let state = row?;
            states.insert(state.device_track_id, state);
        }
        Ok(states)
    }

    pub fn device_selected_playlist_ids(
        &self,
        source_id: &str,
    ) -> Result<Vec<u32>, SqliteLibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT source_playlist_id FROM playlists WHERE source_id = ?1 ORDER BY id")?;
        let rows = statement.query_map([source_id], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            let value = row?;
            let Some(raw_id) = value
                .strip_prefix("onelibrary:")
                .or_else(|| value.strip_prefix("devicesql:"))
            else {
                continue;
            };
            if let Ok(id) = raw_id.parse::<u32>() {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    pub fn device_selected_playlist_paths(
        &self,
        source_id: &str,
    ) -> Result<Vec<String>, SqliteLibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT name FROM playlists WHERE source_id = ?1 ORDER BY name")?;
        let rows = statement.query_map([source_id], |row| row.get::<_, String>(0))?;
        let mut paths = rows.collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Returns every known device-specific playback location for a canonical
    /// track, newest trusted source first. The canonical track URI remains the
    /// durable identity URI; these locations allow playback to follow whichever
    /// synchronized USB is currently mounted.
    pub fn device_audio_uris(&self, track_id: TrackId) -> Result<Vec<String>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT l.audio_uri
               FROM device_track_audio_locations l
               JOIN device_library_sources s ON s.source_id = l.source_id
               JOIN device_library_track_aliases a
                 ON a.source_id = l.source_id
                AND a.device_track_id = l.device_track_id
              WHERE l.canonical_track_id = ?1 AND a.archived = 0
              ORDER BY s.synced_at DESC, l.source_id, l.device_track_id",
        )?;
        let rows = statement.query_map([to_i64(track_id.value())?], |row| row.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // The four synchronized collections form one atomic device snapshot. A
    // request object would only move these explicit transaction inputs around.
    #[allow(clippy::too_many_arguments)]
    pub fn sync_device_aliases(
        &mut self,
        source_id: &str,
        display_name: &str,
        database_revision: &str,
        aliases: &mut [DeviceAliasUpsert],
        new_tracks: &[DeviceTrackImport],
        analyses: &[DeviceAnalysisUpsert],
        hot_cue_updates: &[DeviceHotCueUpsert],
        playlists: &[DevicePlaylistUpsert],
    ) -> Result<(), SqliteLibraryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO device_library_sources(source_id, display_name, database_revision)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(source_id) DO UPDATE SET
               display_name = excluded.display_name,
               database_revision = excluded.database_revision,
               synced_at = CURRENT_TIMESTAMP",
            params![source_id, display_name, database_revision],
        )?;
        transaction.execute(
            "INSERT INTO library_sources(source_id, source_kind, display_name, source_revision)
             VALUES (?1, 'rekordbox-device', ?2, ?3)
             ON CONFLICT(source_id) DO UPDATE SET
               display_name = excluded.display_name,
               source_revision = excluded.source_revision",
            params![source_id, display_name, database_revision],
        )?;
        if source_id.starts_with("usb-fs:") {
            let legacy_source_ids = {
                let mut statement = transaction.prepare(
                    "SELECT source_id
                       FROM device_library_sources
                      WHERE source_id <> ?1
                        AND display_name = ?2 COLLATE NOCASE
                        AND (
                            (source_id LIKE 'usb-fs:%'
                             AND ?1 LIKE source_id || ':%')
                            OR
                            (source_id LIKE 'usb-fs:%'
                             AND instr(substr(source_id, 8), ':') = 0
                             AND ?1 LIKE 'usb-fs:hardware-%')
                            OR
                            (source_id LIKE 'usb-volume:%' AND database_revision = ?3)
                            OR
                            (source_id LIKE 'rekordbox-device:%'
                             AND database_revision IN (?3, 'reset-pending'))
                        )",
                )?;
                statement
                    .query_map(params![source_id, display_name, database_revision], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for legacy_source_id in legacy_source_ids {
                let playlist_ids = {
                    let mut statement =
                        transaction.prepare("SELECT id FROM playlists WHERE source_id = ?1")?;
                    statement
                        .query_map([&legacy_source_id], |row| row.get::<_, i64>(0))?
                        .collect::<Result<Vec<_>, _>>()?
                };
                for playlist_id in playlist_ids {
                    transaction.execute(
                        "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
                        [playlist_id],
                    )?;
                    transaction.execute("DELETE FROM playlists WHERE id = ?1", [playlist_id])?;
                }
                transaction.execute(
                    "UPDATE track_analysis_provenance SET source_id = ?1 WHERE source_id = ?2",
                    params![source_id, legacy_source_id],
                )?;
                transaction.execute(
                    "UPDATE track_hot_cue_provenance SET source_id = ?1 WHERE source_id = ?2",
                    params![source_id, legacy_source_id],
                )?;
                transaction.execute(
                    "UPDATE track_metadata_provenance SET source_id = ?1 WHERE source_id = ?2",
                    params![source_id, legacy_source_id],
                )?;
                transaction.execute(
                    "DELETE FROM device_library_track_aliases WHERE source_id = ?1",
                    [&legacy_source_id],
                )?;
                transaction.execute(
                    "DELETE FROM device_library_sources WHERE source_id = ?1",
                    [&legacy_source_id],
                )?;
                // A creative track preserved by Library Reset may still be
                // owned by the old provider-style source. Keep that otherwise
                // unused library source when necessary; the trusted USB
                // identity and all live aliases have already moved to the
                // stable filesystem source.
                transaction.execute(
                    "DELETE FROM library_sources
                      WHERE source_id = ?1
                        AND NOT EXISTS (
                            SELECT 1 FROM tracks WHERE source_id = ?1
                        )",
                    [&legacy_source_id],
                )?;
            }
        }
        for imported in new_tracks {
            let source_track_id = imported.analysis.source_track_id().as_str();
            let existing_track_id = transaction
                .query_row(
                    "SELECT id FROM tracks WHERE source_id = ?1 AND source_track_id = ?2",
                    params![source_id, source_track_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            let track_id = if let Some(track_id) = existing_track_id {
                track_id
            } else {
                insert_track(&transaction, source_id, &imported.analysis)?;
                let track_id = transaction.last_insert_rowid();
                Self::store_analysis(&transaction, track_id, &imported.analysis)?;
                track_id
            };
            let alias = aliases
                .iter_mut()
                .find(|alias| alias.device_track_id == imported.device_track_id)
                .ok_or_else(|| {
                    SqliteLibraryError::CorruptData(format!(
                        "device import {} is missing its alias",
                        imported.device_track_id
                    ))
                })?;
            alias.canonical_track_id = Some(TrackId::new(from_positive_i64(
                track_id,
                "imported device track id",
            )?));
            alias.match_kind = "imported-device".to_owned();
            alias.sync_disposition = "promoted-initial".to_owned();
            transaction.execute(
                "INSERT INTO track_analysis_provenance
                 (track_id, source_id, device_track_id, analysis_revision, analyzed_at,
                  hot_cues_loaded)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1)
                 ON CONFLICT(track_id) DO NOTHING",
                params![
                    track_id,
                    source_id,
                    i64::from(imported.device_track_id),
                    imported.source_analysis_revision,
                    imported.analyzed_at,
                ],
            )?;
            transaction.execute(
                "INSERT INTO track_hot_cue_provenance
                 (track_id, source_id, device_track_id, analysis_revision, analyzed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_id) DO NOTHING",
                params![
                    track_id,
                    source_id,
                    i64::from(imported.device_track_id),
                    imported.source_analysis_revision,
                    imported.analyzed_at,
                ],
            )?;
        }
        // Track color is metadata, not analysis. Keep an independent monotone
        // provenance lane so an older backup USB cannot undo a newer
        // Rekordbox color while information-only changes still resync.
        for alias in aliases.iter() {
            let Some(canonical_track_id) = alias.canonical_track_id else {
                continue;
            };
            let track_id = to_i64(canonical_track_id.value())?;
            let active = transaction
                .query_row(
                    "SELECT metadata_revision, analyzed_at, master_database_id,
                            master_content_id, information_update_count
                       FROM track_metadata_provenance WHERE track_id = ?1",
                    [track_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )
                .optional()?;
            let promotes = match active.as_ref() {
                None => true,
                Some((revision, _, _, _, _)) if revision == &alias.metadata_revision => false,
                Some((_, _, master_database_id, master_content_id, information_update_count))
                    if alias.master_database_id != 0
                        && alias.master_content_id != 0
                        && i64::from(alias.master_database_id) == *master_database_id
                        && i64::from(alias.master_content_id) == *master_content_id
                        && i64::from(alias.information_update_count)
                            != *information_update_count =>
                {
                    i64::from(alias.information_update_count) > *information_update_count
                }
                Some((_, analyzed_at, _, _, _)) => matches!(
                    compare_rekordbox_dates(&alias.analyzed_at, analyzed_at),
                    Some(std::cmp::Ordering::Greater)
                ),
            };
            if promotes {
                transaction.execute(
                    "UPDATE tracks SET color_rgb = ?1 WHERE id = ?2",
                    params![alias.color_rgb.map(i64::from), track_id],
                )?;
                transaction.execute(
                    "INSERT INTO track_metadata_provenance
                     (track_id, source_id, device_track_id, metadata_revision, analyzed_at,
                      master_database_id, master_content_id, information_update_count)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(track_id) DO UPDATE SET
                       source_id = excluded.source_id,
                       device_track_id = excluded.device_track_id,
                       metadata_revision = excluded.metadata_revision,
                       analyzed_at = excluded.analyzed_at,
                       master_database_id = excluded.master_database_id,
                       master_content_id = excluded.master_content_id,
                       information_update_count = excluded.information_update_count,
                       imported_at = CURRENT_TIMESTAMP",
                    params![
                        track_id,
                        source_id,
                        i64::from(alias.device_track_id),
                        alias.metadata_revision,
                        alias.analyzed_at,
                        i64::from(alias.master_database_id),
                        i64::from(alias.master_content_id),
                        i64::from(alias.information_update_count),
                    ],
                )?;
            }
        }
        transaction.execute(
            "UPDATE device_library_track_aliases SET archived = 1 WHERE source_id = ?1",
            [source_id],
        )?;
        let mut statement = transaction.prepare(
            "INSERT INTO device_library_track_aliases
             (source_id, device_track_id, simulator_signature, canonical_track_id, match_kind,
              title, artist, bpm_milli, duration_millis, file_size, metadata_revision,
              analysis_revision, audio_signature, analyzed_at, sync_disposition, archived)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0)
             ON CONFLICT(source_id, device_track_id) DO UPDATE SET
               simulator_signature = excluded.simulator_signature,
               canonical_track_id = excluded.canonical_track_id,
               match_kind = excluded.match_kind,
               title = excluded.title,
               artist = excluded.artist,
               bpm_milli = excluded.bpm_milli,
               duration_millis = excluded.duration_millis,
               file_size = excluded.file_size,
               metadata_revision = excluded.metadata_revision,
               analysis_revision = excluded.analysis_revision,
               audio_signature = excluded.audio_signature,
               analyzed_at = excluded.analyzed_at,
               sync_disposition = excluded.sync_disposition,
               archived = 0",
        )?;
        for alias in aliases.iter() {
            statement.execute(params![
                source_id,
                i64::from(alias.device_track_id),
                i64::from(alias.simulator_signature),
                alias
                    .canonical_track_id
                    .map(|track_id| i64::try_from(track_id.value()))
                    .transpose()
                    .map_err(|_| SqliteLibraryError::ArithmeticOverflow)?,
                alias.match_kind,
                alias.title,
                alias.artist,
                i64::from(alias.bpm_milli),
                to_i64(alias.duration_millis)?,
                i64::from(alias.file_size),
                alias.metadata_revision,
                alias.analysis_revision,
                alias.audio_signature,
                alias.analyzed_at,
                alias.sync_disposition,
            ])?;
        }
        drop(statement);

        let mut statement = transaction.prepare(
            "INSERT INTO device_track_audio_locations
             (source_id, device_track_id, canonical_track_id, audio_uri)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id, device_track_id) DO UPDATE SET
               canonical_track_id = excluded.canonical_track_id,
               audio_uri = excluded.audio_uri,
               updated_at = CURRENT_TIMESTAMP",
        )?;
        for alias in aliases.iter() {
            let Some(track_id) = alias.canonical_track_id else {
                continue;
            };
            statement.execute(params![
                source_id,
                i64::from(alias.device_track_id),
                to_i64(track_id.value())?,
                alias.audio_uri,
            ])?;
        }
        drop(statement);

        // macOS can assign a new filesystem UUID after a device repair or
        // reformat. Consolidate an older stable USB identity only when both
        // identities resolve to the exact same complete canonical track set.
        // A matching display name alone is never considered sufficient.
        if source_id.starts_with("usb-fs:") {
            consolidate_equivalent_usb_sources(&transaction, source_id, display_name)?;
        }
        let alias_tracks = aliases
            .iter()
            .filter_map(|alias| {
                alias
                    .canonical_track_id
                    .map(|track_id| (alias.device_track_id, track_id))
            })
            .collect::<BTreeMap<_, _>>();
        let existing_playlist_ids = {
            let mut statement =
                transaction.prepare("SELECT id FROM playlists WHERE source_id = ?1")?;
            statement
                .query_map([source_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for playlist_id in existing_playlist_ids {
            transaction.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist_id],
            )?;
            transaction.execute("DELETE FROM playlists WHERE id = ?1", [playlist_id])?;
        }
        for playlist in playlists {
            let source_playlist_id = format!("onelibrary:{}", playlist.device_playlist_id);
            transaction.execute(
                "INSERT INTO playlists(source_id, source_playlist_id, name)
                 VALUES (?1, ?2, ?3)",
                params![source_id, source_playlist_id, playlist.path],
            )?;
            let playlist_id = transaction.last_insert_rowid();
            let mut position = 0_i64;
            for device_track_id in &playlist.device_track_ids {
                let Some(track_id) = alias_tracks.get(device_track_id) else {
                    continue;
                };
                transaction.execute(
                    "INSERT INTO playlist_tracks(playlist_id, track_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![playlist_id, to_i64(track_id.value())?, position],
                )?;
                position = position
                    .checked_add(1)
                    .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
            }
        }
        for alias in aliases
            .iter()
            .filter(|alias| alias.sync_disposition == "current")
        {
            let Some(track_id) = alias.canonical_track_id else {
                continue;
            };
            transaction.execute(
                "INSERT INTO track_analysis_provenance
                 (track_id, source_id, device_track_id, analysis_revision, analyzed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_id) DO NOTHING",
                params![
                    to_i64(track_id.value())?,
                    source_id,
                    i64::from(alias.device_track_id),
                    alias.analysis_revision,
                    alias.analyzed_at,
                ],
            )?;
        }
        for analysis in analyses {
            replace_track_analysis_in_transaction(&transaction, analysis)?;
            transaction.execute(
                "INSERT INTO track_analysis_provenance
                 (track_id, source_id, device_track_id, analysis_revision, analyzed_at,
                  hot_cues_loaded)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1)
                 ON CONFLICT(track_id) DO UPDATE SET
                   source_id = excluded.source_id,
                   device_track_id = excluded.device_track_id,
                   analysis_revision = excluded.analysis_revision,
                   analyzed_at = excluded.analyzed_at,
                   hot_cues_loaded = 1,
                   promoted_at = CURRENT_TIMESTAMP",
                params![
                    to_i64(analysis.track_id.value())?,
                    analysis.source_id,
                    i64::from(analysis.device_track_id),
                    analysis.source_analysis_revision,
                    analysis.analyzed_at,
                ],
            )?;
            transaction.execute(
                "INSERT INTO track_hot_cue_provenance
                 (track_id, source_id, device_track_id, analysis_revision, analyzed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_id) DO UPDATE SET
                   source_id = excluded.source_id,
                   device_track_id = excluded.device_track_id,
                   analysis_revision = excluded.analysis_revision,
                   analyzed_at = excluded.analyzed_at,
                   imported_at = CURRENT_TIMESTAMP",
                params![
                    to_i64(analysis.track_id.value())?,
                    analysis.source_id,
                    i64::from(analysis.device_track_id),
                    analysis.source_analysis_revision,
                    analysis.analyzed_at,
                ],
            )?;
        }
        for update in hot_cue_updates {
            replace_hot_cues_in_transaction(&transaction, update.track_id, &update.hot_cues)?;
            transaction.execute(
                "INSERT INTO track_hot_cue_provenance
                 (track_id, source_id, device_track_id, analysis_revision, analyzed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_id) DO UPDATE SET
                   source_id = excluded.source_id,
                   device_track_id = excluded.device_track_id,
                   analysis_revision = excluded.analysis_revision,
                   analyzed_at = excluded.analyzed_at,
                   imported_at = CURRENT_TIMESTAMP",
                params![
                    to_i64(update.track_id.value())?,
                    update.source_id,
                    i64::from(update.device_track_id),
                    update.source_analysis_revision,
                    update.analyzed_at,
                ],
            )?;
        }
        // A previous USB sync could have imported a second canonical row when
        // the same track already existed under a provider source. Once the
        // alias and playlist have been repaired to that canonical identity,
        // remove only unreferenced device-owned rows that still contain no
        // user-authored timeline revision. Edited Lumi tracks are never
        // eligible for this cleanup.
        transaction.execute(
            "DELETE FROM tracks AS t
              WHERE t.source_id = ?1
                AND NOT EXISTS (
                    SELECT 1 FROM device_library_track_aliases a
                     WHERE a.canonical_track_id = t.id AND a.archived = 0
                )
                AND NOT EXISTS (
                    SELECT 1 FROM playlist_tracks pt WHERE pt.track_id = t.id
                )
                AND NOT EXISTS (
                    SELECT 1 FROM timeline_revisions r
                     WHERE r.track_id = t.id
                       AND (r.revision <> 1 OR r.origin <> 'source-import')
                )",
            [source_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Classifies an incoming device analysis without mutating the active
    /// track. A changed content revision from the same trusted USB is a newer
    /// export of that source and is promoted. Competing USB sources remain
    /// conservatively ordered by Rekordbox's export date.
    pub fn device_analysis_decision(
        &self,
        track_id: TrackId,
        source_id: &str,
        analysis_revision: &str,
        analyzed_at: &str,
    ) -> Result<DeviceAnalysisDecision, SqliteLibraryError> {
        let track_id_value = to_i64(track_id.value())?;
        let active_revision = self.connection.query_row(
            "SELECT analysis_revision FROM tracks WHERE id = ?1",
            [track_id_value],
            |row| row.get::<_, String>(0),
        )?;
        let provenance = self
            .connection
            .query_row(
                "SELECT source_id, analysis_revision, analyzed_at, hot_cues_loaded
               FROM track_analysis_provenance WHERE track_id = ?1",
                [track_id_value],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)? != 0,
                    ))
                },
            )
            .optional()?;

        if provenance
            .as_ref()
            .is_some_and(|(_, revision, _, loaded)| revision == analysis_revision && !loaded)
        {
            return Ok(DeviceAnalysisDecision::PromoteInitial);
        }
        if provenance
            .as_ref()
            .is_some_and(|(_, revision, _, loaded)| revision == analysis_revision && *loaded)
            || active_revision == format!("device:{source_id}:{analysis_revision}")
            || (active_revision.starts_with("device:")
                && active_revision.ends_with(analysis_revision))
        {
            return Ok(DeviceAnalysisDecision::Current);
        }
        let Some((active_source, _, active_date, _)) = provenance else {
            return Ok(if active_revision.starts_with("device:") {
                DeviceAnalysisDecision::HoldConflict
            } else {
                DeviceAnalysisDecision::PromoteInitial
            });
        };
        if active_source == source_id {
            return Ok(DeviceAnalysisDecision::PromoteNewer);
        }
        match compare_rekordbox_dates(analyzed_at, &active_date) {
            Some(std::cmp::Ordering::Greater) => Ok(DeviceAnalysisDecision::PromoteNewer),
            Some(std::cmp::Ordering::Less) => Ok(DeviceAnalysisDecision::ProtectOlder),
            Some(std::cmp::Ordering::Equal) | None => Ok(DeviceAnalysisDecision::HoldConflict),
        }
    }

    /// Hot cues are an independently replaceable Rekordbox projection. Their
    /// provenance must not force a beat-grid or waveform promotion: a trusted
    /// source can refresh its own cue revision, while an older backup source
    /// remains protected by the same monotone date rule.
    pub fn device_hot_cue_decision(
        &self,
        track_id: TrackId,
        source_id: &str,
        analysis_revision: &str,
        analyzed_at: &str,
    ) -> Result<DeviceAnalysisDecision, SqliteLibraryError> {
        let provenance = self
            .connection
            .query_row(
                "SELECT source_id, analysis_revision, analyzed_at
                   FROM track_hot_cue_provenance WHERE track_id = ?1",
                [to_i64(track_id.value())?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((active_source, active_revision, active_date)) = provenance else {
            return Ok(DeviceAnalysisDecision::PromoteInitial);
        };
        if active_revision == analysis_revision {
            return Ok(DeviceAnalysisDecision::Current);
        }
        if active_source == source_id {
            return Ok(DeviceAnalysisDecision::PromoteNewer);
        }
        match compare_rekordbox_dates(analyzed_at, &active_date) {
            Some(std::cmp::Ordering::Greater) => Ok(DeviceAnalysisDecision::PromoteNewer),
            Some(std::cmp::Ordering::Less) => Ok(DeviceAnalysisDecision::ProtectOlder),
            Some(std::cmp::Ordering::Equal) | None => Ok(DeviceAnalysisDecision::HoldConflict),
        }
    }

    pub fn resolve_device_alias(
        &self,
        device_track_id: u32,
        simulator_signature: u32,
    ) -> Result<Option<DeviceAliasResolution>, SqliteLibraryError> {
        let (predicate, value) = if simulator_signature == 0 {
            ("a.device_track_id = ?1", device_track_id)
        } else {
            ("a.simulator_signature = ?1", simulator_signature)
        };
        let sql = format!(
            "SELECT a.source_id, s.display_name, a.device_track_id, a.canonical_track_id,
                    a.analysis_revision, a.match_kind
               FROM device_library_track_aliases a
               JOIN device_library_sources s ON s.source_id = a.source_id
              WHERE {predicate} AND a.archived = 0 AND a.canonical_track_id IS NOT NULL
              ORDER BY s.synced_at DESC, a.source_id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([i64::from(value)], |row| {
            Ok(DeviceAliasResolution {
                source_id: row.get(0)?,
                display_name: row.get(1)?,
                device_track_id: u32::try_from(row.get::<_, i64>(2)?).map_err(|_| {
                    rusqlite::Error::IntegralValueOutOfRange(2, row.get::<_, i64>(2).unwrap_or(-1))
                })?,
                canonical_track_id: TrackId::new(u64::try_from(row.get::<_, i64>(3)?).map_err(
                    |_| {
                        rusqlite::Error::IntegralValueOutOfRange(
                            3,
                            row.get::<_, i64>(3).unwrap_or(-1),
                        )
                    },
                )?),
                analysis_revision: row.get(4)?,
                match_kind: row.get(5)?,
            })
        })?;
        let mut resolved = rows.collect::<Result<Vec<_>, _>>()?;
        let Some(newest) = resolved.first() else {
            return Ok(None);
        };
        // Backup USB media can legitimately expose the same Rekordbox track
        // identity. It is only ambiguous when those aliases disagree about
        // the canonical Lumi track; duplicate agreement is positive evidence.
        if resolved
            .iter()
            .any(|candidate| candidate.canonical_track_id != newest.canonical_track_id)
        {
            return Ok(None);
        }
        Ok(Some(resolved.remove(0)))
    }

    pub fn device_source_summaries(&self) -> Result<Vec<DeviceSourceSummary>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT s.source_id, s.display_name, s.database_revision,
                    COUNT(a.device_track_id), COUNT(a.canonical_track_id), s.synced_at,
                    COALESCE(SUM(CASE WHEN a.sync_disposition IN ('current', 'kept-active') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.sync_disposition LIKE 'promoted-%' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.sync_disposition = 'protected-older' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.sync_disposition = 'held-conflict' THEN 1 ELSE 0 END), 0)
               FROM device_library_sources s
               LEFT JOIN device_library_track_aliases a
                 ON a.source_id = s.source_id AND a.archived = 0
              GROUP BY s.source_id, s.display_name, s.database_revision, s.synced_at
              ORDER BY s.display_name, s.source_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DeviceSourceSummary {
                source_id: row.get(0)?,
                display_name: row.get(1)?,
                database_revision: row.get(2)?,
                active_tracks: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                matched_tracks: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                synced_at: row.get(5)?,
                current_tracks: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
                promoted_tracks: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
                protected_tracks: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
                conflict_tracks: u64::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn device_review_tracks(
        &self,
    ) -> Result<BTreeMap<String, Vec<DeviceReviewTrackSummary>>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT a.source_id, a.device_track_id, a.canonical_track_id,
                    a.title, a.artist, a.bpm_milli, a.duration_millis, a.analyzed_at,
                    a.analysis_revision, a.metadata_revision, a.file_size,
                    COALESCE(active_source.display_name, 'Lumi library'),
                    provenance.analyzed_at,
                    canonical.analysis_revision
               FROM device_library_track_aliases a
               LEFT JOIN tracks canonical
                 ON canonical.id = a.canonical_track_id
               LEFT JOIN track_analysis_provenance provenance
                 ON provenance.track_id = a.canonical_track_id
               LEFT JOIN device_library_sources active_source
                 ON active_source.source_id = provenance.source_id
              WHERE a.archived = 0 AND a.sync_disposition = 'held-conflict'
              ORDER BY a.source_id, a.title COLLATE NOCASE, a.artist COLLATE NOCASE,
                       a.device_track_id",
        )?;
        let rows = statement.query_map([], |row| {
            let raw_device_track_id = row.get::<_, i64>(1)?;
            let device_track_id = u32::try_from(raw_device_track_id)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, raw_device_track_id))?;
            let canonical_track_id = row
                .get::<_, Option<i64>>(2)?
                .map(|value| {
                    u64::try_from(value)
                        .map(TrackId::new)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(2, value))
                })
                .transpose()?;
            let raw_bpm = row.get::<_, i64>(5)?;
            let raw_duration = row.get::<_, i64>(6)?;
            let raw_file_size = row.get::<_, i64>(10)?;
            Ok(DeviceReviewTrackSummary {
                source_id: row.get(0)?,
                device_track_id,
                canonical_track_id,
                title: row.get(3)?,
                artist: row.get(4)?,
                bpm_milli: u32::try_from(raw_bpm)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(5, raw_bpm))?,
                duration_millis: u64::try_from(raw_duration)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, raw_duration))?,
                incoming_analyzed_at: row.get(7)?,
                incoming_analysis_revision: row.get(8)?,
                incoming_metadata_revision: row.get(9)?,
                incoming_file_size: u32::try_from(raw_file_size)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, raw_file_size))?,
                active_source_name: row.get(11)?,
                active_analyzed_at: row.get(12)?,
                active_analysis_revision: row.get(13)?,
            })
        })?;
        let mut review_tracks = BTreeMap::<String, Vec<DeviceReviewTrackSummary>>::new();
        for row in rows {
            let track = row?;
            review_tracks
                .entry(track.source_id.clone())
                .or_default()
                .push(track);
        }
        Ok(review_tracks)
    }

    /// Accepts the active Lumi projection for exactly the USB revision that
    /// was reviewed. A later USB analysis revision automatically reopens the
    /// conflict during the next sync.
    pub fn keep_active_device_analysis(
        &mut self,
        source_id: &str,
        device_track_id: u32,
        expected_incoming_revision: &str,
        expected_active_revision: &str,
    ) -> Result<(), SqliteLibraryError> {
        let transaction = self.connection.transaction()?;
        let (canonical_track_id, incoming_revision, disposition) = transaction.query_row(
            "SELECT canonical_track_id, analysis_revision, sync_disposition
                   FROM device_library_track_aliases
                  WHERE source_id = ?1 AND device_track_id = ?2 AND archived = 0",
            params![source_id, i64::from(device_track_id)],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?;
        let canonical_track_id = canonical_track_id.ok_or(SqliteLibraryError::MissingTrack)?;
        let active_revision = transaction.query_row(
            "SELECT analysis_revision FROM tracks WHERE id = ?1",
            [canonical_track_id],
            |row| row.get::<_, String>(0),
        )?;
        if incoming_revision != expected_incoming_revision
            || active_revision != expected_active_revision
        {
            return Err(SqliteLibraryError::DeviceReviewChanged);
        }
        if disposition != "held-conflict" {
            return Err(SqliteLibraryError::DeviceReviewChanged);
        }
        transaction.execute(
            "UPDATE device_library_track_aliases
                SET sync_disposition = 'kept-active'
              WHERE source_id = ?1 AND device_track_id = ?2 AND archived = 0",
            params![source_id, i64::from(device_track_id)],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Atomically replaces the imported Rekordbox projection after an explicit
    /// review choice. Lumi's authored phrase timeline and AutoLoop choices live
    /// in separate tables and are deliberately preserved.
    pub fn promote_reviewed_device_analysis(
        &mut self,
        analysis: &DeviceAnalysisUpsert,
        expected_active_revision: &str,
    ) -> Result<(), SqliteLibraryError> {
        let transaction = self.connection.transaction()?;
        let incoming = transaction.query_row(
            "SELECT analysis_revision, sync_disposition
               FROM device_library_track_aliases
              WHERE source_id = ?1 AND device_track_id = ?2 AND archived = 0
                AND canonical_track_id = ?3",
            params![
                analysis.source_id,
                i64::from(analysis.device_track_id),
                to_i64(analysis.track_id.value())?,
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;
        let active_revision = transaction.query_row(
            "SELECT analysis_revision FROM tracks WHERE id = ?1",
            [to_i64(analysis.track_id.value())?],
            |row| row.get::<_, String>(0),
        )?;
        if incoming.0 != analysis.source_analysis_revision
            || incoming.1 != "held-conflict"
            || active_revision != expected_active_revision
        {
            return Err(SqliteLibraryError::DeviceReviewChanged);
        }
        replace_track_analysis_in_transaction(&transaction, analysis)?;
        transaction.execute(
            "INSERT INTO track_analysis_provenance
             (track_id, source_id, device_track_id, analysis_revision, analyzed_at, hot_cues_loaded)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(track_id) DO UPDATE SET
               source_id = excluded.source_id,
               device_track_id = excluded.device_track_id,
               analysis_revision = excluded.analysis_revision,
               analyzed_at = excluded.analyzed_at,
               hot_cues_loaded = 1,
               promoted_at = CURRENT_TIMESTAMP",
            params![
                to_i64(analysis.track_id.value())?,
                analysis.source_id,
                i64::from(analysis.device_track_id),
                analysis.source_analysis_revision,
                analysis.analyzed_at,
            ],
        )?;
        transaction.execute(
            "INSERT INTO track_hot_cue_provenance
             (track_id, source_id, device_track_id, analysis_revision, analyzed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(track_id) DO UPDATE SET
               source_id = excluded.source_id,
               device_track_id = excluded.device_track_id,
               analysis_revision = excluded.analysis_revision,
               analyzed_at = excluded.analyzed_at,
               imported_at = CURRENT_TIMESTAMP",
            params![
                to_i64(analysis.track_id.value())?,
                analysis.source_id,
                i64::from(analysis.device_track_id),
                analysis.source_analysis_revision,
                analysis.analyzed_at,
            ],
        )?;
        transaction.execute(
            "UPDATE device_library_track_aliases
                SET sync_disposition = 'promoted-reviewed'
              WHERE source_id = ?1 AND device_track_id = ?2 AND archived = 0",
            params![analysis.source_id, i64::from(analysis.device_track_id)],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn device_track_source_relations(
        &self,
        track_ids: &[TrackId],
    ) -> Result<BTreeMap<TrackId, Vec<DeviceTrackSourceRelation>>, SqliteLibraryError> {
        let requested = track_ids.iter().copied().collect::<BTreeSet<_>>();
        if requested.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut statement = self.connection.prepare(
            "SELECT a.canonical_track_id, a.source_id, s.display_name, a.sync_disposition
               FROM device_library_track_aliases a
               JOIN device_library_sources s ON s.source_id = a.source_id
              WHERE a.archived = 0 AND a.canonical_track_id IS NOT NULL
              ORDER BY s.display_name, a.source_id",
        )?;
        let rows = statement.query_map([], |row| {
            let raw_track_id = row.get::<_, i64>(0)?;
            let track_id = TrackId::new(
                u64::try_from(raw_track_id)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, raw_track_id))?,
            );
            Ok(DeviceTrackSourceRelation {
                track_id,
                source_id: row.get(1)?,
                display_name: row.get(2)?,
                sync_disposition: row.get(3)?,
            })
        })?;
        let mut relations = BTreeMap::<TrackId, Vec<DeviceTrackSourceRelation>>::new();
        for relation in rows {
            let relation = relation?;
            if requested.contains(&relation.track_id) {
                relations
                    .entry(relation.track_id)
                    .or_default()
                    .push(relation);
            }
        }
        Ok(relations)
    }

    pub fn data_summary(&self) -> Result<LibraryDataSummary, SqliteLibraryError> {
        Ok(LibraryDataSummary {
            track_count: self.count_rows("tracks")?,
            playlist_count: self.count_rows("playlists")?,
            user_edited_track_count: from_nonnegative_i64(
                self.connection.query_row(
                    "SELECT COUNT(DISTINCT track_id) FROM timeline_revisions
                      WHERE origin IN ('user-edit', 'revision-restore')",
                    [],
                    |row| row.get(0),
                )?,
                "user-edited track count",
            )?,
            creative_archive_count: self.count_rows("creative_track_archives")?,
            pending_archive_count: from_nonnegative_i64(
                self.connection.query_row(
                    "SELECT COUNT(*) FROM creative_track_archives
                      WHERE state IN ('pending', 'review')",
                    [],
                    |row| row.get(0),
                )?,
                "pending creative archive count",
            )?,
        })
    }

    pub fn track_color_summaries(&self) -> Result<Vec<TrackColorSummary>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT t.color_rgb, COUNT(*)
               FROM tracks t
              WHERE t.color_rgb IS NOT NULL
                AND EXISTS (
                    SELECT 1
                      FROM track_metadata_provenance p
                     WHERE p.track_id = t.id
                )
              GROUP BY t.color_rgb
              ORDER BY t.color_rgb",
        )?;
        statement
            .query_map([], |row| {
                let color = row.get::<_, i64>(0)?;
                let count = row.get::<_, i64>(1)?;
                Ok((color, count))
            })?
            .map(|row| {
                let (color, count) = row?;
                Ok(TrackColorSummary {
                    color_rgb: u32::try_from(color).map_err(|_| {
                        SqliteLibraryError::CorruptData("invalid track color RGB".to_owned())
                    })?,
                    track_count: from_nonnegative_i64(count, "track color count")?,
                })
            })
            .collect()
    }

    pub fn reset_preservable_tracks(
        &self,
    ) -> Result<Vec<ResetPreservableTrackSummary>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.title, t.artist, h.revision
               FROM tracks t
               JOIN timeline_heads h ON h.track_id = t.id
              WHERE EXISTS (
                    SELECT 1 FROM timeline_revisions r
                     WHERE r.track_id = t.id
                       AND r.origin IN ('user-edit', 'revision-restore')
              )
              ORDER BY t.title COLLATE NOCASE, t.artist COLLATE NOCASE, t.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut tracks = Vec::new();
        for row in rows {
            let (track_id, title, artist, revision) = row?;
            tracks.push(ResetPreservableTrackSummary {
                track_id: TrackId::new(from_positive_i64(track_id, "track id")?),
                title,
                artist,
                timeline_revision: from_positive_i64(revision, "timeline revision")?,
            });
        }
        Ok(tracks)
    }

    pub fn creative_archives(&self) -> Result<Vec<CreativeArchiveSummary>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT a.archive_id, a.title, a.artist, COUNT(p.phrase_index),
                    a.total_beats, a.state, a.restored_track_id
               FROM creative_track_archives a
               LEFT JOIN creative_phrase_points p ON p.archive_id = a.archive_id
              GROUP BY a.archive_id, a.title, a.artist, a.total_beats,
                       a.state, a.restored_track_id
              ORDER BY a.updated_at DESC, a.archive_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?;
        let mut archives = Vec::new();
        for row in rows {
            let (archive_id, title, artist, phrase_count, total_beats, state, restored_track_id) =
                row?;
            archives.push(CreativeArchiveSummary {
                archive_id: from_positive_i64(archive_id, "creative archive id")?,
                title,
                artist,
                phrase_count: from_nonnegative_i64(phrase_count, "archive phrase count")?,
                total_beats: i64_to_u32(total_beats, "archive total beats")?,
                state,
                restored_track_id: restored_track_id
                    .map(|value| from_positive_i64(value, "restored track id").map(TrackId::new))
                    .transpose()?,
            });
        }
        Ok(archives)
    }

    pub fn preview_library_reset(
        &self,
        preserve_track_ids: &[TrackId],
    ) -> Result<LibraryResetImpact, SqliteLibraryError> {
        let track_count = self.count_rows("tracks")?;
        let playlist_count = self.count_rows("playlists")?;
        let mut unique = preserve_track_ids.iter().copied().collect::<BTreeSet<_>>();
        unique.retain(|track_id| {
            self.connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?1)",
                    [i64::try_from(track_id.value()).unwrap_or(i64::MAX)],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false)
        });
        let archived_creative_track_count = from_nonnegative_i64(
            self.connection.query_row(
                "SELECT COUNT(DISTINCT track_id) FROM timeline_revisions
                  WHERE origin IN ('user-edit', 'revision-restore')",
                [],
                |row| row.get(0),
            )?,
            "creative archive preview count",
        )?;
        let preserved_track_count =
            u64::try_from(unique.len()).map_err(|_| SqliteLibraryError::ArithmeticOverflow)?;
        Ok(LibraryResetImpact {
            track_count,
            playlist_count,
            preserved_track_count,
            removed_track_count: track_count.saturating_sub(preserved_track_count),
            archived_creative_track_count,
        })
    }

    pub fn reset_library_content(
        &mut self,
        preserve_track_ids: &[TrackId],
    ) -> Result<LibraryResetImpact, SqliteLibraryError> {
        let impact = self.preview_library_reset(preserve_track_ids)?;
        let transaction = self.connection.transaction()?;
        transaction.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS lumi_reset_preserved_tracks(
                track_id INTEGER PRIMARY KEY
             );
             DELETE FROM lumi_reset_preserved_tracks;",
        )?;
        for track_id in preserve_track_ids.iter().copied().collect::<BTreeSet<_>>() {
            transaction.execute(
                "INSERT OR IGNORE INTO lumi_reset_preserved_tracks(track_id)
                 SELECT id FROM tracks WHERE id = ?1",
                [to_i64(track_id.value())?],
            )?;
        }

        archive_user_creative_work(&transaction)?;
        transaction.execute(
            "UPDATE creative_track_archives
                SET state = CASE
                    WHEN original_track_id IN (SELECT track_id FROM lumi_reset_preserved_tracks)
                        THEN 'preserved'
                    ELSE 'pending'
                END,
                    restored_track_id = CASE
                    WHEN original_track_id IN (SELECT track_id FROM lumi_reset_preserved_tracks)
                        THEN original_track_id
                    ELSE NULL
                END,
                    updated_at = CURRENT_TIMESTAMP",
            [],
        )?;

        transaction.execute("DELETE FROM playlist_tracks", [])?;
        transaction.execute("DELETE FROM playlists", [])?;
        transaction.execute("DELETE FROM source_mirror_playlist_tracks", [])?;
        transaction.execute("DELETE FROM source_mirror_playlists", [])?;
        transaction.execute("DELETE FROM source_mirror_tracks", [])?;
        transaction.execute("DELETE FROM source_mirror_revisions", [])?;
        transaction.execute("DELETE FROM source_mirrors", [])?;
        transaction.execute("DELETE FROM import_baselines", [])?;
        // `timeline_heads` points both at the track and its current revision.
        // Removing the head explicitly avoids an ambiguous cascade order when
        // SQLite deletes a track and all of its revision history together.
        transaction.execute(
            "DELETE FROM timeline_heads
              WHERE track_id NOT IN (SELECT track_id FROM lumi_reset_preserved_tracks)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM tracks
              WHERE id NOT IN (SELECT track_id FROM lumi_reset_preserved_tracks)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM device_library_track_aliases
              WHERE canonical_track_id IS NULL
                 OR canonical_track_id NOT IN (SELECT track_id FROM lumi_reset_preserved_tracks)",
            [],
        )?;
        transaction.execute(
            "UPDATE device_library_sources
                SET database_revision = 'reset-pending', synced_at = CURRENT_TIMESTAMP",
            [],
        )?;
        transaction.execute(
            "INSERT INTO library_settings(key, value) VALUES ('suppress-demo-seed', 1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )?;
        transaction.execute_batch("DROP TABLE lumi_reset_preserved_tracks;")?;
        transaction.commit()?;
        Ok(impact)
    }

    pub fn suppress_demo_seed(&self) -> Result<bool, SqliteLibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM library_settings WHERE key = 'suppress-demo-seed'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0)
            != 0)
    }

    pub fn relink_creative_archives(&mut self) -> Result<CreativeRelinkResult, SqliteLibraryError> {
        let archive_ids = {
            let mut statement = self.connection.prepare(
                "SELECT archive_id FROM creative_track_archives
                  WHERE state IN ('pending', 'review') ORDER BY archive_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut result = CreativeRelinkResult::default();
        for archive_id in archive_ids {
            match self.relink_creative_archive(archive_id)? {
                CreativeArchiveRelinkState::Restored => result.restored += 1,
                CreativeArchiveRelinkState::Review => result.pending_review += 1,
                CreativeArchiveRelinkState::Pending => {}
            }
        }
        Ok(result)
    }

    fn relink_creative_archive(
        &mut self,
        archive_id: i64,
    ) -> Result<CreativeArchiveRelinkState, SqliteLibraryError> {
        let archive = self.connection.query_row(
            "SELECT title, artist, bpm_milli, duration_millis, audio_signature, total_beats
               FROM creative_track_archives WHERE archive_id = ?1",
            [archive_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )?;
        let (title, artist, bpm_milli, duration_millis, audio_signature, total_beats) = archive;
        let mut candidates = if audio_signature.is_empty() {
            Vec::new()
        } else {
            let mut statement = self.connection.prepare(
                "SELECT DISTINCT canonical_track_id
                   FROM device_library_track_aliases
                  WHERE archived = 0 AND audio_signature = ?1
                    AND canonical_track_id IS NOT NULL",
            )?;
            statement
                .query_map([&audio_signature], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        if candidates.is_empty() {
            let mut statement = self.connection.prepare(
                "SELECT id FROM tracks
                  WHERE lower(trim(title)) = lower(trim(?1))
                    AND lower(trim(artist)) = lower(trim(?2))
                    AND abs(bpm_milli - ?3) <= 10
                    AND abs(duration_millis - ?4) <= 1000",
            )?;
            candidates = statement
                .query_map(params![title, artist, bpm_milli, duration_millis], |row| {
                    row.get::<_, i64>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        candidates.sort_unstable();
        candidates.dedup();
        if candidates.len() != 1 {
            return Ok(CreativeArchiveRelinkState::Pending);
        }
        let track_id = candidates[0];
        let (head_revision, target_total_beats) = self.connection.query_row(
            "SELECT h.revision, r.total_beats
               FROM timeline_heads h
               JOIN timeline_revisions r
                 ON r.track_id = h.track_id AND r.revision = h.revision
              WHERE h.track_id = ?1",
            [track_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        if target_total_beats != total_beats {
            self.connection.execute(
                "UPDATE creative_track_archives SET state = 'review', updated_at = CURRENT_TIMESTAMP
                  WHERE archive_id = ?1",
                [archive_id],
            )?;
            return Ok(CreativeArchiveRelinkState::Review);
        }
        let transaction = self.connection.transaction()?;
        let next_revision = head_revision
            .checked_add(1)
            .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        let baseline_revision: String = transaction.query_row(
            "SELECT analysis_revision FROM tracks WHERE id = ?1",
            [track_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO timeline_revisions
             (track_id, revision, baseline_revision, total_beats, origin, reason,
              parent_revision, restored_from_revision)
             VALUES (?1, ?2, ?3, ?4, 'revision-restore', 'restore-revision', ?5, NULL)",
            params![
                track_id,
                next_revision,
                baseline_revision,
                total_beats,
                head_revision
            ],
        )?;
        transaction.execute(
            "INSERT INTO phrase_points
             (track_id, revision, phrase_index, beat, role_id, loop_strategy)
             SELECT ?1, ?2, phrase_index, beat, role_id, loop_strategy
               FROM creative_phrase_points WHERE archive_id = ?3 ORDER BY phrase_index",
            params![track_id, next_revision, archive_id],
        )?;
        transaction.execute(
            "UPDATE timeline_heads SET revision = ?1 WHERE track_id = ?2",
            params![next_revision, track_id],
        )?;
        transaction.execute(
            "UPDATE creative_track_archives
                SET state = 'restored', restored_track_id = ?1, updated_at = CURRENT_TIMESTAMP
              WHERE archive_id = ?2",
            params![track_id, archive_id],
        )?;
        transaction.commit()?;
        Ok(CreativeArchiveRelinkState::Restored)
    }

    fn count_rows(&self, table: &str) -> Result<u64, SqliteLibraryError> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        from_nonnegative_i64(
            self.connection.query_row(&sql, [], |row| row.get(0))?,
            "table row count",
        )
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
                CREATE TABLE hot_cues (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    cue_index INTEGER NOT NULL,
                    time_millis INTEGER NOT NULL,
                    loop_end_millis INTEGER,
                    name TEXT NOT NULL,
                    color_rgb INTEGER NOT NULL,
                    PRIMARY KEY(track_id, cue_index)
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
                           ('autoloop-catalog-revision', 0),
                           ('suppress-demo-seed', 0);
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
                    total_beats INTEGER NOT NULL,
                    origin TEXT NOT NULL,
                    reason TEXT NOT NULL,
                    parent_revision INTEGER,
                    restored_from_revision INTEGER,
                    PRIMARY KEY(track_id, revision)
                );
                CREATE TABLE phrase_points (
                    track_id INTEGER NOT NULL,
                    revision INTEGER NOT NULL,
                    phrase_index INTEGER NOT NULL,
                    beat INTEGER NOT NULL,
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
                CREATE TABLE source_mirrors (
                    source_id TEXT PRIMARY KEY,
                    source_kind TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    source_revision TEXT NOT NULL
                );
                CREATE TABLE source_mirror_tracks (
                    source_id TEXT NOT NULL REFERENCES source_mirrors(source_id),
                    source_track_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT,
                    average_bpm TEXT,
                    musical_key TEXT,
                    duration_millis INTEGER,
                    color TEXT,
                    audio_uri TEXT NOT NULL,
                    archived INTEGER NOT NULL,
                    last_seen_revision TEXT NOT NULL,
                    PRIMARY KEY(source_id, source_track_id)
                );
                CREATE INDEX source_mirror_tracks_active
                    ON source_mirror_tracks(source_id, archived, source_track_id);
                CREATE TABLE source_mirror_playlists (
                    source_id TEXT NOT NULL REFERENCES source_mirrors(source_id),
                    source_playlist_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    PRIMARY KEY(source_id, source_playlist_id)
                );
                CREATE TABLE source_mirror_playlist_tracks (
                    source_id TEXT NOT NULL,
                    source_playlist_id TEXT NOT NULL,
                    source_track_id TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    PRIMARY KEY(source_id, source_playlist_id, position),
                    UNIQUE(source_id, source_playlist_id, source_track_id),
                    FOREIGN KEY(source_id, source_playlist_id)
                        REFERENCES source_mirror_playlists(source_id, source_playlist_id)
                        ON DELETE CASCADE,
                    FOREIGN KEY(source_id, source_track_id)
                        REFERENCES source_mirror_tracks(source_id, source_track_id)
                );
                CREATE TABLE source_mirror_revisions (
                    source_id TEXT NOT NULL,
                    source_revision TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    active_track_count INTEGER NOT NULL,
                    archived_track_count INTEGER NOT NULL,
                    playlist_count INTEGER NOT NULL,
                    PRIMARY KEY(source_id, source_revision)
                );
                CREATE TABLE device_library_sources (
                    source_id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    database_revision TEXT NOT NULL,
                    synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE device_library_track_aliases (
                    source_id TEXT NOT NULL REFERENCES device_library_sources(source_id),
                    device_track_id INTEGER NOT NULL,
                    simulator_signature INTEGER NOT NULL,
                    canonical_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
                    match_kind TEXT NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    bpm_milli INTEGER NOT NULL,
                    duration_millis INTEGER NOT NULL,
                    file_size INTEGER NOT NULL,
                    metadata_revision TEXT NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    audio_signature TEXT NOT NULL DEFAULT '',
                    analyzed_at TEXT NOT NULL,
                    sync_disposition TEXT NOT NULL,
                    archived INTEGER NOT NULL,
                    PRIMARY KEY(source_id, device_track_id)
                );
                CREATE INDEX device_alias_by_simulator_signature
                    ON device_library_track_aliases(simulator_signature, archived);
                CREATE INDEX device_alias_by_canonical_track
                    ON device_library_track_aliases(canonical_track_id, archived);
                CREATE TABLE track_analysis_provenance (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    analyzed_at TEXT NOT NULL,
                    hot_cues_loaded INTEGER NOT NULL DEFAULT 0,
                    promoted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE track_hot_cue_provenance (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    analyzed_at TEXT NOT NULL,
                    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE creative_track_archives (
                    archive_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    identity_key TEXT NOT NULL UNIQUE,
                    original_track_id INTEGER,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    bpm_milli INTEGER NOT NULL,
                    duration_millis INTEGER NOT NULL,
                    audio_signature TEXT NOT NULL DEFAULT '',
                    total_beats INTEGER NOT NULL,
                    source_timeline_revision INTEGER NOT NULL,
                    state TEXT NOT NULL DEFAULT 'pending',
                    restored_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX creative_archives_state
                    ON creative_track_archives(state, archive_id);
                CREATE TABLE creative_phrase_points (
                    archive_id INTEGER NOT NULL REFERENCES creative_track_archives(archive_id)
                        ON DELETE CASCADE,
                    phrase_index INTEGER NOT NULL,
                    beat INTEGER NOT NULL,
                    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                    loop_strategy TEXT NOT NULL,
                    PRIMARY KEY(archive_id, phrase_index)
                );
                CREATE TABLE light_planning_policy (
                    singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
                    revision INTEGER NOT NULL,
                    policy_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 12;
                COMMIT;
                ",
            )?;
            current = 12;
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
            current = 4;
        }
        if current == 4 {
            if self.table_exists("timeline_revisions")?
                && self.table_exists("phrase_instances")?
                && self.table_exists("beat_grids")?
            {
                self.connection.execute_batch(
                    "
                    BEGIN IMMEDIATE;
                    UPDATE timeline_revisions
                       SET total_bars = total_bars * COALESCE(
                           (SELECT beats_per_bar FROM beat_grids
                             WHERE beat_grids.track_id = timeline_revisions.track_id),
                           4
                       );
                    UPDATE phrase_instances
                       SET start_bar = start_bar * COALESCE(
                           (SELECT beats_per_bar FROM beat_grids
                             WHERE beat_grids.track_id = phrase_instances.track_id),
                           4
                       );
                    ALTER TABLE timeline_revisions RENAME COLUMN total_bars TO total_beats;
                    ALTER TABLE phrase_instances RENAME TO phrase_points;
                    ALTER TABLE phrase_points RENAME COLUMN start_bar TO beat;
                    ALTER TABLE phrase_points DROP COLUMN end_bar;
                    PRAGMA user_version = 5;
                    COMMIT;
                    ",
                )?;
            } else {
                self.connection.execute_batch("PRAGMA user_version = 5;")?;
            }
            current = 5;
        }
        if current == 5 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE source_mirrors (
                    source_id TEXT PRIMARY KEY,
                    source_kind TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    source_revision TEXT NOT NULL
                );
                CREATE TABLE source_mirror_tracks (
                    source_id TEXT NOT NULL REFERENCES source_mirrors(source_id),
                    source_track_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT,
                    average_bpm TEXT,
                    musical_key TEXT,
                    duration_millis INTEGER,
                    color TEXT,
                    audio_uri TEXT NOT NULL,
                    archived INTEGER NOT NULL,
                    last_seen_revision TEXT NOT NULL,
                    PRIMARY KEY(source_id, source_track_id)
                );
                CREATE INDEX source_mirror_tracks_active
                    ON source_mirror_tracks(source_id, archived, source_track_id);
                CREATE TABLE source_mirror_playlists (
                    source_id TEXT NOT NULL REFERENCES source_mirrors(source_id),
                    source_playlist_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    PRIMARY KEY(source_id, source_playlist_id)
                );
                CREATE TABLE source_mirror_playlist_tracks (
                    source_id TEXT NOT NULL,
                    source_playlist_id TEXT NOT NULL,
                    source_track_id TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    PRIMARY KEY(source_id, source_playlist_id, position),
                    UNIQUE(source_id, source_playlist_id, source_track_id),
                    FOREIGN KEY(source_id, source_playlist_id)
                        REFERENCES source_mirror_playlists(source_id, source_playlist_id)
                        ON DELETE CASCADE,
                    FOREIGN KEY(source_id, source_track_id)
                        REFERENCES source_mirror_tracks(source_id, source_track_id)
                );
                CREATE TABLE source_mirror_revisions (
                    source_id TEXT NOT NULL,
                    source_revision TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    active_track_count INTEGER NOT NULL,
                    archived_track_count INTEGER NOT NULL,
                    playlist_count INTEGER NOT NULL,
                    PRIMARY KEY(source_id, source_revision)
                );
                PRAGMA user_version = 6;
                COMMIT;
                ",
            )?;
            current = 6;
        }
        if current == 6 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE device_library_sources (
                    source_id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    database_revision TEXT NOT NULL,
                    synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE device_library_track_aliases (
                    source_id TEXT NOT NULL REFERENCES device_library_sources(source_id),
                    device_track_id INTEGER NOT NULL,
                    simulator_signature INTEGER NOT NULL,
                    canonical_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
                    match_kind TEXT NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    bpm_milli INTEGER NOT NULL,
                    duration_millis INTEGER NOT NULL,
                    file_size INTEGER NOT NULL,
                    metadata_revision TEXT NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    archived INTEGER NOT NULL,
                    PRIMARY KEY(source_id, device_track_id)
                );
                CREATE INDEX device_alias_by_simulator_signature
                    ON device_library_track_aliases(simulator_signature, archived);
                CREATE INDEX device_alias_by_canonical_track
                    ON device_library_track_aliases(canonical_track_id, archived);
                PRAGMA user_version = 7;
                COMMIT;
                ",
            )?;
            current = 7;
        }
        if current == 7 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                ALTER TABLE device_library_track_aliases
                    ADD COLUMN analyzed_at TEXT NOT NULL DEFAULT '';
                ALTER TABLE device_library_track_aliases
                    ADD COLUMN sync_disposition TEXT NOT NULL DEFAULT 'current';
                CREATE TABLE track_analysis_provenance (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    analyzed_at TEXT NOT NULL,
                    promoted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 8;
                COMMIT;
                ",
            )?;
            current = 8;
        }
        if current == 8 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                ALTER TABLE device_library_track_aliases
                    ADD COLUMN audio_signature TEXT NOT NULL DEFAULT '';
                INSERT INTO library_settings(key, value) VALUES ('suppress-demo-seed', 0)
                    ON CONFLICT(key) DO NOTHING;
                CREATE TABLE creative_track_archives (
                    archive_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    identity_key TEXT NOT NULL UNIQUE,
                    original_track_id INTEGER,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    bpm_milli INTEGER NOT NULL,
                    duration_millis INTEGER NOT NULL,
                    audio_signature TEXT NOT NULL DEFAULT '',
                    total_beats INTEGER NOT NULL,
                    source_timeline_revision INTEGER NOT NULL,
                    state TEXT NOT NULL DEFAULT 'pending',
                    restored_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX creative_archives_state
                    ON creative_track_archives(state, archive_id);
                CREATE TABLE creative_phrase_points (
                    archive_id INTEGER NOT NULL REFERENCES creative_track_archives(archive_id)
                        ON DELETE CASCADE,
                    phrase_index INTEGER NOT NULL,
                    beat INTEGER NOT NULL,
                    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                    loop_strategy TEXT NOT NULL,
                    PRIMARY KEY(archive_id, phrase_index)
                );
                PRAGMA user_version = 9;
                COMMIT;
                ",
            )?;
            current = 9;
        }
        if current == 9 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE hot_cues (
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    cue_index INTEGER NOT NULL,
                    time_millis INTEGER NOT NULL,
                    loop_end_millis INTEGER,
                    name TEXT NOT NULL,
                    color_rgb INTEGER NOT NULL,
                    PRIMARY KEY(track_id, cue_index)
                );
                ALTER TABLE track_analysis_provenance
                    ADD COLUMN hot_cues_loaded INTEGER NOT NULL DEFAULT 0;
                PRAGMA user_version = 10;
                COMMIT;
                ",
            )?;
            current = 10;
        }
        if current == 10 {
            let can_seed_hot_cue_provenance =
                self.table_exists("tracks")? && self.table_exists("track_analysis_provenance")?;
            self.connection
                .execute_batch(if can_seed_hot_cue_provenance {
                    "
                BEGIN IMMEDIATE;
                CREATE TABLE track_hot_cue_provenance (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    analyzed_at TEXT NOT NULL,
                    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO track_hot_cue_provenance
                    (track_id, source_id, device_track_id, analysis_revision, analyzed_at)
                SELECT track_id, source_id, device_track_id, analysis_revision, analyzed_at
                  FROM track_analysis_provenance
                 WHERE hot_cues_loaded = 1;
                PRAGMA user_version = 11;
                COMMIT;
                "
                } else {
                    "
                BEGIN IMMEDIATE;
                CREATE TABLE track_hot_cue_provenance (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    analysis_revision TEXT NOT NULL,
                    analyzed_at TEXT NOT NULL,
                    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 11;
                COMMIT;
                "
                })?;
            current = 11;
        }
        if current == 11 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE light_planning_policy (
                    singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
                    revision INTEGER NOT NULL,
                    policy_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 12;
                COMMIT;
                ",
            )?;
            current = 12;
        }
        if current == 12 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE track_metadata_provenance (
                    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    metadata_revision TEXT NOT NULL,
                    analyzed_at TEXT NOT NULL,
                    master_database_id INTEGER NOT NULL,
                    master_content_id INTEGER NOT NULL,
                    information_update_count INTEGER NOT NULL,
                    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 13;
                COMMIT;
                ",
            )?;
            current = 13;
        }
        if current == 13 {
            self.connection.execute_batch(
                "
                BEGIN IMMEDIATE;
                CREATE TABLE device_track_audio_locations (
                    source_id TEXT NOT NULL,
                    device_track_id INTEGER NOT NULL,
                    canonical_track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    audio_uri TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY(source_id, device_track_id),
                    FOREIGN KEY(source_id, device_track_id)
                        REFERENCES device_library_track_aliases(source_id, device_track_id)
                        ON DELETE CASCADE
                );
                CREATE INDEX device_audio_by_canonical_track
                    ON device_track_audio_locations(canonical_track_id, source_id);
                PRAGMA user_version = 14;
                COMMIT;
                ",
            )?;
            current = 14;
        }
        if current == 14 {
            self.connection
                .execute_batch(if self.table_exists("phrase_roles")? {
                    "
                BEGIN IMMEDIATE;
                ALTER TABLE phrase_roles
                    ADD COLUMN color_rgb INTEGER NOT NULL DEFAULT 3386777
                    CHECK(color_rgb BETWEEN 0 AND 16777215);
                UPDATE phrase_roles SET color_rgb = CASE role_id
                    WHEN 'intro-outro' THEN 4230386
                    WHEN 'bridge' THEN 6187975
                    WHEN 'breakdown-1' THEN 8013780
                    WHEN 'breakdown-2' THEN 8013780
                    WHEN 'breakdown-3' THEN 8013780
                    WHEN 'synth' THEN 13712824
                    WHEN 'pre-drop' THEN 15889459
                    WHEN 'buildup-1' THEN 16099359
                    WHEN 'buildup-2' THEN 16099359
                    WHEN 'buildup-3' THEN 16099359
                    WHEN 'drop' THEN 15414082
                    ELSE 3386777
                END;
                PRAGMA user_version = 15;
                COMMIT;
                "
                } else {
                    // Some deliberately minimal legacy fixtures contain only the
                    // timeline tables. Keep those migrations valid without
                    // weakening the production schema created for a full library.
                    "PRAGMA user_version = 15;"
                })?;
        }
        Ok(())
    }

    fn table_exists(&self, name: &str) -> Result<bool, SqliteLibraryError> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )?)
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
        {
            let mut statement = transaction.prepare(
                "INSERT INTO hot_cues
                 (track_id, cue_index, time_millis, loop_end_millis, name, color_rgb)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for cue in track.hot_cues() {
                statement.execute(params![
                    track_id,
                    i64::from(cue.index()),
                    to_i64(cue.time_millis())?,
                    cue.loop_end_millis().map(to_i64).transpose()?,
                    cue.name(),
                    i64::from(cue.color_rgb()),
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
        transaction.execute("DELETE FROM hot_cues WHERE track_id = ?1", [track_id])?;
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
                "SELECT baseline_revision, total_beats, origin, reason,
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
        let Some((baseline_revision, total_beats, origin, reason, parent_revision, restored_from)) =
            header
        else {
            return Ok(None);
        };
        let mut statement = self.connection.prepare(
            "SELECT phrase_index, beat, role_id, loop_strategy
             FROM phrase_points
             WHERE track_id = ?1 AND revision = ?2 ORDER BY phrase_index",
        )?;
        let mut rows = statement.query(params![
            to_i64(track_id.value())?,
            to_i64(revision.value())?
        ])?;
        let mut points = Vec::new();
        while let Some(row) = rows.next()? {
            points.push((
                to_u16(row.get(0)?, "phrase point index")?,
                to_u32(row.get(1)?, "phrase point beat")?,
                PhraseRoleId::try_new(row.get::<_, String>(2)?)?,
                decode_loop_strategy(&row.get::<_, String>(3)?)?,
            ));
        }
        let total_beats = to_u32(total_beats, "total beats")?;
        let phrases = points
            .iter()
            .enumerate()
            .map(|(offset, (index, beat, role_id, loop_strategy))| {
                let end_beat = points.get(offset + 1).map_or(total_beats, |point| point.1);
                PhraseInstance::new(*index, *beat, end_beat, role_id.clone())
                    .with_loop_strategy(loop_strategy.clone())
            })
            .collect();
        Ok(Some(LumiPhraseTimeline::try_new_with_history(
            track_id,
            revision,
            SourceRevision::try_new(baseline_revision)?,
            total_beats,
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
        // Lumi exposes one active canonical library source. Activating a
        // different provider replaces the previous provider atomically; the
        // source mirror remains separate staging until this transaction.
        transaction.execute(
            "DELETE FROM playlists WHERE source_id <> ?1",
            [baseline.source_id().as_str()],
        )?;
        transaction.execute(
            "DELETE FROM tracks WHERE source_id <> ?1",
            [baseline.source_id().as_str()],
        )?;
        transaction.execute(
            "DELETE FROM library_sources WHERE source_id <> ?1",
            [baseline.source_id().as_str()],
        )?;
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
        sync_playlists(&transaction, baseline)?;
        transaction.commit()?;
        Ok(ImportResult {
            inserted,
            updated,
            unchanged,
        })
    }

    fn preview_source_mirror(
        &self,
        snapshot: &SourceMirrorSnapshot,
    ) -> Result<SourceMirrorDiff, Self::Error> {
        calculate_source_mirror_diff(&self.connection, snapshot)
    }

    fn apply_source_mirror(
        &mut self,
        snapshot: &SourceMirrorSnapshot,
    ) -> Result<SourceMirrorDiff, Self::Error> {
        let transaction = self.connection.transaction()?;
        let diff = calculate_source_mirror_diff(&transaction, snapshot)?;
        transaction.execute(
            "INSERT INTO source_mirrors(source_id, source_kind, display_name, source_revision)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id) DO UPDATE SET
               source_kind = excluded.source_kind,
               display_name = excluded.display_name,
               source_revision = excluded.source_revision",
            params![
                snapshot.source_id().as_str(),
                snapshot.source_kind(),
                snapshot.display_name(),
                snapshot.source_revision().as_str(),
            ],
        )?;
        transaction.execute(
            "UPDATE source_mirror_tracks SET archived = 1 WHERE source_id = ?1",
            [snapshot.source_id().as_str()],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO source_mirror_tracks
                 (source_id, source_track_id, title, artist, average_bpm, musical_key,
                  duration_millis, color, audio_uri, archived, last_seen_revision)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)
                 ON CONFLICT(source_id, source_track_id) DO UPDATE SET
                   title = excluded.title,
                   artist = excluded.artist,
                   average_bpm = excluded.average_bpm,
                   musical_key = excluded.musical_key,
                   duration_millis = excluded.duration_millis,
                   color = excluded.color,
                   audio_uri = excluded.audio_uri,
                   archived = 0,
                   last_seen_revision = excluded.last_seen_revision",
            )?;
            for track in snapshot.tracks() {
                statement.execute(params![
                    snapshot.source_id().as_str(),
                    track.source_track_id().as_str(),
                    track.title(),
                    track.artist(),
                    track.average_bpm(),
                    track.musical_key(),
                    track.duration_millis().map(to_i64).transpose()?,
                    track.color(),
                    track.audio_uri(),
                    snapshot.source_revision().as_str(),
                ])?;
            }
        }
        transaction.execute(
            "DELETE FROM source_mirror_playlists WHERE source_id = ?1",
            [snapshot.source_id().as_str()],
        )?;
        for playlist in snapshot.playlists() {
            transaction.execute(
                "INSERT INTO source_mirror_playlists(source_id, source_playlist_id, name)
                 VALUES (?1, ?2, ?3)",
                params![
                    snapshot.source_id().as_str(),
                    playlist.source_playlist_id(),
                    playlist.name(),
                ],
            )?;
            let mut statement = transaction.prepare(
                "INSERT INTO source_mirror_playlist_tracks
                 (source_id, source_playlist_id, source_track_id, position)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (position, track_id) in playlist.track_ids().iter().enumerate() {
                statement.execute(params![
                    snapshot.source_id().as_str(),
                    playlist.source_playlist_id(),
                    track_id.as_str(),
                    usize_to_i64(position)?,
                ])?;
            }
        }
        transaction.execute(
            "INSERT INTO source_mirror_revisions
             (source_id, source_revision, active_track_count, archived_track_count, playlist_count)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source_id, source_revision) DO NOTHING",
            params![
                snapshot.source_id().as_str(),
                snapshot.source_revision().as_str(),
                i64::from(diff.active_tracks),
                i64::from(diff.archived_tracks),
                i64::from(diff.playlists),
            ],
        )?;
        transaction.commit()?;
        Ok(diff)
    }

    fn source_mirror_summary(
        &self,
        id: &lumi_library::LibrarySourceId,
    ) -> Result<Option<SourceMirrorSummary>, Self::Error> {
        self.connection
            .query_row(
                "SELECT m.source_kind, m.display_name, m.source_revision,
                        SUM(CASE WHEN t.archived = 0 THEN 1 ELSE 0 END),
                        SUM(CASE WHEN t.archived = 1 THEN 1 ELSE 0 END),
                        (SELECT COUNT(*) FROM source_mirror_playlists p
                          WHERE p.source_id = m.source_id)
                   FROM source_mirrors m
                   LEFT JOIN source_mirror_tracks t ON t.source_id = m.source_id
                  WHERE m.source_id = ?1
                  GROUP BY m.source_id, m.source_kind, m.display_name, m.source_revision",
                [id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?
            .map(|(kind, name, revision, active, archived, playlists)| {
                Ok(SourceMirrorSummary::new(
                    id.clone(),
                    kind,
                    name,
                    SourceRevision::try_new(revision)?,
                    i64_to_u32(active, "active source mirror tracks")?,
                    i64_to_u32(archived, "archived source mirror tracks")?,
                    i64_to_u32(playlists, "source mirror playlists")?,
                ))
            })
            .transpose()
    }

    fn library_source(
        &self,
        id: &lumi_library::LibrarySourceId,
    ) -> Result<Option<lumi_library::LibrarySourceSummary>, Self::Error> {
        self.connection
            .query_row(
                "SELECT source_kind, display_name, source_revision
                 FROM library_sources WHERE source_id = ?1",
                [id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .map(|(kind, display_name, revision)| {
                Ok(lumi_library::LibrarySourceSummary::new(
                    id.clone(),
                    kind,
                    display_name,
                    SourceRevision::try_new(revision)?,
                ))
            })
            .transpose()
    }

    fn complete_source_refresh(
        &mut self,
        baseline: &ImportedLibraryBaseline,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let stored_track_count = transaction.query_row(
            "SELECT COUNT(*) FROM tracks WHERE source_id = ?1",
            [baseline.source_id().as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        if stored_track_count != usize_to_i64(baseline.tracks().len())? {
            return Err(SqliteLibraryError::IncompleteSourceRefresh(
                "the source track set is not identical".to_owned(),
            ));
        }
        for track in baseline.tracks() {
            let stored_revision = transaction
                .query_row(
                    "SELECT analysis_revision FROM tracks
                     WHERE source_id = ?1 AND source_track_id = ?2",
                    params![
                        baseline.source_id().as_str(),
                        track.source_track_id().as_str(),
                    ],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if stored_revision.as_deref() != Some(track.analysis_revision().as_str()) {
                return Err(SqliteLibraryError::IncompleteSourceRefresh(
                    track.source_track_id().as_str().to_owned(),
                ));
            }
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
        let affected = transaction.execute(
            "UPDATE library_sources SET source_kind = ?1, display_name = ?2,
                    source_revision = ?3 WHERE source_id = ?4",
            params![
                baseline.source_kind(),
                baseline.display_name(),
                baseline.source_revision().as_str(),
                baseline.source_id().as_str(),
            ],
        )?;
        if affected != 1 {
            return Err(SqliteLibraryError::MissingLibrarySource(
                baseline.source_id().as_str().to_owned(),
            ));
        }
        sync_playlists(&transaction, baseline)?;
        transaction.commit()?;
        Ok(())
    }

    fn restore_source_checkpoint(
        &mut self,
        baseline: &ImportedLibraryBaseline,
    ) -> Result<(), Self::Error> {
        let transaction = self.connection.transaction()?;
        let affected = transaction.execute(
            "UPDATE library_sources SET source_kind = ?1, display_name = ?2,
                    source_revision = ?3 WHERE source_id = ?4",
            params![
                baseline.source_kind(),
                baseline.display_name(),
                baseline.source_revision().as_str(),
                baseline.source_id().as_str(),
            ],
        )?;
        if affected != 1 {
            return Err(SqliteLibraryError::MissingLibrarySource(
                baseline.source_id().as_str().to_owned(),
            ));
        }
        sync_playlists(&transaction, baseline)?;
        transaction.commit()?;
        Ok(())
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
             (track_id, revision, baseline_revision, total_beats, origin, reason,
              parent_revision, restored_from_revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                track_id,
                to_i64(timeline.revision().value())?,
                timeline.baseline_revision().as_str(),
                i64::from(timeline.total_beats()),
                encode_origin(timeline.origin()),
                encode_reason(timeline.reason()),
                parent_revision,
                restored_from_revision,
            ],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO phrase_points
                 (track_id, revision, phrase_index, beat, role_id, loop_strategy)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for phrase in timeline.phrases() {
                statement.execute(params![
                    track_id,
                    to_i64(timeline.revision().value())?,
                    i64::from(phrase.index()),
                    i64::from(phrase.start_beat()),
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
        let total = self.connection.query_row(
            "SELECT COUNT(DISTINCT LOWER(TRIM(name))) FROM playlists",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection.prepare(
            "SELECT MIN(p.id), MIN(p.source_playlist_id), MIN(p.name),
                    COUNT(DISTINCT pt.track_id)
             FROM playlists p
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
             GROUP BY LOWER(TRIM(p.name))
             ORDER BY MIN(p.name) COLLATE NOCASE, MIN(p.id)
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
            "SELECT COUNT(DISTINCT pt.track_id)
               FROM playlist_tracks pt
               JOIN playlists p ON p.id = pt.playlist_id
              WHERE LOWER(TRIM(p.name)) = (
                    SELECT LOWER(TRIM(name)) FROM playlists WHERE id = ?1
              )",
            [to_i64(playlist_id.value())?],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                    t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                    t.analysis_revision, h.revision
             FROM playlist_tracks pt
             JOIN playlists p ON p.id = pt.playlist_id
             JOIN tracks t ON t.id = pt.track_id
             LEFT JOIN timeline_heads h ON h.track_id = t.id
             WHERE LOWER(TRIM(p.name)) = (
                    SELECT LOWER(TRIM(name)) FROM playlists WHERE id = ?1
             )
             GROUP BY t.id, t.source_track_id, t.title, t.artist, t.bpm_milli,
                      t.key_pitch, t.key_mode, t.duration_millis, t.color_rgb,
                      t.analysis_revision, h.revision
             ORDER BY MIN(CASE WHEN p.id = (
                        SELECT MIN(p2.id) FROM playlists p2
                        WHERE LOWER(TRIM(p2.name)) = LOWER(TRIM(p.name))
                      ) THEN pt.position ELSE 2147483647 END),
                      MIN(pt.position), t.id
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
        let mut cue_statement = self.connection.prepare(
            "SELECT cue_index, time_millis, loop_end_millis, name, color_rgb
             FROM hot_cues WHERE track_id = ?1 ORDER BY cue_index",
        )?;
        let mut cue_rows = cue_statement.query([to_i64(id.value())?])?;
        let mut hot_cues = Vec::new();
        while let Some(row) = cue_rows.next()? {
            hot_cues.push(HotCue::try_new(
                to_u8(row.get(0)?, "hot cue index")?,
                from_nonnegative_i64(row.get(1)?, "hot cue time")?,
                row.get::<_, Option<i64>>(2)?
                    .map(|value| from_nonnegative_i64(value, "hot cue loop end"))
                    .transpose()?,
                row.get::<_, String>(3)?,
                to_u32(row.get(4)?, "hot cue color")?,
            )?);
        }
        Ok(Some(
            StoredTrack::new(summary, audio_uri, beat_grid, waveform, raw_phrases)
                .with_hot_cues(hot_cues),
        ))
    }

    fn phrase_role_catalog(&self) -> Result<PhraseRoleCatalog, Self::Error> {
        let revision = setting_u64(&self.connection, CATALOG_REVISION_KEY)?;
        let defaults_version = u16::try_from(setting_u64(&self.connection, DEFAULTS_VERSION_KEY)?)
            .map_err(|_| SqliteLibraryError::ArithmeticOverflow)?;
        let mut statement = self.connection.prepare(
            "SELECT role_id, display_name, sort_order, archived, color_rgb
             FROM phrase_roles ORDER BY sort_order, display_name COLLATE NOCASE, role_id",
        )?;
        let mut rows = statement.query([])?;
        let mut roles = Vec::new();
        while let Some(row) = rows.next()? {
            roles.push(PhraseRole::try_new_with_color_rgb(
                PhraseRoleId::try_new(row.get::<_, String>(0)?)?,
                row.get::<_, String>(1)?,
                to_u16(row.get(2)?, "phrase role sort order")?,
                row.get(3)?,
                to_u32(row.get(4)?, "phrase role color")?,
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
        set_setting(
            &transaction,
            DEFAULTS_VERSION_KEY,
            u64::from(catalog.defaults_version()),
        )?;
        set_setting(&transaction, CATALOG_REVISION_KEY, catalog.revision())?;
        transaction.commit()?;
        Ok(())
    }

    fn phrase_role_usages(&self) -> Result<Vec<PhraseRoleUsage>, Self::Error> {
        let mut statement = self.connection.prepare(
            "SELECT phrase_points.role_id, tracks.id, tracks.title, COUNT(*)
             FROM phrase_points
             JOIN timeline_heads
               ON timeline_heads.track_id = phrase_points.track_id
              AND timeline_heads.revision = phrase_points.revision
             JOIN tracks ON tracks.id = phrase_points.track_id
             GROUP BY phrase_points.role_id, tracks.id, tracks.title
             ORDER BY phrase_points.role_id, tracks.title COLLATE NOCASE, tracks.id",
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
             (track_id, revision, baseline_revision, total_beats, origin, reason,
              parent_revision, restored_from_revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                to_i64(timeline.track_id().value())?,
                to_i64(timeline.revision().value())?,
                timeline.baseline_revision().as_str(),
                i64::from(timeline.total_beats()),
                encode_origin(timeline.origin()),
                encode_reason(timeline.reason()),
                parent_revision,
                restored_from_revision,
            ],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO phrase_points
                 (track_id, revision, phrase_index, beat, role_id, loop_strategy)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for phrase in timeline.phrases() {
                statement.execute(params![
                    to_i64(timeline.track_id().value())?,
                    to_i64(timeline.revision().value())?,
                    i64::from(phrase.index()),
                    i64::from(phrase.start_beat()),
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
            "SELECT r.revision, r.baseline_revision, r.total_beats, r.origin,
                    r.reason, r.parent_revision, r.restored_from_revision,
                    COUNT(p.phrase_index)
             FROM timeline_revisions r
             LEFT JOIN phrase_points p
               ON p.track_id = r.track_id AND p.revision = r.revision
             WHERE r.track_id = ?1
             GROUP BY r.revision, r.baseline_revision, r.total_beats, r.origin,
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
                to_u32(row.get(2)?, "timeline total beats")?,
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
        "INSERT INTO phrase_roles(role_id, display_name, sort_order, archived, color_rgb)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(role_id) DO UPDATE SET
           display_name = excluded.display_name,
           sort_order = excluded.sort_order,
           archived = excluded.archived,
           color_rgb = excluded.color_rgb",
        params![
            role.id().as_str(),
            role.display_name(),
            i64::from(role.sort_order()),
            role.is_archived(),
            i64::from(role.color_rgb()),
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredMirrorTrack {
    title: String,
    artist: Option<String>,
    average_bpm: Option<String>,
    musical_key: Option<String>,
    duration_millis: Option<u64>,
    color: Option<String>,
    audio_uri: String,
    archived: bool,
}

fn calculate_source_mirror_diff(
    connection: &Connection,
    snapshot: &SourceMirrorSnapshot,
) -> Result<SourceMirrorDiff, SqliteLibraryError> {
    let mut statement = connection.prepare(
        "SELECT source_track_id, title, artist, average_bpm, musical_key,
                duration_millis, color, audio_uri, archived
           FROM source_mirror_tracks WHERE source_id = ?1",
    )?;
    let existing = statement
        .query_map([snapshot.source_id().as_str()], |row| {
            let duration = row
                .get::<_, Option<i64>>(5)?
                .map(|value| {
                    u64::try_from(value).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })
                })
                .transpose()?;
            Ok((
                row.get::<_, String>(0)?,
                StoredMirrorTrack {
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    average_bpm: row.get(3)?,
                    musical_key: row.get(4)?,
                    duration_millis: duration,
                    color: row.get(6)?,
                    audio_uri: row.get(7)?,
                    archived: row.get(8)?,
                },
            ))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let incoming_ids = snapshot
        .tracks()
        .iter()
        .map(|track| track.source_track_id().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut diff = SourceMirrorDiff {
        active_tracks: usize_to_u32(snapshot.tracks().len())?,
        archived_tracks: usize_to_u32(
            existing
                .keys()
                .filter(|track_id| !incoming_ids.contains(track_id.as_str()))
                .count(),
        )?,
        playlists: usize_to_u32(snapshot.playlists().len())?,
        ..SourceMirrorDiff::default()
    };
    for track in snapshot.tracks() {
        let Some(stored) = existing.get(track.source_track_id().as_str()) else {
            diff.inserted = diff
                .inserted
                .checked_add(1)
                .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
            continue;
        };
        if stored.archived {
            diff.restored = diff
                .restored
                .checked_add(1)
                .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        } else if mirror_track_matches(stored, track) {
            diff.unchanged = diff
                .unchanged
                .checked_add(1)
                .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        } else {
            diff.updated = diff
                .updated
                .checked_add(1)
                .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
        }
    }
    diff.archived = usize_to_u32(
        existing
            .iter()
            .filter(|(track_id, track)| {
                !track.archived && !incoming_ids.contains(track_id.as_str())
            })
            .count(),
    )?;
    Ok(diff)
}

fn mirror_track_matches(
    stored: &StoredMirrorTrack,
    incoming: &lumi_library::SourceMirrorTrack,
) -> bool {
    stored.title == incoming.title()
        && stored.artist.as_deref() == incoming.artist()
        && stored.average_bpm.as_deref() == incoming.average_bpm()
        && stored.musical_key.as_deref() == incoming.musical_key()
        && stored.duration_millis == incoming.duration_millis()
        && stored.color.as_deref() == incoming.color()
        && stored.audio_uri == incoming.audio_uri()
}

fn sync_playlists(
    transaction: &Transaction<'_>,
    baseline: &ImportedLibraryBaseline,
) -> Result<(), SqliteLibraryError> {
    let incoming_ids = baseline
        .playlists()
        .iter()
        .map(|playlist| playlist.source_playlist_id().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let obsolete_ids = {
        let mut statement = transaction
            .prepare("SELECT id, source_playlist_id FROM playlists WHERE source_id = ?1")?;
        let rows = statement.query_map([baseline.source_id().as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(id, source_playlist_id)| {
                (!incoming_ids.contains(source_playlist_id.as_str())).then_some(id)
            })
            .collect::<Vec<_>>()
    };
    for playlist_id in obsolete_ids {
        transaction.execute("DELETE FROM playlists WHERE id = ?1", [playlist_id])?;
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

fn replace_track_analysis_in_transaction(
    transaction: &Transaction<'_>,
    analysis: &DeviceAnalysisUpsert,
) -> Result<(), SqliteLibraryError> {
    let track_id = to_i64(analysis.track_id.value())?;
    let reconciled_timeline = timeline_for_analysis_refresh(
        transaction,
        track_id,
        &analysis.analysis_revision,
        &analysis.beat_grid,
        &analysis.raw_phrases,
    )?;
    let updated = transaction.execute(
        "UPDATE tracks SET analysis_revision = ?1, duration_millis = ?2 WHERE id = ?3",
        params![
            analysis.analysis_revision,
            to_i64(analysis.duration_millis)?,
            track_id
        ],
    )?;
    if updated != 1 {
        return Err(SqliteLibraryError::MissingTrack);
    }
    transaction.execute("DELETE FROM beat_markers WHERE track_id = ?1", [track_id])?;
    transaction.execute("DELETE FROM beat_grids WHERE track_id = ?1", [track_id])?;
    transaction.execute(
        "DELETE FROM waveform_points WHERE track_id = ?1",
        [track_id],
    )?;
    transaction.execute("DELETE FROM raw_phrases WHERE track_id = ?1", [track_id])?;
    transaction.execute("DELETE FROM hot_cues WHERE track_id = ?1", [track_id])?;
    transaction.execute(
        "INSERT INTO beat_grids(track_id, beats_per_bar) VALUES (?1, ?2)",
        params![track_id, i64::from(analysis.beat_grid.beats_per_bar())],
    )?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO beat_markers
             (track_id, beat_index, time_millis, bar_index, beat_in_bar)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for marker in analysis.beat_grid.markers() {
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
        for (index, point) in analysis.waveform.iter().enumerate() {
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
        for (index, phrase) in analysis.raw_phrases.iter().enumerate() {
            statement.execute(params![
                track_id,
                usize_to_i64(index)?,
                i64::from(phrase.start_beat()),
                i64::from(phrase.end_beat()),
                phrase.source_label(),
            ])?;
        }
    }
    {
        let mut statement = transaction.prepare(
            "INSERT INTO hot_cues
             (track_id, cue_index, time_millis, loop_end_millis, name, color_rgb)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for cue in &analysis.hot_cues {
            statement.execute(params![
                track_id,
                i64::from(cue.index()),
                to_i64(cue.time_millis())?,
                cue.loop_end_millis().map(to_i64).transpose()?,
                cue.name(),
                i64::from(cue.color_rgb()),
            ])?;
        }
    }
    if let Some(timeline) = reconciled_timeline {
        insert_timeline_revision_in_transaction(transaction, &timeline)?;
    }
    Ok(())
}

fn timeline_for_analysis_refresh(
    transaction: &Transaction<'_>,
    track_id: i64,
    analysis_revision: &str,
    beat_grid: &BeatGrid,
    new_raw_phrases: &[RawPhraseObservation],
) -> Result<Option<LumiPhraseTimeline>, SqliteLibraryError> {
    let Some((head_revision, baseline_revision)) = transaction
        .query_row(
            "SELECT h.revision, r.baseline_revision
               FROM timeline_heads h
               JOIN timeline_revisions r
                 ON r.track_id = h.track_id AND r.revision = h.revision
              WHERE h.track_id = ?1",
            [track_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
    else {
        return Ok(None);
    };
    if baseline_revision == analysis_revision {
        return Ok(None);
    }

    let old_raw_phrases = {
        let mut statement = transaction.prepare(
            "SELECT start_beat, end_beat, source_label
               FROM raw_phrases WHERE track_id = ?1 ORDER BY phrase_index",
        )?;
        statement
            .query_map([track_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut points = {
        let mut statement = transaction.prepare(
            "SELECT beat, role_id, loop_strategy
               FROM phrase_points
              WHERE track_id = ?1 AND revision = ?2
              ORDER BY phrase_index",
        )?;
        statement
            .query_map(params![track_id, head_revision], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    points = repair_legacy_partial_bar_timeline_points(&old_raw_phrases, new_raw_phrases, points);

    let total_beats = i64::from(beat_grid.total_beats());
    points.retain(|(beat, _, _)| *beat >= 0 && *beat < total_beats);
    if points.first().map(|point| point.0) != Some(0) {
        return Err(SqliteLibraryError::CorruptData(
            "analysis refresh lost the first phrase boundary".to_owned(),
        ));
    }
    let revision = timeline_revision(head_revision, "timeline head")?
        .checked_next()
        .ok_or(SqliteLibraryError::ArithmeticOverflow)?;
    let parent = timeline_revision(head_revision, "timeline head")?;
    let phrases = points
        .iter()
        .enumerate()
        .map(|(index, (start, role_id, loop_strategy))| {
            let end = points.get(index + 1).map_or(total_beats, |point| point.0);
            Ok(PhraseInstance::new(
                u16::try_from(index).map_err(|_| SqliteLibraryError::ArithmeticOverflow)?,
                to_u32(*start, "phrase start")?,
                to_u32(end, "phrase end")?,
                PhraseRoleId::try_new(role_id.clone())?,
            )
            .with_loop_strategy(decode_loop_strategy(loop_strategy)?))
        })
        .collect::<Result<Vec<_>, SqliteLibraryError>>()?;
    let timeline = LumiPhraseTimeline::try_new_with_history(
        TrackId::new(from_positive_i64(track_id, "timeline track id")?),
        revision,
        SourceRevision::try_new(analysis_revision)?,
        to_u32(total_beats, "timeline total beats")?,
        TimelineRevisionOrigin::SourceReconcile,
        TimelineRevisionReason::SourceReconcile,
        Some(parent),
        None,
        phrases,
    )?;
    Ok(Some(timeline))
}

fn legacy_partial_bar_phrase_projection(
    old: &[(i64, i64, String)],
    new: &[RawPhraseObservation],
) -> bool {
    old.len() == new.len() + 1
        && old.len() >= 2
        && old[0].0 == 0
        && old[0].1 == 1
        && old[0].2 == old[1].2
        && old.iter().skip(1).zip(new).all(|(old_phrase, new_phrase)| {
            old_phrase.0 == i64::from(new_phrase.start_beat()) + 1
                && old_phrase.2 == new_phrase.source_label()
        })
}

fn repair_legacy_partial_bar_timeline_points(
    old_raw_phrases: &[(i64, i64, String)],
    new_raw_phrases: &[RawPhraseObservation],
    points: Vec<(i64, String, String)>,
) -> Vec<(i64, String, String)> {
    if !legacy_partial_bar_phrase_projection(old_raw_phrases, new_raw_phrases) {
        return points;
    }
    let source_start_mapping = old_raw_phrases
        .iter()
        .skip(1)
        .zip(new_raw_phrases)
        .map(|(old, new)| (old.0, i64::from(new.start_beat())))
        .collect::<BTreeMap<_, _>>();
    let mut repaired = BTreeMap::<i64, (String, String)>::new();
    for (beat, role_id, loop_strategy) in points {
        let mapped = source_start_mapping.get(&beat).copied().unwrap_or(beat);
        // Insertion order deliberately lets the later one-beat source phrase
        // win when it collapses onto beat zero. User-moved boundaries are not
        // source starts and therefore remain exactly where the user put them.
        repaired.insert(mapped, (role_id, loop_strategy));
    }
    repaired
        .into_iter()
        .map(|(beat, (role_id, loop_strategy))| (beat, role_id, loop_strategy))
        .collect()
}

fn insert_timeline_revision_in_transaction(
    transaction: &Transaction<'_>,
    timeline: &LumiPhraseTimeline,
) -> Result<(), SqliteLibraryError> {
    transaction.execute(
        "INSERT INTO timeline_revisions
         (track_id, revision, baseline_revision, total_beats, origin, reason,
          parent_revision, restored_from_revision)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            to_i64(timeline.track_id().value())?,
            to_i64(timeline.revision().value())?,
            timeline.baseline_revision().as_str(),
            i64::from(timeline.total_beats()),
            encode_origin(timeline.origin()),
            encode_reason(timeline.reason()),
            timeline
                .parent_revision()
                .map(|revision| to_i64(revision.value()))
                .transpose()?,
        ],
    )?;
    let mut statement = transaction.prepare(
        "INSERT INTO phrase_points
         (track_id, revision, phrase_index, beat, role_id, loop_strategy)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for phrase in timeline.phrases() {
        statement.execute(params![
            to_i64(timeline.track_id().value())?,
            to_i64(timeline.revision().value())?,
            i64::from(phrase.index()),
            i64::from(phrase.start_beat()),
            phrase.role_id().as_str(),
            encode_loop_strategy(phrase.loop_strategy())?,
        ])?;
    }
    drop(statement);
    transaction.execute(
        "UPDATE timeline_heads SET revision = ?1 WHERE track_id = ?2",
        params![
            to_i64(timeline.revision().value())?,
            to_i64(timeline.track_id().value())?,
        ],
    )?;
    Ok(())
}

fn replace_hot_cues_in_transaction(
    transaction: &Transaction<'_>,
    track_id: TrackId,
    hot_cues: &[HotCue],
) -> Result<(), SqliteLibraryError> {
    let track_id = to_i64(track_id.value())?;
    let track_exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?1)",
        [track_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !track_exists {
        return Err(SqliteLibraryError::MissingTrack);
    }
    transaction.execute("DELETE FROM hot_cues WHERE track_id = ?1", [track_id])?;
    let mut statement = transaction.prepare(
        "INSERT INTO hot_cues
         (track_id, cue_index, time_millis, loop_end_millis, name, color_rgb)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for cue in hot_cues {
        statement.execute(params![
            track_id,
            i64::from(cue.index()),
            to_i64(cue.time_millis())?,
            cue.loop_end_millis().map(to_i64).transpose()?,
            cue.name(),
            i64::from(cue.color_rgb()),
        ])?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreativeArchiveRelinkState {
    Restored,
    Review,
    Pending,
}

fn archive_user_creative_work(transaction: &Transaction<'_>) -> Result<(), SqliteLibraryError> {
    transaction.execute_batch(
        "INSERT INTO creative_track_archives
         (identity_key, original_track_id, title, artist, bpm_milli, duration_millis,
          audio_signature, total_beats, source_timeline_revision, state,
          restored_track_id, updated_at)
         SELECT
            CASE
              WHEN COALESCE((
                    SELECT MAX(NULLIF(a.audio_signature, ''))
                      FROM device_library_track_aliases a
                     WHERE a.canonical_track_id = t.id AND a.archived = 0
              ), '') <> ''
              THEN COALESCE((
                    SELECT MAX(NULLIF(a.audio_signature, ''))
                      FROM device_library_track_aliases a
                     WHERE a.canonical_track_id = t.id AND a.archived = 0
              ), '')
              ELSE 'metadata:' || lower(trim(t.title)) || char(31) ||
                   lower(trim(t.artist)) || char(31) || t.bpm_milli || char(31) ||
                   t.duration_millis
            END,
            t.id, t.title, t.artist, t.bpm_milli, t.duration_millis,
            COALESCE((
                SELECT MAX(NULLIF(a.audio_signature, ''))
                  FROM device_library_track_aliases a
                 WHERE a.canonical_track_id = t.id AND a.archived = 0
            ), ''),
            r.total_beats, h.revision, 'pending', NULL, CURRENT_TIMESTAMP
           FROM tracks t
           JOIN timeline_heads h ON h.track_id = t.id
           JOIN timeline_revisions r ON r.track_id = h.track_id AND r.revision = h.revision
          WHERE EXISTS (
                SELECT 1 FROM timeline_revisions edited
                 WHERE edited.track_id = t.id
                   AND edited.origin IN ('user-edit', 'revision-restore')
          )
          ON CONFLICT(identity_key) DO UPDATE SET
            original_track_id = excluded.original_track_id,
            title = excluded.title,
            artist = excluded.artist,
            bpm_milli = excluded.bpm_milli,
            duration_millis = excluded.duration_millis,
            audio_signature = excluded.audio_signature,
            total_beats = excluded.total_beats,
            source_timeline_revision = excluded.source_timeline_revision,
            state = 'pending',
            restored_track_id = NULL,
            updated_at = CURRENT_TIMESTAMP;

         DELETE FROM creative_phrase_points
          WHERE archive_id IN (
                SELECT a.archive_id
                  FROM creative_track_archives a
                  JOIN tracks t ON t.id = a.original_track_id
                 WHERE EXISTS (
                    SELECT 1 FROM timeline_revisions edited
                     WHERE edited.track_id = t.id
                       AND edited.origin IN ('user-edit', 'revision-restore')
                 )
          );

         INSERT INTO creative_phrase_points
         (archive_id, phrase_index, beat, role_id, loop_strategy)
         SELECT a.archive_id, p.phrase_index, p.beat, p.role_id, p.loop_strategy
           FROM creative_track_archives a
           JOIN timeline_heads h ON h.track_id = a.original_track_id
           JOIN phrase_points p ON p.track_id = h.track_id AND p.revision = h.revision
          WHERE EXISTS (
                SELECT 1 FROM timeline_revisions edited
                 WHERE edited.track_id = h.track_id
                   AND edited.origin IN ('user-edit', 'revision-restore')
          );",
    )?;
    Ok(())
}

fn compare_rekordbox_dates(incoming: &str, active: &str) -> Option<std::cmp::Ordering> {
    fn valid(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    }
    (valid(incoming) && valid(active)).then(|| incoming.cmp(active))
}

fn consolidate_equivalent_usb_sources(
    transaction: &Transaction<'_>,
    source_id: &str,
    display_name: &str,
) -> Result<(), SqliteLibraryError> {
    let current_tracks = complete_active_canonical_set(transaction, source_id)?;
    let Some(current_tracks) = current_tracks.filter(|tracks| !tracks.is_empty()) else {
        return Ok(());
    };
    let candidates = {
        let mut statement = transaction.prepare(
            "SELECT source_id
               FROM device_library_sources
              WHERE source_id <> ?1
                AND source_id LIKE 'usb-fs:%'
                AND display_name = ?2 COLLATE NOCASE",
        )?;
        statement
            .query_map(params![source_id, display_name], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for previous_source_id in candidates {
        if complete_active_canonical_set(transaction, &previous_source_id)?.as_ref()
            != Some(&current_tracks)
        {
            continue;
        }
        let playlist_ids = {
            let mut statement =
                transaction.prepare("SELECT id FROM playlists WHERE source_id = ?1")?;
            statement
                .query_map([&previous_source_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for playlist_id in playlist_ids {
            transaction.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist_id],
            )?;
            transaction.execute("DELETE FROM playlists WHERE id = ?1", [playlist_id])?;
        }
        transaction.execute(
            "UPDATE track_analysis_provenance
                SET source_id = ?1,
                    device_track_id = COALESCE(
                        (SELECT MIN(a.device_track_id)
                           FROM device_library_track_aliases a
                          WHERE a.source_id = ?1
                            AND a.canonical_track_id = track_analysis_provenance.track_id
                            AND a.archived = 0),
                        device_track_id)
              WHERE source_id = ?2",
            params![source_id, previous_source_id],
        )?;
        transaction.execute(
            "UPDATE track_hot_cue_provenance
                SET source_id = ?1,
                    device_track_id = COALESCE(
                        (SELECT MIN(a.device_track_id)
                           FROM device_library_track_aliases a
                          WHERE a.source_id = ?1
                            AND a.canonical_track_id = track_hot_cue_provenance.track_id
                            AND a.archived = 0),
                        device_track_id)
              WHERE source_id = ?2",
            params![source_id, previous_source_id],
        )?;
        transaction.execute(
            "UPDATE track_metadata_provenance
                SET source_id = ?1,
                    device_track_id = COALESCE(
                        (SELECT MIN(a.device_track_id)
                           FROM device_library_track_aliases a
                          WHERE a.source_id = ?1
                            AND a.canonical_track_id = track_metadata_provenance.track_id
                            AND a.archived = 0),
                        device_track_id)
              WHERE source_id = ?2",
            params![source_id, previous_source_id],
        )?;
        transaction.execute(
            "DELETE FROM device_library_track_aliases WHERE source_id = ?1",
            [&previous_source_id],
        )?;
        transaction.execute(
            "DELETE FROM device_library_sources WHERE source_id = ?1",
            [&previous_source_id],
        )?;
        transaction.execute(
            "DELETE FROM library_sources
              WHERE source_id = ?1
                AND NOT EXISTS (SELECT 1 FROM tracks WHERE source_id = ?1)",
            [&previous_source_id],
        )?;
    }
    Ok(())
}

/// Returns `None` when an active alias is unresolved. Such a partial snapshot
/// is never safe evidence for automatic source consolidation.
fn complete_active_canonical_set(
    transaction: &Transaction<'_>,
    source_id: &str,
) -> Result<Option<BTreeSet<i64>>, SqliteLibraryError> {
    let mut statement = transaction.prepare(
        "SELECT canonical_track_id
           FROM device_library_track_aliases
          WHERE source_id = ?1 AND archived = 0",
    )?;
    let rows = statement.query_map([source_id], |row| row.get::<_, Option<i64>>(0))?;
    let mut tracks = BTreeSet::new();
    for row in rows {
        let Some(track_id) = row? else {
            return Ok(None);
        };
        tracks.insert(track_id);
    }
    Ok(Some(tracks))
}

fn backup_staging_path(destination: &Path) -> PathBuf {
    let mut value = destination.as_os_str().to_os_string();
    value.push(".partial");
    PathBuf::from(value)
}

fn validate_backup_connection(connection: &Connection) -> Result<(), SqliteLibraryError> {
    let integrity: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(SqliteLibraryError::IntegrityCheckFailed(integrity));
    }
    let schema: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if schema > SCHEMA_VERSION {
        return Err(SqliteLibraryError::UnsupportedSchema(schema));
    }
    let required_tables: u32 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master
          WHERE type = 'table'
            AND name IN ('tracks', 'phrase_roles', 'autoloop_themes', 'timeline_heads')",
        [],
        |row| row.get(0),
    )?;
    if required_tables != 4 {
        return Err(SqliteLibraryError::CorruptData(
            "backup is missing required creative or lighting tables".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum SqliteLibraryError {
    #[error("SQLite library error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid persisted JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SQLite backup I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("backup destination already exists: {0}")]
    BackupDestinationExists(String),
    #[error("SQLite integrity check failed: {0}")]
    IntegrityCheckFailed(String),
    #[error("library schema version {0} is newer than this Lumi build supports")]
    UnsupportedSchema(u32),
    #[error("Light Planning Policy changed; expected revision {expected}, actual {actual}")]
    LightPlanningRevisionConflict { expected: u64, actual: u64 },
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
    #[error("music-library track is missing")]
    MissingTrack,
    #[error("music-library source {0} is missing")]
    MissingLibrarySource(String),
    #[error("source refresh is incomplete for {0}")]
    IncompleteSourceRefresh(String),
    #[error("source refresh references missing track {0}")]
    MissingReconcileTrack(String),
    #[error("source refresh track identity does not match the timeline")]
    ReconcileTrackIdentityMismatch,
    #[error("analysis revision changed; expected {expected}, actual {actual}")]
    AnalysisRevisionConflict { expected: String, actual: String },
    #[error("the USB review changed; refresh the source before choosing again")]
    DeviceReviewChanged,
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

fn usize_to_u32(value: usize) -> Result<u32, SqliteLibraryError> {
    u32::try_from(value).map_err(|_| SqliteLibraryError::ArithmeticOverflow)
}

fn i64_to_u32(value: i64, label: &str) -> Result<u32, SqliteLibraryError> {
    u32::try_from(value).map_err(|_| SqliteLibraryError::CorruptData(format!("invalid {label}")))
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
    use lumi_domain::TrackId;
    use lumi_library::{
        HotCue, ImportedLibraryBaseline, ImportedTrackAnalysis, LibraryRepository,
        RawPhraseObservation, SourceRevision, TrackColor, TrackPageRequest,
    };
    use lumi_library_demo::DemoLibrarySourceProvider;
    use lumi_library_source::MusicLibrarySourceProvider;
    use lumi_light_plans::{BankOrganization, ColorBehavior, ThemeRule};

    use super::{
        DeviceAliasUpsert, DeviceAnalysisDecision, DeviceAnalysisUpsert, DeviceHotCueUpsert,
        DevicePlaylistUpsert, DeviceTrackImport, SqliteLibraryError, SqliteLibraryRepository,
        legacy_partial_bar_phrase_projection, repair_legacy_partial_bar_timeline_points,
    };

    #[test]
    fn detects_only_the_legacy_partial_bar_phrase_projection()
    -> Result<(), Box<dyn std::error::Error>> {
        let legacy = vec![
            (0, 1, "Intro".to_owned()),
            (1, 33, "Intro".to_owned()),
            (33, 65, "Drop".to_owned()),
        ];
        let corrected = vec![
            RawPhraseObservation::try_new(0, 32, "Intro")?,
            RawPhraseObservation::try_new(32, 64, "Drop")?,
        ];
        assert!(legacy_partial_bar_phrase_projection(&legacy, &corrected));

        let unrelated = vec![(0, 32, "Intro".to_owned()), (32, 64, "Drop".to_owned())];
        assert!(!legacy_partial_bar_phrase_projection(
            &unrelated, &corrected
        ));

        let repaired = repair_legacy_partial_bar_timeline_points(
            &legacy,
            &corrected,
            vec![
                (0, "old-leading".to_owned(), "auto".to_owned()),
                (1, "intro".to_owned(), "fixed".to_owned()),
                // A user deliberately moved the old source boundary from 33.
                (29, "custom".to_owned(), "auto".to_owned()),
            ],
        );
        assert_eq!(
            repaired,
            vec![
                (0, "intro".to_owned(), "fixed".to_owned()),
                (29, "custom".to_owned(), "auto".to_owned()),
            ]
        );
        Ok(())
    }

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

    #[test]
    fn device_hot_cues_enrich_a_track_without_replacing_its_analysis()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let source_track = &baseline.tracks()[0];
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let track_id = repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .tracks()
            .iter()
            .find(|track| track.source_track_id() == source_track.source_track_id())
            .ok_or("stored source track not found")?
            .id();
        let before = repository.track(track_id)?.ok_or("stored track missing")?;
        let cue = HotCue::try_new(1, 12_500, None, "Drop", 0x00ff_3366)?;
        let mut aliases = vec![DeviceAliasUpsert {
            device_track_id: 1_256,
            simulator_signature: 0,
            canonical_track_id: Some(track_id),
            match_kind: "metadata+file-size".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
            metadata_revision: "metadata-v1".to_owned(),
            color_rgb: Some(0x32_80_ff),
            master_database_id: 1,
            master_content_id: 1_256,
            information_update_count: 1,
            analysis_revision: "analysis-v1".to_owned(),
            audio_signature: "audio:test:1256".to_owned(),
            analyzed_at: "2026-08-13".to_owned(),
            sync_disposition: "held-conflict".to_owned(),
        }];

        assert_eq!(
            repository.device_hot_cue_decision(
                track_id,
                "usb-fs:gray",
                "analysis-v1",
                "2026-08-13",
            )?,
            DeviceAnalysisDecision::PromoteInitial
        );
        repository.sync_device_aliases(
            "usb-fs:gray",
            "DJ VIC GRAY",
            "database-v1",
            &mut aliases,
            &[],
            &[],
            &[DeviceHotCueUpsert {
                track_id,
                source_id: "usb-fs:gray".to_owned(),
                device_track_id: 1_256,
                source_analysis_revision: "analysis-v1".to_owned(),
                analyzed_at: "2026-08-13".to_owned(),
                hot_cues: vec![cue.clone()],
            }],
            &[],
        )?;

        let review_tracks = repository.device_review_tracks()?;
        let gray_review = review_tracks
            .get("usb-fs:gray")
            .ok_or("held conflict was not exposed for review")?;
        assert_eq!(gray_review.len(), 1);
        assert_eq!(gray_review[0].device_track_id, 1_256);
        assert_eq!(gray_review[0].canonical_track_id, Some(track_id));
        assert_eq!(gray_review[0].title, source_track.title());
        assert_eq!(
            gray_review[0].active_source_name.as_deref(),
            Some("Lumi library")
        );
        assert_eq!(
            gray_review[0].active_analysis_revision.as_deref(),
            Some(before.summary().source_revision().as_str()),
            "review actions remain available for tracks imported before provenance tracking"
        );

        let after = repository
            .track(track_id)?
            .ok_or("enriched track missing")?;
        assert_eq!(
            after.summary().source_revision(),
            before.summary().source_revision()
        );
        assert_eq!(after.beat_grid(), before.beat_grid());
        assert_eq!(after.waveform(), before.waveform());
        assert_eq!(after.raw_phrases(), before.raw_phrases());
        assert_eq!(after.hot_cues(), &[cue]);
        assert_eq!(
            after.summary().color().map(TrackColor::rgb_u32),
            Some(0x32_80_ff)
        );
        assert_eq!(repository.track_color_summaries()?[0].track_count, 1);

        aliases[0].metadata_revision = "metadata-older".to_owned();
        aliases[0].color_rgb = Some(0xff_33_33);
        aliases[0].information_update_count = 0;
        aliases[0].analyzed_at = "2026-08-12".to_owned();
        repository.sync_device_aliases(
            "usb-fs:gray",
            "DJ VIC GRAY",
            "database-older",
            &mut aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        assert_eq!(
            repository
                .track(track_id)?
                .and_then(|track| track.summary().color())
                .map(TrackColor::rgb_u32),
            Some(0x32_80_ff),
            "an older USB metadata revision must not overwrite the active color"
        );

        aliases[0].metadata_revision = "metadata-newer".to_owned();
        aliases[0].color_rgb = Some(0x32_d7_4b);
        aliases[0].information_update_count = 2;
        aliases[0].analyzed_at = "2026-08-14".to_owned();
        repository.sync_device_aliases(
            "usb-fs:gray",
            "DJ VIC GRAY",
            "database-newer",
            &mut aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        assert_eq!(
            repository
                .track(track_id)?
                .and_then(|track| track.summary().color())
                .map(TrackColor::rgb_u32),
            Some(0x32_d7_4b),
            "a monotone information revision must promote its Rekordbox color"
        );
        assert_eq!(
            repository.device_hot_cue_decision(
                track_id,
                "usb-fs:gray",
                "analysis-v1",
                "2026-08-13",
            )?,
            DeviceAnalysisDecision::Current
        );
        Ok(())
    }

    #[test]
    fn keeping_a_review_is_exact_revision_scoped_and_stale_safe()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let source_track = &baseline.tracks()[0];
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let track_id = repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .tracks()[0]
            .id();
        let active_revision = repository
            .track(track_id)?
            .ok_or("stored track missing")?
            .summary()
            .source_revision()
            .as_str()
            .to_owned();
        let mut aliases = vec![DeviceAliasUpsert {
            device_track_id: 7,
            simulator_signature: 0,
            canonical_track_id: Some(track_id),
            match_kind: "metadata-exact".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
            metadata_revision: "metadata-review".to_owned(),
            color_rgb: source_track.color().map(TrackColor::rgb_u32),
            master_database_id: 1,
            master_content_id: 7,
            information_update_count: 1,
            analysis_revision: "analysis-review".to_owned(),
            audio_signature: "audio:test:7".to_owned(),
            analyzed_at: "2026-08-23".to_owned(),
            sync_disposition: "held-conflict".to_owned(),
        }];
        repository.sync_device_aliases(
            "usb-fs:review",
            "Review USB",
            "database-review",
            &mut aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        assert!(matches!(
            repository.keep_active_device_analysis(
                "usb-fs:review",
                7,
                "analysis-review",
                "stale-active",
            ),
            Err(SqliteLibraryError::DeviceReviewChanged)
        ));
        repository.keep_active_device_analysis(
            "usb-fs:review",
            7,
            "analysis-review",
            &active_revision,
        )?;
        assert!(repository.device_review_tracks()?.is_empty());
        assert_eq!(repository.device_source_summaries()?[0].current_tracks, 1);
        Ok(())
    }

    #[test]
    fn promoting_a_review_replaces_the_exact_source_projection_atomically()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let source_track = &baseline.tracks()[0];
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let track_id = repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .tracks()[0]
            .id();
        let active_revision = repository
            .track(track_id)?
            .ok_or("stored track missing")?
            .summary()
            .source_revision()
            .as_str()
            .to_owned();
        let mut aliases = vec![DeviceAliasUpsert {
            device_track_id: 8,
            simulator_signature: 0,
            canonical_track_id: Some(track_id),
            match_kind: "metadata-exact".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
            metadata_revision: "metadata-review".to_owned(),
            color_rgb: source_track.color().map(TrackColor::rgb_u32),
            master_database_id: 1,
            master_content_id: 8,
            information_update_count: 1,
            analysis_revision: "analysis-review".to_owned(),
            audio_signature: "audio:test:8".to_owned(),
            analyzed_at: "2026-08-23".to_owned(),
            sync_disposition: "held-conflict".to_owned(),
        }];
        repository.sync_device_aliases(
            "usb-fs:review",
            "Review USB",
            "database-review",
            &mut aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        let reviewed_phrases = vec![RawPhraseObservation::try_new(
            0,
            source_track.beat_grid().total_beats(),
            "Reviewed USB phrase",
        )?];
        repository.promote_reviewed_device_analysis(
            &DeviceAnalysisUpsert {
                track_id,
                source_id: "usb-fs:review".to_owned(),
                device_track_id: 8,
                analysis_revision: "device:review:analysis-review".to_owned(),
                source_analysis_revision: "analysis-review".to_owned(),
                analyzed_at: "2026-08-23".to_owned(),
                duration_millis: source_track.duration_millis() + 9,
                beat_grid: source_track.beat_grid().clone(),
                waveform: source_track.waveform().to_vec(),
                raw_phrases: reviewed_phrases.clone(),
                hot_cues: source_track.hot_cues().to_vec(),
            },
            &active_revision,
        )?;
        let promoted = repository
            .track(track_id)?
            .ok_or("promoted track missing")?;
        assert_eq!(
            promoted.summary().source_revision().as_str(),
            "device:review:analysis-review"
        );
        assert_eq!(promoted.raw_phrases(), reviewed_phrases);
        assert!(repository.device_review_tracks()?.is_empty());
        assert_eq!(repository.device_source_summaries()?[0].promoted_tracks, 1);

        let mut second_source = vec![DeviceAliasUpsert {
            device_track_id: 9,
            simulator_signature: 0,
            canonical_track_id: Some(track_id),
            match_kind: "metadata-exact".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Second/Track.mp3".to_owned(),
            metadata_revision: "metadata-second".to_owned(),
            color_rgb: source_track.color().map(TrackColor::rgb_u32),
            master_database_id: 2,
            master_content_id: 9,
            information_update_count: 1,
            analysis_revision: "analysis-second".to_owned(),
            audio_signature: "audio:test:9".to_owned(),
            analyzed_at: "2026-08-23".to_owned(),
            sync_disposition: "held-conflict".to_owned(),
        }];
        repository.sync_device_aliases(
            "usb-fs:second",
            "Second USB",
            "database-second",
            &mut second_source,
            &[],
            &[],
            &[],
            &[],
        )?;
        let reviews = repository.device_review_tracks()?;
        let review = &reviews["usb-fs:second"][0];
        assert_eq!(
            review.active_analysis_revision.as_deref(),
            Some("device:review:analysis-review"),
            "the UI action must carry the active canonical projection revision"
        );
        repository.keep_active_device_analysis(
            "usb-fs:second",
            9,
            "analysis-second",
            review
                .active_analysis_revision
                .as_deref()
                .ok_or("active review revision missing")?,
        )?;
        assert!(repository.device_review_tracks()?.is_empty());
        Ok(())
    }

    #[test]
    fn device_alias_resolves_real_and_simulated_identity_and_archives_on_resync()
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
        let mut aliases = vec![DeviceAliasUpsert {
            device_track_id: 1_256,
            simulator_signature: 3_456_789_012,
            audio_signature: "audio:test:1256".to_owned(),
            canonical_track_id: Some(track_id),
            match_kind: "metadata+file-size".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
            metadata_revision: "metadata-v1".to_owned(),
            color_rgb: Some(0x32_80_ff),
            master_database_id: 1,
            master_content_id: 1_256,
            information_update_count: 1,
            analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-10".to_owned(),
            sync_disposition: "promoted-initial".to_owned(),
        }];
        let refreshed_duration = source_track.duration_millis() + 7;
        let refreshed_raw_phrases = vec![RawPhraseObservation::try_new(
            0,
            source_track.beat_grid().total_beats(),
            "Refreshed source phrase",
        )?];
        let analyses = vec![DeviceAnalysisUpsert {
            track_id,
            source_id: "rekordbox-device:test".to_owned(),
            device_track_id: 1_256,
            analysis_revision: "device:test:analysis-v1".to_owned(),
            source_analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-10".to_owned(),
            duration_millis: refreshed_duration,
            beat_grid: source_track.beat_grid().clone(),
            waveform: source_track.waveform().to_vec(),
            raw_phrases: refreshed_raw_phrases.clone(),
            hot_cues: source_track.hot_cues().to_vec(),
        }];
        repository.sync_device_aliases(
            "rekordbox-device:test",
            "Test USB",
            "database-v1",
            &mut aliases,
            &[],
            &analyses,
            &[],
            &[DevicePlaylistUpsert {
                device_playlist_id: 77,
                path: "Sets/90s Dance/90s Club".to_owned(),
                device_track_ids: vec![1_256],
            }],
        )?;

        let refreshed = repository
            .track(track_id)?
            .ok_or("refreshed track missing")?;
        assert_eq!(refreshed.summary().duration_millis(), refreshed_duration);
        assert_eq!(refreshed.raw_phrases(), refreshed_raw_phrases);

        let device_playlist = repository
            .page_playlists(TrackPageRequest::try_new(0, 25)?)?
            .playlists()
            .iter()
            .find(|playlist| playlist.source_playlist_id().as_str() == "onelibrary:77")
            .cloned()
            .ok_or("device playlist not persisted")?;
        assert_eq!(device_playlist.name(), "Sets/90s Dance/90s Club");
        assert_eq!(device_playlist.track_count(), 1);

        assert_eq!(
            repository
                .resolve_device_alias(1_256, 0)?
                .ok_or("real device alias not resolved")?
                .canonical_track_id,
            track_id
        );
        assert_eq!(
            repository
                .resolve_device_alias(42, 3_456_789_012)?
                .ok_or("simulator alias not resolved")?
                .canonical_track_id,
            track_id
        );
        let summary = repository.device_source_summaries()?;
        assert_eq!(summary[0].active_tracks, 1);
        assert_eq!(summary[0].matched_tracks, 1);
        let relations = repository.device_track_source_relations(&[track_id])?;
        assert_eq!(relations[&track_id].len(), 1);
        assert_eq!(relations[&track_id][0].display_name, "Test USB");
        assert_eq!(relations[&track_id][0].sync_disposition, "promoted-initial");
        assert_eq!(
            repository.device_analysis_decision(
                track_id,
                "rekordbox-device:test",
                "analysis-v1",
                "2026-08-10",
            )?,
            DeviceAnalysisDecision::Current
        );
        assert_eq!(
            repository.device_analysis_decision(
                track_id,
                "rekordbox-device:backup",
                "analysis-older",
                "2026-08-09",
            )?,
            DeviceAnalysisDecision::ProtectOlder
        );
        assert_eq!(
            repository.device_analysis_decision(
                track_id,
                "rekordbox-device:test",
                "analysis-v2-content-change",
                "2026-08-10",
            )?,
            DeviceAnalysisDecision::PromoteNewer
        );
        assert_eq!(
            repository.device_analysis_decision(
                track_id,
                "rekordbox-device:primary",
                "analysis-newer",
                "2026-08-11",
            )?,
            DeviceAnalysisDecision::PromoteNewer
        );
        assert_eq!(
            repository.device_analysis_decision(
                track_id,
                "rekordbox-device:backup",
                "analysis-same-day-conflict",
                "2026-08-10",
            )?,
            DeviceAnalysisDecision::HoldConflict
        );

        repository.sync_device_aliases(
            "rekordbox-device:test",
            "Test USB",
            "database-v2",
            &mut [],
            &[],
            &[],
            &[],
            &[],
        )?;
        assert!(repository.resolve_device_alias(1_256, 0)?.is_none());
        assert!(
            repository
                .resolve_device_alias(42, 3_456_789_012)?
                .is_none()
        );
        assert!(
            repository
                .device_track_source_relations(&[track_id])?
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn device_sync_imports_new_tracks_and_playlist_membership_atomically()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let source_track = &baseline.tracks()[0];
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let before = repository
            .page_tracks(TrackPageRequest::try_new(0, 100)?)?
            .total();
        let imported_analysis = ImportedTrackAnalysis::try_new(
            lumi_library::SourceTrackId::try_new("onelibrary:9001")?,
            SourceRevision::try_new("device:usb-fs:test:analysis-new")?,
            "A New USB Track",
            "USB Artist",
            source_track.bpm_milli(),
            source_track.musical_key(),
            source_track.duration_millis(),
            source_track.color(),
            "file://localhost/Volumes/Test/A%20New%20USB%20Track.wav",
            source_track.beat_grid().clone(),
            source_track.waveform().to_vec(),
            source_track.raw_phrases().to_vec(),
        )?;
        let mut aliases = vec![DeviceAliasUpsert {
            device_track_id: 9_001,
            simulator_signature: 123,
            audio_signature: "audio:test:9001".to_owned(),
            canonical_track_id: None,
            match_kind: "unmatched".to_owned(),
            title: "A New USB Track".to_owned(),
            artist: "USB Artist".to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 321,
            audio_uri: "file://localhost/Volumes/Test/A%20New%20USB%20Track.wav".to_owned(),
            metadata_revision: "metadata-new".to_owned(),
            color_rgb: Some(0xff_33_33),
            master_database_id: 1,
            master_content_id: 9_001,
            information_update_count: 1,
            analysis_revision: "analysis-new".to_owned(),
            analyzed_at: "2026-08-10".to_owned(),
            sync_disposition: "unmatched".to_owned(),
        }];
        repository.sync_device_aliases(
            "usb-fs:test",
            "Test USB",
            "database-new",
            &mut aliases,
            &[DeviceTrackImport {
                device_track_id: 9_001,
                source_analysis_revision: "analysis-new".to_owned(),
                analyzed_at: "2026-08-10".to_owned(),
                analysis: imported_analysis,
            }],
            &[],
            &[],
            &[DevicePlaylistUpsert {
                device_playlist_id: 77,
                path: "MainStage 140+".to_owned(),
                device_track_ids: vec![9_001],
            }],
        )?;

        let resolution = repository
            .resolve_device_alias(9_001, 0)?
            .ok_or("new USB track did not resolve")?;
        assert_eq!(
            aliases[0].canonical_track_id,
            Some(resolution.canonical_track_id)
        );
        assert_eq!(aliases[0].match_kind, "imported-device");
        assert_eq!(aliases[0].sync_disposition, "promoted-initial");
        assert_eq!(
            repository
                .page_tracks(TrackPageRequest::try_new(0, 100)?)?
                .total(),
            before + 1
        );
        let playlist = repository
            .page_playlists(TrackPageRequest::try_new(0, 100)?)?
            .playlists()
            .iter()
            .find(|playlist| playlist.source_playlist_id().as_str() == "onelibrary:77")
            .cloned()
            .ok_or("imported USB playlist missing")?;
        assert_eq!(playlist.track_count(), 1);
        assert_eq!(
            repository
                .page_playlist_tracks(playlist.id(), TrackPageRequest::try_new(0, 100)?)?
                .tracks()[0]
                .title(),
            "A New USB Track"
        );

        let canonical_track_id = repository
            .page_tracks(TrackPageRequest::try_new(0, 100)?)?
            .tracks()
            .iter()
            .find(|track| track.source_track_id() == source_track.source_track_id())
            .ok_or("provider canonical track missing")?
            .id();
        aliases[0].canonical_track_id = Some(canonical_track_id);
        aliases[0].match_kind = "metadata-canonical-repair".to_owned();
        aliases[0].sync_disposition = "current".to_owned();
        repository.sync_device_aliases(
            "usb-fs:test",
            "Test USB",
            "database-repaired",
            &mut aliases,
            &[],
            &[],
            &[],
            &[DevicePlaylistUpsert {
                device_playlist_id: 77,
                path: "MainStage 140+".to_owned(),
                device_track_ids: vec![9_001],
            }],
        )?;
        assert_eq!(
            repository
                .page_tracks(TrackPageRequest::try_new(0, 100)?)?
                .total(),
            before
        );
        let repaired_playlist = repository
            .stored_device_playlists()?
            .remove("usb-fs:test")
            .and_then(|mut playlists| playlists.pop())
            .ok_or("stored device playlist missing")?;
        assert_eq!(repaired_playlist.track_count, 1);
        assert_eq!(
            repository
                .page_playlist_tracks(
                    repaired_playlist.playlist_id,
                    TrackPageRequest::try_new(0, 100)?,
                )?
                .tracks()[0]
                .id(),
            canonical_track_id
        );
        Ok(())
    }

    #[test]
    fn identical_playlists_from_two_usb_sources_are_presented_once()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let source_tracks = &baseline.tracks()[..2];
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let track_page = repository.page_tracks(TrackPageRequest::try_new(0, 25)?)?;
        let canonical_track_ids = source_tracks
            .iter()
            .map(|source_track| {
                track_page
                    .tracks()
                    .iter()
                    .find(|track| track.source_track_id() == source_track.source_track_id())
                    .map(lumi_library::TrackSummary::id)
                    .ok_or("canonical track missing")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let aliases = source_tracks
            .iter()
            .zip(&canonical_track_ids)
            .enumerate()
            .map(
                |(offset, (source_track, canonical_track_id))| -> Result<_, std::num::TryFromIntError> {
                    Ok(DeviceAliasUpsert {
                    device_track_id: 90 + u32::try_from(offset)?,
                    simulator_signature: 0,
                    audio_signature: format!("audio:shared:{}", 90 + offset),
                    canonical_track_id: Some(*canonical_track_id),
                    match_kind: "metadata+file-size".to_owned(),
                    title: source_track.title().to_owned(),
                    artist: source_track.artist().to_owned(),
                    bpm_milli: source_track.bpm_milli(),
                    duration_millis: source_track.duration_millis(),
                    file_size: 123,
                    audio_uri: format!("file://localhost/Volumes/Test/Track-{offset}.mp3"),
                    metadata_revision: "metadata-v1".to_owned(),
                    color_rgb: None,
                    master_database_id: 1,
                    master_content_id: 90 + u32::try_from(offset)?,
                    information_update_count: 1,
                    analysis_revision: "analysis-v1".to_owned(),
                    analyzed_at: "2026-08-13".to_owned(),
                    sync_disposition: "current".to_owned(),
                })},
            )
            .collect::<Result<Vec<_>, _>>()?;
        let playlist = DevicePlaylistUpsert {
            device_playlist_id: 77,
            path: "Genre 5 Stars/MainStage 140+".to_owned(),
            device_track_ids: vec![90, 91],
        };
        for (source_id, display_name) in [
            ("usb-fs:chrm", "DJ VIC CHRM"),
            ("usb-fs:gray", "DJ VIC GRAY"),
        ] {
            repository.sync_device_aliases(
                source_id,
                display_name,
                "database-v1",
                &mut aliases.clone(),
                &[],
                &[],
                &[],
                std::slice::from_ref(&playlist),
            )?;
        }

        for (device_track_id, canonical_track_id) in
            [90_u32, 91].into_iter().zip(&canonical_track_ids)
        {
            assert_eq!(
                repository
                    .resolve_device_alias(device_track_id, 0)?
                    .ok_or("shared backup USB alias did not resolve")?
                    .canonical_track_id,
                *canonical_track_id
            );
        }

        let page = repository.page_playlists(TrackPageRequest::try_new(0, 25)?)?;
        let matching = page
            .playlists()
            .iter()
            .filter(|playlist| playlist.name() == "Genre 5 Stars/MainStage 140+")
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].track_count(), 2);
        let tracks =
            repository.page_playlist_tracks(matching[0].id(), TrackPageRequest::try_new(0, 25)?)?;
        assert_eq!(tracks.total(), 2);
        assert_eq!(
            tracks
                .tracks()
                .iter()
                .map(lumi_library::TrackSummary::id)
                .collect::<Vec<_>>(),
            canonical_track_ids
        );
        let relations = repository.device_track_source_relations(&canonical_track_ids)?;
        assert!(
            canonical_track_ids
                .iter()
                .all(|track_id| relations[track_id].len() == 2)
        );
        Ok(())
    }

    #[test]
    fn conflicting_usb_aliases_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let canonical = repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .tracks()
            .iter()
            .take(2)
            .map(lumi_library::TrackSummary::id)
            .collect::<Vec<_>>();
        assert_eq!(canonical.len(), 2);

        for (source_id, canonical_track_id) in
            ["usb-fs:gray", "usb-fs:chrm"].into_iter().zip(canonical)
        {
            let mut aliases = vec![DeviceAliasUpsert {
                device_track_id: 1_256,
                simulator_signature: 0,
                audio_signature: "audio:ambiguous:1256".to_owned(),
                canonical_track_id: Some(canonical_track_id),
                match_kind: "metadata+file-size".to_owned(),
                title: "Ambiguous".to_owned(),
                artist: "Test".to_owned(),
                bpm_milli: 140_000,
                duration_millis: 180_000,
                file_size: 123,
                audio_uri: "file://localhost/Volumes/Test/Ambiguous.mp3".to_owned(),
                metadata_revision: "metadata-v1".to_owned(),
                color_rgb: None,
                master_database_id: 1,
                master_content_id: 1_256,
                information_update_count: 1,
                analysis_revision: "analysis-v1".to_owned(),
                analyzed_at: "2026-08-16".to_owned(),
                sync_disposition: "current".to_owned(),
            }];
            repository.sync_device_aliases(
                source_id,
                source_id,
                "database-v1",
                &mut aliases,
                &[],
                &[],
                &[],
                &[],
            )?;
        }

        assert!(repository.resolve_device_alias(1_256, 0)?.is_none());
        Ok(())
    }

    #[test]
    fn stable_filesystem_identity_replaces_ephemeral_mount_records_atomically()
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
        let alias = DeviceAliasUpsert {
            device_track_id: 88,
            simulator_signature: 99,
            audio_signature: "audio:test:88".to_owned(),
            canonical_track_id: Some(track_id),
            match_kind: "metadata+file-size".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
            metadata_revision: "metadata-v1".to_owned(),
            color_rgb: None,
            master_database_id: 1,
            master_content_id: 88,
            information_update_count: 1,
            analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-10".to_owned(),
            sync_disposition: "current".to_owned(),
        };
        let playlist = DevicePlaylistUpsert {
            device_playlist_id: 24,
            path: "Genre 5 Stars/MainStage 140+".to_owned(),
            device_track_ids: vec![88],
        };
        let mut legacy_aliases = vec![alias.clone()];
        repository.sync_device_aliases(
            "usb-volume:{legacy-mount}",
            "DJ VIC CHRM",
            "database-v1",
            &mut legacy_aliases,
            &[],
            &[],
            &[],
            std::slice::from_ref(&playlist),
        )?;
        let mut stable_aliases = vec![alias];
        repository.sync_device_aliases(
            "usb-fs:stable-volume-uuid",
            "DJ VIC CHRM",
            "database-v1",
            &mut stable_aliases,
            &[],
            &[],
            &[],
            &[playlist],
        )?;

        let sources = repository.device_source_summaries()?;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_id, "usb-fs:stable-volume-uuid");
        let relation = repository.device_track_source_relations(&[track_id])?;
        assert_eq!(relation[&track_id].len(), 1);
        assert_eq!(
            relation[&track_id][0].source_id,
            "usb-fs:stable-volume-uuid"
        );
        let playlists = repository.page_playlists(TrackPageRequest::try_new(0, 25)?)?;
        assert_eq!(
            playlists
                .playlists()
                .iter()
                .filter(|value| value.source_playlist_id().as_str() == "onelibrary:24")
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn changed_filesystem_uuid_consolidates_only_an_equivalent_named_usb()
    -> Result<(), Box<dyn std::error::Error>> {
        let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        let track_ids = repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .tracks()
            .iter()
            .take(2)
            .map(lumi_library::TrackSummary::id)
            .collect::<Vec<_>>();
        let source_track = &baseline.tracks()[0];
        let alias_for = |track_id: TrackId, audio_uri: &str| DeviceAliasUpsert {
            device_track_id: 88,
            simulator_signature: 99,
            audio_signature: "audio:test:88".to_owned(),
            canonical_track_id: Some(track_id),
            match_kind: "metadata+file-size".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: audio_uri.to_owned(),
            metadata_revision: "metadata-v1".to_owned(),
            color_rgb: None,
            master_database_id: 1,
            master_content_id: 88,
            information_update_count: 1,
            analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-22".to_owned(),
            sync_disposition: "current".to_owned(),
        };

        let mut old_aliases = vec![alias_for(
            track_ids[0],
            "file://localhost/Volumes/Old%20Gray/Track.mp3",
        )];
        repository.sync_device_aliases(
            "usb-fs:old-gray-uuid",
            "DJ VIC GRAY",
            "database-old",
            &mut old_aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        let mut equivalent_aliases = vec![alias_for(
            track_ids[0],
            "file://localhost/Volumes/DJ%20VIC%20GRAY/Track.mp3",
        )];
        repository.sync_device_aliases(
            "usb-fs:new-gray-uuid",
            "DJ VIC GRAY",
            "database-new",
            &mut equivalent_aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        assert_eq!(repository.device_source_summaries()?.len(), 1);
        assert_eq!(
            repository.device_audio_uris(track_ids[0])?,
            vec!["file://localhost/Volumes/DJ%20VIC%20GRAY/Track.mp3"]
        );

        let mut different_aliases = vec![alias_for(
            track_ids[1],
            "file://localhost/Volumes/Other%20Gray/Track.mp3",
        )];
        repository.sync_device_aliases(
            "usb-fs:different-gray-uuid",
            "DJ VIC GRAY",
            "database-different",
            &mut different_aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        assert_eq!(repository.device_source_summaries()?.len(), 2);
        Ok(())
    }

    #[test]
    fn stable_filesystem_identity_replaces_reset_pending_provider_identity()
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
        let alias = DeviceAliasUpsert {
            device_track_id: 1_256,
            simulator_signature: 99,
            audio_signature: "audio:test:1256".to_owned(),
            canonical_track_id: Some(track_id),
            match_kind: "metadata+file-size".to_owned(),
            title: source_track.title().to_owned(),
            artist: source_track.artist().to_owned(),
            bpm_milli: source_track.bpm_milli(),
            duration_millis: source_track.duration_millis(),
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
            metadata_revision: "metadata-v1".to_owned(),
            color_rgb: None,
            master_database_id: 1,
            master_content_id: 1_256,
            information_update_count: 1,
            analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-10".to_owned(),
            sync_disposition: "current".to_owned(),
        };
        let mut legacy_aliases = vec![alias.clone()];
        repository.sync_device_aliases(
            "rekordbox-device:dj-vic-gray",
            "DJ VIC GRAY",
            "database-before-reset",
            &mut legacy_aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
        repository.connection.execute(
            "UPDATE device_library_sources
                SET database_revision = 'reset-pending'
              WHERE source_id = 'rekordbox-device:dj-vic-gray'",
            [],
        )?;

        let mut stable_aliases = vec![alias];
        repository.sync_device_aliases(
            "usb-fs:stable-gray-volume-uuid",
            "DJ VIC GRAY",
            "database-after-reset",
            &mut stable_aliases,
            &[],
            &[],
            &[],
            &[],
        )?;

        let sources = repository.device_source_summaries()?;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_id, "usb-fs:stable-gray-volume-uuid");
        let relations = repository.device_track_source_relations(&[track_id])?;
        assert_eq!(relations[&track_id].len(), 1);
        assert_eq!(relations[&track_id][0].display_name, "DJ VIC GRAY");
        assert_eq!(
            repository
                .resolve_device_alias(1_256, 0)?
                .ok_or("stable GRAY alias did not resolve")?
                .source_id,
            "usb-fs:stable-gray-volume-uuid"
        );
        Ok(())
    }

    #[test]
    fn light_planning_policy_is_revisioned_and_persistent() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut repository = SqliteLibraryRepository::in_memory()?;
        let initial = repository.light_planning_policy()?;
        assert_eq!(initial.revision, 1);
        let mut replacement = initial.clone();
        replacement.theme_cooldown_tracks = 2;
        replacement.bank_organization = BankOrganization::Themes;
        replacement.default_theme_id = Some(2);
        replacement.automatic_mid_track_theme_changes = false;
        replacement.theme_rules = vec![
            ThemeRule {
                theme_id: 1,
                enabled: true,
                selection_weight: 2,
                color_behavior: ColorBehavior::Neutral,
                color_rgb: Vec::new(),
            },
            ThemeRule {
                theme_id: 2,
                enabled: true,
                selection_weight: 4,
                color_behavior: ColorBehavior::Prefer,
                color_rgb: vec![0xff_00_00],
            },
        ];
        let stored = repository.replace_light_planning_policy(1, replacement)?;
        assert_eq!(stored.revision, 2);
        assert_eq!(stored.default_theme_id, Some(2));
        assert_eq!(stored.theme_rules.len(), 2);
        assert_eq!(repository.light_planning_policy()?, stored);
        assert!(matches!(
            repository.replace_light_planning_policy(1, initial),
            Err(SqliteLibraryError::LightPlanningRevisionConflict {
                expected: 1,
                actual: 2
            })
        ));
        let schema: u32 = repository
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(schema, 15);
        Ok(())
    }
}
