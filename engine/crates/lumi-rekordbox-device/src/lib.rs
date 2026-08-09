//! Read-only adapter for Rekordbox Device Library (DeviceSQL) media.
//!
//! This crate deliberately exposes no write operation. A sync parses the
//! `PIONEER/rekordbox/export.pdb` database and fingerprints every analysis
//! companion referenced by a track. Consequently beat-grid and cue changes
//! produce a new track analysis revision on the next sync.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use rekordbox_pdb::Database;
use sha2::{Digest, Sha256};
use thiserror::Error;

const DATABASE_RELATIVE_PATH: &str = "PIONEER/rekordbox/export.pdb";
const MAX_DATABASE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ANALYSIS_FILE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceTrack {
    pub device_track_id: u32,
    pub title: String,
    pub artist: String,
    pub musical_key: String,
    pub bpm_milli: u32,
    pub duration_millis: u32,
    pub file_size: u32,
    pub audio_path: PathBuf,
    pub analysis_dat_path: PathBuf,
    pub metadata_revision: String,
    pub analysis_revision: String,
    /// Metadata fallback used only by BLT's shallow simulator, whose
    /// `rekordbox-id` is intentionally hard-coded to 42 for every track.
    pub simulator_signature: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLibrarySnapshot {
    pub source_id: String,
    pub display_name: String,
    pub database_path: PathBuf,
    pub database_revision: String,
    pub tracks: BTreeMap<u32, DeviceTrack>,
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
    let database = Database::from_file(&database_path)
        .map_err(|error| DeviceError::Database(error.to_string()))?;

    let artists = database
        .artists
        .iter()
        .map(|artist| (artist.id, artist.name.as_str()))
        .collect::<BTreeMap<_, _>>();
    let keys = database
        .keys
        .iter()
        .map(|key| (key.id, key.name.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut tracks = BTreeMap::new();
    for track in &database.tracks {
        let audio_path = canonical_declared_child(&canonical_root, track.file_path())?;
        let analysis_dat_path = canonical_declared_child(&canonical_root, track.analyze_path())?;
        let metadata_revision = metadata_revision(MetadataRevisionInput {
            id: track.id,
            title: track.title(),
            artist: artists.get(&track.artist_id).copied().unwrap_or_default(),
            tempo: track.tempo,
            duration: track.duration,
            file_size: track.file_size,
            file_path: track.file_path(),
            analyze_path: track.analyze_path(),
            analyze_date: track.analyze_date(),
        });
        let analysis_revision = analysis_set_revision(&analysis_dat_path)?;
        let device_track = DeviceTrack {
            device_track_id: track.id,
            title: track.title().to_owned(),
            artist: artists
                .get(&track.artist_id)
                .copied()
                .unwrap_or_default()
                .to_owned(),
            musical_key: keys
                .get(&track.key_id)
                .copied()
                .unwrap_or_default()
                .to_owned(),
            bpm_milli: track.tempo.saturating_mul(10),
            duration_millis: u32::from(track.duration).saturating_mul(1_000),
            file_size: track.file_size,
            audio_path,
            analysis_dat_path,
            metadata_revision,
            analysis_revision,
            simulator_signature: simulator_signature(
                track.title(),
                artists.get(&track.artist_id).copied().unwrap_or_default(),
                track.tempo,
                track.duration,
            ),
        };
        if tracks.insert(track.id, device_track).is_some() {
            return Err(DeviceError::DuplicateTrackId(track.id));
        }
    }

    let display_name = canonical_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Rekordbox Device")
        .to_owned();
    let source_id = format!("rekordbox-device:{}", normalize_identity(&display_name));
    Ok(DeviceLibrarySnapshot {
        source_id,
        display_name,
        database_path,
        database_revision,
        tracks,
    })
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
    tempo: u32,
    duration: u16,
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
    format!("devicesql:{}", hex_digest(digest.finalize().as_slice()))
}

fn analysis_set_revision(dat_path: &Path) -> Result<String, DeviceError> {
    let mut digest = Sha256::new();
    let mut found = 0_u8;
    for extension in ["DAT", "EXT", "2EX"] {
        let path = dat_path.with_extension(extension);
        if !path.is_file() {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_ANALYSIS_FILE_BYTES {
            return Err(DeviceError::AnalysisFileTooLarge(path));
        }
        digest.update(extension.as_bytes());
        digest.update(metadata.len().to_le_bytes());
        digest.update(sha256_file_bytes(&path, MAX_ANALYSIS_FILE_BYTES)?);
        found = found.saturating_add(1);
    }
    if found == 0 {
        return Err(DeviceError::MissingAnalysisSet(dat_path.to_path_buf()));
    }
    digest.update([found]);
    Ok(format!("anlz:{}", hex_digest(digest.finalize().as_slice())))
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
    #[error("the DeviceSQL database is missing, not a file, or exceeds its safety limit")]
    InvalidDatabaseFile,
    #[error("DeviceSQL parse failed: {0}")]
    Database(String),
    #[error("device contains duplicate track id {0}")]
    DuplicateTrackId(u32),
    #[error("declared device path is unsafe: {0}")]
    UnsafeDeclaredPath(String),
    #[error("resolved path escapes the selected device: {0}")]
    PathEscapesDevice(PathBuf),
    #[error("analysis set is missing for {0}")]
    MissingAnalysisSet(PathBuf),
    #[error("analysis file exceeds its safety limit: {0}")]
    AnalysisFileTooLarge(PathBuf),
    #[error("file exceeds its safety limit: {0}")]
    FileTooLarge(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}
