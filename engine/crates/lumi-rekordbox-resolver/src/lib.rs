//! Stable Rekordbox track identity to ANLZ resolution through a read-only database snapshot.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};
use thiserror::Error;

const MAXIMUM_REQUESTED_TRACKS: usize = 100_000;
const QUERY_CHUNK_SIZE: usize = 500;

pub struct DatabaseKey {
    bytes: [u8; 128],
    len: usize,
}

impl DatabaseKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ResolverError> {
        let value = value.into();
        if value.len() < 32
            || value.len() > 128
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ResolverError::InvalidDatabaseKey);
        }
        let mut bytes = [0_u8; 128];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self {
            bytes,
            len: value.len(),
        })
    }

    fn expose(&self) -> Result<&str, ResolverError> {
        std::str::from_utf8(&self.bytes[..self.len]).map_err(|_| ResolverError::InvalidDatabaseKey)
    }
}

impl Drop for DatabaseKey {
    fn drop(&mut self) {
        self.bytes.fill(0);
        self.len = 0;
    }
}

impl fmt::Debug for DatabaseKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DatabaseKey([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseSnapshot {
    path: PathBuf,
    sha256: String,
}

impl DatabaseSnapshot {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Copies a closed Rekordbox database into a new Lumi-owned file and proves the
/// source did not change during the copy. Live WAL/journal sidecars fail closed.
pub fn create_database_snapshot(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<DatabaseSnapshot, ResolverError> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    if !source.is_file() || destination.exists() {
        return Err(ResolverError::InvalidSnapshotPath);
    }
    let source = fs::canonicalize(source)?;
    let destination_parent = destination
        .parent()
        .ok_or(ResolverError::InvalidSnapshotPath)?;
    let destination_parent = fs::canonicalize(destination_parent)?;
    let destination = destination_parent.join(
        destination
            .file_name()
            .ok_or(ResolverError::InvalidSnapshotPath)?,
    );
    if source == destination || live_sidecar_exists(&source) {
        return Err(ResolverError::SourceDatabaseMayBeLive);
    }

    let before = fs::metadata(&source)?;
    let mut input = File::open(&source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)?;
    if let Err(error) = io::copy(&mut input, &mut output).and_then(|_| output.sync_all()) {
        let _ = fs::remove_file(&destination);
        return Err(error.into());
    }
    let after = fs::metadata(&source)?;
    let snapshot_hash = sha256_file(&destination)?;
    let source_hash = sha256_file(&source)?;
    if before.len() != after.len()
        || before.modified()? != after.modified()?
        || source_hash != snapshot_hash
        || live_sidecar_exists(&source)
    {
        let _ = fs::remove_file(&destination);
        return Err(ResolverError::SourceChangedDuringSnapshot);
    }
    let mut permissions = fs::metadata(&destination)?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&destination, permissions)?;
    Ok(DatabaseSnapshot {
        path: destination,
        sha256: snapshot_hash,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedTrack {
    source_track_id: String,
    expected_audio_path: PathBuf,
}

impl RequestedTrack {
    pub fn try_new(
        source_track_id: impl Into<String>,
        expected_audio_location: impl Into<String>,
    ) -> Result<Self, ResolverError> {
        let source_track_id = source_track_id.into();
        if source_track_id.is_empty()
            || source_track_id.len() > 20
            || !source_track_id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ResolverError::InvalidSourceTrackId);
        }
        let expected_audio_path = decode_audio_location(&expected_audio_location.into())?;
        if !expected_audio_path.is_absolute() {
            return Err(ResolverError::InvalidAudioPath);
        }
        Ok(Self {
            source_track_id,
            expected_audio_path,
        })
    }

    #[must_use]
    pub fn source_track_id(&self) -> &str {
        &self.source_track_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTrack {
    source_track_id: String,
    database_audio_path: PathBuf,
    analysis_file: PathBuf,
    audio_path_matches: bool,
}

impl ResolvedTrack {
    #[must_use]
    pub fn source_track_id(&self) -> &str {
        &self.source_track_id
    }

    #[must_use]
    pub fn database_audio_path(&self) -> &Path {
        &self.database_audio_path
    }

    #[must_use]
    pub fn analysis_file(&self) -> &Path {
        &self.analysis_file
    }

    #[must_use]
    pub const fn audio_path_matches(&self) -> bool {
        self.audio_path_matches
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResolveReport {
    pub requested_tracks: usize,
    pub resolved_tracks: usize,
    pub missing_database_rows: usize,
    pub missing_analysis_paths: usize,
    pub audio_path_mismatches: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveResult {
    pub report: ResolveReport,
    pub tracks: BTreeMap<String, ResolvedTrack>,
}

#[derive(Debug)]
pub struct SqlCipherResolver {
    executable: PathBuf,
}

impl SqlCipherResolver {
    pub fn try_new(executable: impl Into<PathBuf>) -> Result<Self, ResolverError> {
        let executable = executable.into();
        if !executable.is_file() {
            return Err(ResolverError::SqlCipherUnavailable);
        }
        Ok(Self { executable })
    }

    pub fn resolve(
        &self,
        snapshot: &DatabaseSnapshot,
        key: &DatabaseKey,
        analysis_root: impl AsRef<Path>,
        requested_tracks: impl IntoIterator<Item = RequestedTrack>,
    ) -> Result<ResolveResult, ResolverError> {
        let requested = requested_tracks
            .into_iter()
            .map(|track| (track.source_track_id.clone(), track))
            .collect::<BTreeMap<_, _>>();
        if requested.is_empty() || requested.len() > MAXIMUM_REQUESTED_TRACKS {
            return Err(ResolverError::InvalidRequestedTrackCount(requested.len()));
        }
        let analysis_root = fs::canonicalize(analysis_root)?;
        if !analysis_root.is_dir() {
            return Err(ResolverError::InvalidAnalysisRoot);
        }
        let rows = self.query(snapshot, key, requested.keys())?;
        let mut report = ResolveReport {
            requested_tracks: requested.len(),
            ..ResolveReport::default()
        };
        let mut tracks = BTreeMap::new();
        for (source_track_id, requested_track) in requested {
            let Some(row) = rows.get(&source_track_id) else {
                report.missing_database_rows += 1;
                continue;
            };
            let Some(analysis_relative_path) = row.analysis_relative_path.as_deref() else {
                report.missing_analysis_paths += 1;
                continue;
            };
            let Some(analysis_file) =
                resolve_analysis_file(&analysis_root, analysis_relative_path)?
            else {
                report.missing_analysis_paths += 1;
                continue;
            };
            let database_audio_path = PathBuf::from(&row.audio_path);
            let audio_path_matches = normalized_path(&database_audio_path)
                == normalized_path(&requested_track.expected_audio_path);
            if !audio_path_matches {
                report.audio_path_mismatches += 1;
            }
            tracks.insert(
                source_track_id.clone(),
                ResolvedTrack {
                    source_track_id,
                    database_audio_path,
                    analysis_file,
                    audio_path_matches,
                },
            );
        }
        report.resolved_tracks = tracks.len();
        Ok(ResolveResult { report, tracks })
    }

    fn query<'a>(
        &self,
        snapshot: &DatabaseSnapshot,
        key: &DatabaseKey,
        requested_ids: impl Iterator<Item = &'a String>,
    ) -> Result<BTreeMap<String, DatabaseRow>, ResolverError> {
        let ids = requested_ids.cloned().collect::<Vec<_>>();
        let mut child = Command::new(&self.executable)
            .arg("-readonly")
            .arg("-batch")
            .arg(snapshot.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| ResolverError::SqlCipherUnavailable)?;
        let mut stdin = child.stdin.take().ok_or(ResolverError::SqlCipherProtocol)?;
        stdin.write_all(b".bail on\n.headers off\n.mode tabs\nPRAGMA key='")?;
        stdin.write_all(key.expose()?.as_bytes())?;
        stdin.write_all(b"';\nPRAGMA query_only=ON;\n")?;
        for chunk in ids.chunks(QUERY_CHUNK_SIZE) {
            stdin.write_all(b"SELECT CAST(ID AS TEXT), hex(CAST(FolderPath AS BLOB)), ")?;
            stdin.write_all(
                b"hex(CAST(COALESCE(AnalysisDataPath, '') AS BLOB)) FROM djmdContent ",
            )?;
            stdin.write_all(b"WHERE rb_local_deleted=0 AND ID IN (")?;
            stdin.write_all(chunk.join(",").as_bytes())?;
            stdin.write_all(b") ORDER BY ID;\n")?;
        }
        drop(stdin);
        let output = child.wait_with_output()?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(ResolverError::SqlCipherRejectedSnapshot);
        }
        parse_rows(&output.stdout)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DatabaseRow {
    audio_path: String,
    analysis_relative_path: Option<String>,
}

fn parse_rows(bytes: &[u8]) -> Result<BTreeMap<String, DatabaseRow>, ResolverError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ResolverError::SqlCipherProtocol)?;
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        // SQLCipher acknowledges a successful key pragma with this fixed token.
        if line == "ok" {
            continue;
        }
        let mut columns = line.split('\t');
        let id = columns.next().ok_or(ResolverError::SqlCipherProtocol)?;
        let audio_hex = columns.next().ok_or(ResolverError::SqlCipherProtocol)?;
        let analysis_hex = columns.next().ok_or(ResolverError::SqlCipherProtocol)?;
        if columns.next().is_some()
            || id.is_empty()
            || !id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ResolverError::SqlCipherProtocol);
        }
        let audio_path = decode_hex(audio_hex)?;
        let analysis_relative_path = if analysis_hex.is_empty() {
            None
        } else {
            Some(decode_hex(analysis_hex)?)
        };
        if rows
            .insert(
                id.to_owned(),
                DatabaseRow {
                    audio_path,
                    analysis_relative_path,
                },
            )
            .is_some()
        {
            return Err(ResolverError::DuplicateDatabaseIdentity);
        }
    }
    Ok(rows)
}

fn decode_hex(value: &str) -> Result<String, ResolverError> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ResolverError::SqlCipherProtocol);
    }
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).map_err(|_| ResolverError::SqlCipherProtocol)?;
            u8::from_str_radix(pair, 16).map_err(|_| ResolverError::SqlCipherProtocol)
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|_| ResolverError::SqlCipherProtocol)
}

fn resolve_analysis_file(
    analysis_root: &Path,
    value: &str,
) -> Result<Option<PathBuf>, ResolverError> {
    let normalized = value.replace('\\', "/");
    let normalized = normalized
        .strip_prefix("share/")
        .unwrap_or(normalized.as_str())
        .trim_start_matches('/');
    let relative = Path::new(normalized);
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ResolverError::UnsafeAnalysisPath);
    }
    let candidate = analysis_root.join(relative);
    if !candidate.is_file() {
        return Ok(None);
    }
    let candidate = fs::canonicalize(candidate)?;
    if !candidate.starts_with(analysis_root) {
        return Err(ResolverError::UnsafeAnalysisPath);
    }
    Ok(Some(candidate))
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn decode_audio_location(value: &str) -> Result<PathBuf, ResolverError> {
    let path = value
        .strip_prefix("file://localhost")
        .or_else(|| value.strip_prefix("file://"))
        .unwrap_or(value);
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes
                .get(index + 1)
                .ok_or(ResolverError::InvalidAudioPath)?;
            let low = *bytes
                .get(index + 2)
                .ok_or(ResolverError::InvalidAudioPath)?;
            decoded.push(
                hex_nibble(high)
                    .and_then(|value| value.checked_mul(16))
                    .and_then(|value| hex_nibble(low).and_then(|low| value.checked_add(low)))
                    .ok_or(ResolverError::InvalidAudioPath)?,
            );
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| ResolverError::InvalidAudioPath)?;
    Ok(PathBuf::from(decoded))
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn live_sidecar_exists(source: &Path) -> bool {
    let Some(name) = source.file_name().and_then(|value| value.to_str()) else {
        return true;
    };
    ["-wal", "-shm", "-journal"]
        .iter()
        .any(|suffix| source.with_file_name(format!("{name}{suffix}")).exists())
}

fn sha256_file(path: &Path) -> Result<String, ResolverError> {
    let mut input = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1_024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("invalid database key")]
    InvalidDatabaseKey,
    #[error("invalid source track identity")]
    InvalidSourceTrackId,
    #[error("invalid absolute audio path")]
    InvalidAudioPath,
    #[error("invalid database snapshot path")]
    InvalidSnapshotPath,
    #[error("Rekordbox database may be live; close Rekordbox before snapshotting")]
    SourceDatabaseMayBeLive,
    #[error("source database changed while creating the snapshot")]
    SourceChangedDuringSnapshot,
    #[error("invalid requested track count: {0}")]
    InvalidRequestedTrackCount(usize),
    #[error("invalid Rekordbox analysis root")]
    InvalidAnalysisRoot,
    #[error("unsafe Rekordbox analysis path")]
    UnsafeAnalysisPath,
    #[error("SQLCipher executable is unavailable")]
    SqlCipherUnavailable,
    #[error("SQLCipher rejected the read-only snapshot")]
    SqlCipherRejectedSnapshot,
    #[error("unexpected SQLCipher output")]
    SqlCipherProtocol,
    #[error("duplicate Rekordbox database identity")]
    DuplicateDatabaseIdentity,
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{DatabaseKey, ResolverError, create_database_snapshot, parse_rows};

    #[test]
    fn key_debug_output_is_redacted() -> Result<(), Box<dyn Error>> {
        let key = DatabaseKey::try_new("a".repeat(64))?;
        assert_eq!(format!("{key:?}"), "DatabaseKey([REDACTED])");
        Ok(())
    }

    #[test]
    fn snapshot_is_identical_and_read_only() -> Result<(), Box<dyn Error>> {
        let root = temp_root("snapshot")?;
        fs::create_dir_all(&root)?;
        let source = root.join("master.db");
        let destination = root.join("snapshot.db");
        fs::write(&source, b"immutable-source")?;
        let snapshot = create_database_snapshot(&source, &destination)?;
        assert_eq!(fs::read(snapshot.path())?, b"immutable-source");
        assert!(fs::metadata(snapshot.path())?.permissions().readonly());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn snapshot_refuses_live_wal() -> Result<(), Box<dyn Error>> {
        let root = temp_root("wal")?;
        fs::create_dir_all(&root)?;
        let source = root.join("master.db");
        fs::write(&source, b"source")?;
        fs::write(root.join("master.db-wal"), b"wal")?;
        let Err(error) = create_database_snapshot(&source, root.join("snapshot.db")) else {
            return Err(io::Error::other("live source unexpectedly accepted").into());
        };
        assert!(matches!(error, ResolverError::SourceDatabaseMayBeLive));
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn database_rows_use_hex_encoded_paths() -> Result<(), Box<dyn Error>> {
        let rows =
            parse_rows(b"42\t2F4D757369632F746573742E6D7033\t50494F4E4545522F414E4C5A2E444154\n")?;
        let row = rows
            .get("42")
            .ok_or_else(|| io::Error::other("missing parsed row"))?;
        assert_eq!(row.audio_path, "/Music/test.mp3");
        assert_eq!(
            row.analysis_relative_path.as_deref(),
            Some("PIONEER/ANLZ.DAT")
        );
        Ok(())
    }

    fn temp_root(label: &str) -> Result<std::path::PathBuf, std::time::SystemTimeError> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(std::env::temp_dir().join(format!("lumi-rekordbox-resolver-{label}-{nanos}")))
    }
}
