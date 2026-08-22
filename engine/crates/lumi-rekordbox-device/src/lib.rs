//! Read-only adapter for current Rekordbox OneLibrary USB media.
//!
//! This crate exposes no write operation. It reads the SQLCipher-backed
//! `PIONEER/rekordbox/exportLibrary.db` and fingerprints the referenced
//! analysis companions. Beat-grid and cue changes therefore become visible
//! to Lumi during the next inspection, before the user chooses to sync.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use thiserror::Error;

const DATABASE_RELATIVE_PATH: &str = "PIONEER/rekordbox/exportLibrary.db";
const MAX_DATABASE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ANALYSIS_FILE_BYTES: u64 = 64 * 1024 * 1024;
const AUDIO_SIGNATURE_WINDOW_BYTES: usize = 64 * 1024;
// OneLibrary media uses this shared SQLCipher format key. It is not a user
// credential and compatible open-source readers implement the same key.
const ONELIBRARY_FORMAT_KEY: &str =
    "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceTrack {
    pub device_track_id: u32,
    pub title: String,
    pub artist: String,
    pub musical_key: String,
    /// Canonical RGB projection of Rekordbox's fixed track-color catalog.
    pub color_rgb: Option<u32>,
    pub bpm_milli: u32,
    pub duration_millis: u32,
    pub file_size: u32,
    pub audio_path: PathBuf,
    pub analysis_dat_path: PathBuf,
    pub metadata_revision: String,
    pub analysis_revision: String,
    /// Export date retained as a conservative fallback ordering fact.
    pub analyzed_at: String,
    /// Stable, bounded signature of the audio container. It allows trusted
    /// backup exports with renamed metadata to resolve to the same canonical
    /// Lumi track without hashing the complete music library.
    pub audio_signature: String,
    /// Metadata fallback used only by BLT's shallow simulator, whose
    /// `rekordbox-id` is intentionally hard-coded to 42 for every track.
    pub simulator_signature: u32,
    /// Stable OneLibrary identity and monotone update counters.
    pub master_database_id: u32,
    pub master_content_id: u32,
    pub analysis_update_count: u32,
    pub information_update_count: u32,
    pub cue_update_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlaylist {
    pub device_playlist_id: u32,
    pub path: String,
    pub name: String,
    pub track_ids: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLibrarySnapshot {
    pub source_id: String,
    pub display_name: String,
    pub database_path: PathBuf,
    pub database_revision: String,
    pub database_version: String,
    pub exported_at: String,
    pub tracks: BTreeMap<u32, DeviceTrack>,
    pub playlists: Vec<DevicePlaylist>,
}

impl DeviceLibrarySnapshot {
    #[must_use]
    pub fn track(&self, device_track_id: u32) -> Option<&DeviceTrack> {
        self.tracks.get(&device_track_id)
    }
}

/// Reads a mounted Rekordbox device without opening any file for writing.
pub fn read_device_library(root: impl AsRef<Path>) -> Result<DeviceLibrarySnapshot, DeviceError> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(DeviceError::InvalidDeviceRoot);
    }
    let canonical_root = fs::canonicalize(root)?;
    let database_path = canonical_child(&canonical_root, Path::new(DATABASE_RELATIVE_PATH))?;
    let database_metadata = fs::metadata(&database_path)?;
    if !database_metadata.is_file() || database_metadata.len() > MAX_DATABASE_BYTES {
        return Err(DeviceError::InvalidDatabaseFile);
    }
    let database_revision = sha256_file(&database_path, MAX_DATABASE_BYTES)?;
    let database = open_onelibrary(&database_path)?;
    let (exported_device_name, database_version, exported_at) = database
        .query_row(
            "SELECT COALESCE(deviceName, ''), COALESCE(dbVersion, ''),
                    COALESCE(createdDate, '') FROM property LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(database_error)?;
    let mut tracks = BTreeMap::new();
    let mut statement = database
        .prepare(
            "SELECT c.content_id, COALESCE(c.title, ''), COALESCE(a.name, ''),
                    COALESCE(k.name, ''), COALESCE(color.name, ''),
                    CAST(COALESCE(c.bpmx100, 0) AS INTEGER),
                    CAST(COALESCE(c.length, 0) AS INTEGER),
                    CAST(COALESCE(c.fileSize, 0) AS INTEGER),
                    COALESCE(c.path, ''), COALESCE(c.analysisDataFilePath, ''),
                    CAST(COALESCE(c.masterDbId, 0) AS INTEGER),
                    CAST(COALESCE(c.masterContentId, 0) AS INTEGER),
                    CAST(COALESCE(c.analysisDataUpdateCount, 0) AS INTEGER),
                    CAST(COALESCE(c.informationUpdateCount, 0) AS INTEGER),
                    CAST(COALESCE(c.cueUpdateCount, 0) AS INTEGER)
               FROM content c
               LEFT JOIN artist a ON c.artist_id_artist = a.artist_id
               LEFT JOIN key k ON c.key_id = k.key_id
               LEFT JOIN color ON c.color_id = color.color_id
              ORDER BY c.content_id",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(OneLibraryTrackRow {
                id: checked_u32(row.get::<_, i64>(0)?, 0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                musical_key: row.get(3)?,
                color_name: row.get(4)?,
                bpm_centi: checked_u32(row.get::<_, i64>(5)?, 5)?,
                duration: checked_u32(row.get::<_, i64>(6)?, 6)?,
                file_size: checked_u32(row.get::<_, i64>(7)?, 7)?,
                file_path: row.get(8)?,
                analysis_path: row.get(9)?,
                master_database_id: checked_u32(row.get::<_, i64>(10)?, 10)?,
                master_content_id: checked_u32(row.get::<_, i64>(11)?, 11)?,
                analysis_update_count: checked_u32(row.get::<_, i64>(12)?, 12)?,
                information_update_count: checked_u32(row.get::<_, i64>(13)?, 13)?,
                cue_update_count: checked_u32(row.get::<_, i64>(14)?, 14)?,
            })
        })
        .map_err(database_error)?;
    for row in rows {
        let track = row.map_err(database_error)?;
        let audio_path = canonical_declared_child(&canonical_root, &track.file_path)?;
        let analysis_dat_path = canonical_declared_child(&canonical_root, &track.analysis_path)?;
        let analysis_dat_revision = sha256_file(&analysis_dat_path, MAX_ANALYSIS_FILE_BYTES)?;
        let update_identity = format!(
            "{}:{}:{}:{}:{}",
            track.master_database_id,
            track.master_content_id,
            track.analysis_update_count,
            track.information_update_count,
            track.cue_update_count
        );
        let metadata_revision = metadata_revision(MetadataRevisionInput {
            id: track.id,
            title: &track.title,
            artist: &track.artist,
            color: &track.color_name,
            tempo: track.bpm_centi,
            duration: track.duration,
            file_size: track.file_size,
            file_path: &track.file_path,
            analyze_path: &track.analysis_path,
            analyze_date: &update_identity,
        });
        let analysis_revision = format!(
            "onelibrary:{}:{}:{}:{}:dat:{}",
            track.master_database_id,
            track.master_content_id,
            track.analysis_update_count,
            track.cue_update_count,
            analysis_dat_revision,
        );
        let device_track = DeviceTrack {
            device_track_id: track.id,
            title: track.title.clone(),
            artist: track.artist.clone(),
            musical_key: track.musical_key,
            color_rgb: rekordbox_track_color_rgb(&track.color_name),
            bpm_milli: track.bpm_centi.saturating_mul(10),
            duration_millis: onelibrary_duration_millis(track.duration),
            file_size: track.file_size,
            audio_path,
            analysis_dat_path,
            metadata_revision,
            analysis_revision,
            analyzed_at: exported_at.clone(),
            audio_signature: String::new(),
            simulator_signature: simulator_signature(
                &track.title,
                &track.artist,
                track.bpm_centi,
                u16::try_from(track.duration).unwrap_or(u16::MAX),
            ),
            master_database_id: track.master_database_id,
            master_content_id: track.master_content_id,
            analysis_update_count: track.analysis_update_count,
            information_update_count: track.information_update_count,
            cue_update_count: track.cue_update_count,
        };
        if tracks.insert(track.id, device_track).is_some() {
            return Err(DeviceError::DuplicateTrackId(track.id));
        }
    }
    drop(statement);
    let playlists = device_playlists(&database, tracks.keys().copied().collect())?;

    let display_name = canonical_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            (!exported_device_name.trim().is_empty()).then_some(exported_device_name.as_str())
        })
        .unwrap_or("Rekordbox OneLibrary")
        .to_owned();
    let source_id = format!("rekordbox-device:{}", normalize_identity(&display_name));
    Ok(DeviceLibrarySnapshot {
        source_id,
        display_name,
        database_path,
        database_revision,
        database_version,
        exported_at,
        tracks,
        playlists,
    })
}

fn device_playlists(
    database: &Connection,
    known_track_ids: BTreeSet<u32>,
) -> Result<Vec<DevicePlaylist>, DeviceError> {
    let mut statement = database
        .prepare(
            "SELECT playlist_id, COALESCE(name, ''), COALESCE(attribute, 0),
                    playlist_id_parent FROM playlist ORDER BY sequenceNo, playlist_id",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(OneLibraryPlaylistNode {
                id: checked_u32(row.get::<_, i64>(0)?, 0)?,
                name: row.get(1)?,
                is_folder: row.get::<_, i64>(2)? == 1,
                parent_id: row
                    .get::<_, Option<i64>>(3)?
                    .map(|value| checked_u32(value, 3))
                    .transpose()?,
            })
        })
        .map_err(database_error)?;
    let node_values = rows
        .map(|row| row.map_err(database_error))
        .collect::<Result<Vec<_>, _>>()?;
    let nodes = node_values
        .iter()
        .map(|node| (node.id, node))
        .collect::<BTreeMap<_, _>>();
    if nodes.len() != node_values.len() {
        return Err(DeviceError::InvalidPlaylistTree(
            "duplicate playlist id".to_owned(),
        ));
    }
    let mut entries = BTreeMap::<u32, Vec<(u32, u32)>>::new();
    drop(statement);
    let mut statement = database
        .prepare(
            "SELECT playlist_id, content_id, COALESCE(sequenceNo, 0)
               FROM playlist_content ORDER BY playlist_id, sequenceNo",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                checked_u32(row.get::<_, i64>(0)?, 0)?,
                checked_u32(row.get::<_, i64>(1)?, 1)?,
                checked_u32(row.get::<_, i64>(2)?, 2)?,
            ))
        })
        .map_err(database_error)?;
    for entry in rows {
        let (playlist_id, track_id, position) = entry.map_err(database_error)?;
        if !known_track_ids.contains(&track_id) {
            return Err(DeviceError::InvalidPlaylistTree(format!(
                "playlist {} references missing track {}",
                playlist_id, track_id
            )));
        }
        entries
            .entry(playlist_id)
            .or_default()
            .push((position, track_id));
    }
    let mut playlists = node_values
        .iter()
        .filter(|node| !node.is_folder)
        .map(|node| {
            let mut segments = vec![node.name.trim().to_owned()];
            let mut parent_id = node.parent_id.unwrap_or(0);
            let mut visited = BTreeSet::from([node.id]);
            while parent_id != 0 {
                if !visited.insert(parent_id) {
                    return Err(DeviceError::InvalidPlaylistTree(format!(
                        "cycle at playlist node {parent_id}"
                    )));
                }
                let parent = nodes.get(&parent_id).ok_or_else(|| {
                    DeviceError::InvalidPlaylistTree(format!(
                        "missing parent {parent_id} for playlist {}",
                        node.id
                    ))
                })?;
                if !parent.is_folder {
                    return Err(DeviceError::InvalidPlaylistTree(format!(
                        "playlist {} has non-folder parent {parent_id}",
                        node.id
                    )));
                }
                segments.push(parent.name.trim().to_owned());
                parent_id = parent.parent_id.unwrap_or(0);
                if segments.len() > 32 {
                    return Err(DeviceError::InvalidPlaylistTree(
                        "playlist nesting exceeds 32 levels".to_owned(),
                    ));
                }
            }
            segments.reverse();
            let mut playlist_entries = entries.remove(&node.id).unwrap_or_default();
            playlist_entries.sort_by_key(|(position, _)| *position);
            let mut seen = BTreeSet::new();
            let track_ids = playlist_entries
                .into_iter()
                .filter_map(|(_, track_id)| seen.insert(track_id).then_some(track_id))
                .collect::<Vec<_>>();
            Ok(DevicePlaylist {
                device_playlist_id: node.id,
                path: segments.join("/"),
                name: node.name.trim().to_owned(),
                track_ids,
            })
        })
        .collect::<Result<Vec<_>, DeviceError>>()?;
    if let Some(playlist_id) = entries.keys().next() {
        return Err(DeviceError::InvalidPlaylistTree(format!(
            "entries reference missing or folder playlist {playlist_id}"
        )));
    }
    playlists.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(playlists)
}

fn open_onelibrary(path: &Path) -> Result<Connection, DeviceError> {
    let database = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(database_error)?;
    database
        .execute_batch(&format!(
            "PRAGMA key = '{ONELIBRARY_FORMAT_KEY}';\
             PRAGMA cipher_compatibility = 4;\
             PRAGMA query_only = ON;"
        ))
        .map_err(database_error)?;
    let property_rows = database
        .query_row("SELECT COUNT(*) FROM property", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(database_error)?;
    if property_rows != 1 {
        return Err(DeviceError::UnsupportedOneLibrarySchema);
    }
    Ok(database)
}

fn database_error(error: rusqlite::Error) -> DeviceError {
    DeviceError::Database(error.to_string())
}

fn checked_u32(value: i64, column: usize) -> Result<u32, rusqlite::Error> {
    u32::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(column, value))
}

const fn onelibrary_duration_millis(value: u32) -> u32 {
    if value > 86_400 {
        value
    } else {
        value.saturating_mul(1_000)
    }
}

struct OneLibraryTrackRow {
    id: u32,
    title: String,
    artist: String,
    musical_key: String,
    color_name: String,
    bpm_centi: u32,
    duration: u32,
    file_size: u32,
    file_path: String,
    analysis_path: String,
    master_database_id: u32,
    master_content_id: u32,
    analysis_update_count: u32,
    information_update_count: u32,
    cue_update_count: u32,
}

struct OneLibraryPlaylistNode {
    id: u32,
    name: String,
    is_folder: bool,
    parent_id: Option<u32>,
}

/// Hashes the file size plus at most 64 KiB from the start and end of an audio
/// file. This is bounded, read-only and stable across USB copies.
pub fn audio_content_signature(path: impl AsRef<Path>) -> Result<String, DeviceError> {
    let path = path.as_ref();
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(DeviceError::InvalidAudioFile(path.to_path_buf()));
    }
    let length = metadata.len();
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    digest.update(length.to_le_bytes());
    let start_length = usize::try_from(length.min(AUDIO_SIGNATURE_WINDOW_BYTES as u64))
        .map_err(|_| DeviceError::InvalidAudioFile(path.to_path_buf()))?;
    let mut start = vec![0_u8; start_length];
    file.read_exact(&mut start)?;
    digest.update(&start);
    if length > AUDIO_SIGNATURE_WINDOW_BYTES as u64 {
        let end_length = usize::try_from(length.min(AUDIO_SIGNATURE_WINDOW_BYTES as u64))
            .map_err(|_| DeviceError::InvalidAudioFile(path.to_path_buf()))?;
        file.seek(SeekFrom::End(-(end_length as i64)))?;
        let mut end = vec![0_u8; end_length];
        file.read_exact(&mut end)?;
        digest.update(&end);
    }
    Ok(format!(
        "audio:{}",
        hex_digest(digest.finalize().as_slice())
    ))
}

/// Returns the Java `String.hashCode` of the normalized metadata tuple sent
/// by Lumi's BLT protocol v4 expression during shallow simulation.
#[must_use]
pub fn simulator_signature(title: &str, artist: &str, tempo_centi_bpm: u32, duration: u16) -> u32 {
    let identity = format!(
        "{}\u{1f}{}\u{1f}{tempo_centi_bpm}\u{1f}{duration}",
        normalize_match_text(title),
        normalize_match_text(artist),
    );
    identity.encode_utf16().fold(0_u32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(u32::from(unit))
    })
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_lowercase()
}

fn canonical_declared_child(root: &Path, declared_path: &str) -> Result<PathBuf, DeviceError> {
    let relative = Path::new(declared_path.trim_start_matches('/'));
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DeviceError::UnsafeDeclaredPath(declared_path.to_owned()));
    }
    canonical_child(root, relative)
}

fn canonical_child(root: &Path, relative: &Path) -> Result<PathBuf, DeviceError> {
    let candidate = root.join(relative);
    let canonical = fs::canonicalize(&candidate)?;
    if !canonical.starts_with(root) {
        return Err(DeviceError::PathEscapesDevice(candidate));
    }
    Ok(canonical)
}

struct MetadataRevisionInput<'a> {
    id: u32,
    title: &'a str,
    artist: &'a str,
    color: &'a str,
    tempo: u32,
    duration: u32,
    file_size: u32,
    file_path: &'a str,
    analyze_path: &'a str,
    analyze_date: &'a str,
}

fn metadata_revision(input: MetadataRevisionInput<'_>) -> String {
    let mut digest = Sha256::new();
    for value in [
        input.id.to_string(),
        input.title.to_owned(),
        input.artist.to_owned(),
        input.color.to_owned(),
        input.tempo.to_string(),
        input.duration.to_string(),
        input.file_size.to_string(),
        input.file_path.to_owned(),
        input.analyze_path.to_owned(),
        input.analyze_date.to_owned(),
    ] {
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    format!("onelibrary:{}", hex_digest(digest.finalize().as_slice()))
}

/// Rekordbox OneLibrary stores track colors as one of eight named catalog
/// entries rather than arbitrary RGB. Lumi projects those stable identities
/// to vivid RGB values so the same value can be persisted, rendered and used
/// by deterministic Light Plan rules.
pub const REKORDBOX_TRACK_COLORS: [(&str, u32); 8] = [
    ("Pink", 0xff_33_cc),
    ("Red", 0xff_33_33),
    ("Orange", 0xff_8c_1a),
    ("Yellow", 0xff_d6_00),
    ("Green", 0x32_d7_4b),
    ("Aqua", 0x32_d7_d5),
    ("Blue", 0x32_80_ff),
    ("Purple", 0xaf_52_de),
];

#[must_use]
pub fn rekordbox_track_color_rgb(name: &str) -> Option<u32> {
    REKORDBOX_TRACK_COLORS
        .iter()
        .find(|(catalog_name, _)| catalog_name.eq_ignore_ascii_case(name.trim()))
        .map(|(_, rgb)| *rgb)
}

#[must_use]
pub const fn rekordbox_track_color_name(rgb: u32) -> Option<&'static str> {
    match rgb {
        0xff_33_cc => Some("Pink"),
        0xff_33_33 => Some("Red"),
        0xff_8c_1a => Some("Orange"),
        0xff_d6_00 => Some("Yellow"),
        0x32_d7_4b => Some("Green"),
        0x32_d7_d5 => Some("Aqua"),
        0x32_80_ff => Some("Blue"),
        0xaf_52_de => Some("Purple"),
        _ => None,
    }
}

fn sha256_file(path: &Path, maximum_bytes: u64) -> Result<String, DeviceError> {
    Ok(hex_digest(&sha256_file_bytes(path, maximum_bytes)?))
}

fn sha256_file_bytes(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, DeviceError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > maximum_bytes {
        return Err(DeviceError::FileTooLarge(path.to_path_buf()));
    }
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > maximum_bytes {
            return Err(DeviceError::FileTooLarge(path.to_path_buf()));
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().to_vec())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn normalize_identity(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("the selected folder is not a mounted device root")]
    InvalidDeviceRoot,
    #[error(
        "the OneLibrary database is missing, not a file, or exceeds its safety limit; export this USB with a current rekordbox OneLibrary version"
    )]
    InvalidDatabaseFile,
    #[error("OneLibrary read failed: {0}")]
    Database(String),
    #[error("the USB does not contain the supported current OneLibrary schema")]
    UnsupportedOneLibrarySchema,
    #[error("device contains duplicate track id {0}")]
    DuplicateTrackId(u32),
    #[error("invalid OneLibrary playlist tree: {0}")]
    InvalidPlaylistTree(String),
    #[error("declared device path is unsafe: {0}")]
    UnsafeDeclaredPath(String),
    #[error("resolved path escapes the selected device: {0}")]
    PathEscapesDevice(PathBuf),
    #[error("analysis set is missing for {0}")]
    MissingAnalysisSet(PathBuf),
    #[error("analysis file exceeds its safety limit: {0}")]
    AnalysisFileTooLarge(PathBuf),
    #[error("audio file is missing or invalid: {0}")]
    InvalidAudioFile(PathBuf),
    #[error("file exceeds its safety limit: {0}")]
    FileTooLarge(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDevice(PathBuf);

    impl Drop for TestDevice {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn current_onelibrary_is_read_only_and_preserves_playlist_order() -> Result<(), DeviceError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let device = TestDevice(
            std::env::temp_dir().join(format!("lumi-onelibrary-{}-{nonce}", std::process::id())),
        );
        let database_directory = device.0.join("PIONEER/rekordbox");
        let analysis_directory = device.0.join("PIONEER/USBANLZ/000/ABCDEF");
        let audio_directory = device.0.join("Contents");
        fs::create_dir_all(&database_directory)?;
        fs::create_dir_all(&analysis_directory)?;
        fs::create_dir_all(&audio_directory)?;
        fs::write(audio_directory.join("track.wav"), b"audio")?;
        fs::write(analysis_directory.join("ANLZ0000.DAT"), b"analysis")?;
        let database_path = database_directory.join("exportLibrary.db");
        let database = Connection::open(&database_path).map_err(database_error)?;
        database
            .execute_batch(&format!(
                "PRAGMA key = '{ONELIBRARY_FORMAT_KEY}';
                 PRAGMA cipher_compatibility = 4;
                 CREATE TABLE property(deviceName TEXT, dbVersion TEXT, numberOfContents INTEGER, createdDate TEXT);
                 INSERT INTO property VALUES ('TEST USB', '1000', 1, '2026-08-10');
                 CREATE TABLE artist(artist_id INTEGER PRIMARY KEY, name TEXT);
                 INSERT INTO artist VALUES (2, 'Artist');
                 CREATE TABLE key(key_id INTEGER PRIMARY KEY, name TEXT);
                 INSERT INTO key VALUES (3, '8A');
                 CREATE TABLE color(color_id INTEGER PRIMARY KEY, name TEXT);
                 INSERT INTO color VALUES (7, 'Blue');
                 CREATE TABLE content(
                    content_id INTEGER PRIMARY KEY, title TEXT, artist_id_artist INTEGER,
                    key_id INTEGER, color_id INTEGER, bpmx100 INTEGER, length INTEGER, fileSize INTEGER,
                    path TEXT, analysisDataFilePath TEXT, masterDbId INTEGER,
                    masterContentId INTEGER, analysisDataUpdateCount INTEGER,
                    informationUpdateCount INTEGER, cueUpdateCount TEXT
                 );
                 INSERT INTO content VALUES (
                    10, 'Track', 2, 3, 7, 12800, 180, 5,
                    '/Contents/track.wav', '/PIONEER/USBANLZ/000/ABCDEF/ANLZ0000.DAT',
                    20, 30, 4, 5, '6'
                 );
                 CREATE TABLE playlist(
                    playlist_id INTEGER PRIMARY KEY, sequenceNo INTEGER, name TEXT,
                    image_id INTEGER, attribute INTEGER, playlist_id_parent INTEGER
                 );
                 INSERT INTO playlist VALUES (40, 1, 'Folder', NULL, 1, NULL);
                 INSERT INTO playlist VALUES (41, 2, 'Set', NULL, 0, 40);
                 CREATE TABLE playlist_content(playlist_id INTEGER, content_id INTEGER, sequenceNo INTEGER);
                 INSERT INTO playlist_content VALUES (41, 10, 1);"
            ))
            .map_err(database_error)?;
        drop(database);
        let before = sha256_file(&database_path, MAX_DATABASE_BYTES)?;
        let snapshot = read_device_library(&device.0)?;
        let after = sha256_file(&database_path, MAX_DATABASE_BYTES)?;
        assert_eq!(before, after);
        assert_eq!(snapshot.database_version, "1000");
        assert_eq!(snapshot.tracks[&10].analysis_update_count, 4);
        assert_eq!(snapshot.tracks[&10].color_rgb, Some(0x32_80_ff));
        assert_eq!(snapshot.playlists[0].path, "Folder/Set");
        assert_eq!(snapshot.playlists[0].track_ids, vec![10]);
        let original_analysis_revision = snapshot.tracks[&10].analysis_revision.clone();
        assert!(original_analysis_revision.starts_with("onelibrary:20:30:4:6:dat:"));

        // Some Rekordbox exports update the authoritative DAT beatgrid without
        // advancing the OneLibrary counters. Content identity must still make
        // the next Lumi inspection classify the track as changed.
        fs::write(analysis_directory.join("ANLZ0000.DAT"), b"updated analysis")?;
        let updated = read_device_library(&device.0)?;
        assert_eq!(updated.tracks[&10].analysis_update_count, 4);
        assert_eq!(updated.tracks[&10].cue_update_count, 6);
        assert_ne!(
            updated.tracks[&10].analysis_revision,
            original_analysis_revision
        );
        Ok(())
    }

    #[test]
    fn maps_the_fixed_rekordbox_track_color_catalog() {
        let expected = [
            ("Pink", 0xff_33_cc),
            ("Red", 0xff_33_33),
            ("Orange", 0xff_8c_1a),
            ("Yellow", 0xff_d6_00),
            ("Green", 0x32_d7_4b),
            ("Aqua", 0x32_d7_d5),
            ("Blue", 0x32_80_ff),
            ("Purple", 0xaf_52_de),
        ];
        for (name, rgb) in expected {
            assert_eq!(rekordbox_track_color_rgb(name), Some(rgb));
            assert_eq!(rekordbox_track_color_name(rgb), Some(name));
        }
        assert_eq!(rekordbox_track_color_rgb(""), None);
    }
}
