//! Bounded, read-only Rekordbox XML playlist-source adapter.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAXIMUM_FILE_SIZE: u64 = 256 * 1_024 * 1_024;
const MAXIMUM_COLLECTION_TRACKS: usize = 1_000_000;
const MAXIMUM_PLAYLIST_NODES: usize = 20_000;
const MAXIMUM_TRACK_REFERENCES: usize = 1_000_000;
const MAXIMUM_DEPTH: usize = 32;
const MAXIMUM_NODE_NAME_BYTES: usize = 512;
const MAXIMUM_PATH_BYTES: usize = 2_048;
const MAXIMUM_TRACK_FIELD_BYTES: usize = 32 * 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RekordboxXmlSyncRequest {
    folder: PathBuf,
    followed_paths: BTreeSet<String>,
    include_future_child_playlists: bool,
}

impl RekordboxXmlSyncRequest {
    pub fn try_new(
        folder: impl Into<PathBuf>,
        followed_paths: impl IntoIterator<Item = String>,
        include_future_child_playlists: bool,
    ) -> Result<Self, RekordboxXmlError> {
        let folder = folder.into();
        let followed_paths = followed_paths
            .into_iter()
            .map(|path| path.trim().to_owned())
            .filter(|path| !path.is_empty())
            .collect::<BTreeSet<_>>();
        if followed_paths.is_empty() {
            return Err(RekordboxXmlError::NoFollowedPlaylists);
        }
        Ok(Self {
            folder,
            followed_paths,
            include_future_child_playlists,
        })
    }

    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RekordboxXmlMirrorSnapshot {
    export_path: PathBuf,
    content_sha256: String,
    product_version: String,
    collection_track_count: usize,
    playlists: Vec<RekordboxXmlPlaylist>,
    tracks: Vec<RekordboxXmlTrack>,
    diagnostics: RekordboxXmlDiagnostics,
    selection_paths: Vec<String>,
    include_future_child_playlists: bool,
}

impl RekordboxXmlMirrorSnapshot {
    #[must_use]
    pub fn export_path(&self) -> &Path {
        &self.export_path
    }
    #[must_use]
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }
    #[must_use]
    pub fn product_version(&self) -> &str {
        &self.product_version
    }
    #[must_use]
    pub const fn collection_track_count(&self) -> usize {
        self.collection_track_count
    }
    #[must_use]
    pub fn playlists(&self) -> &[RekordboxXmlPlaylist] {
        &self.playlists
    }
    #[must_use]
    pub fn tracks(&self) -> &[RekordboxXmlTrack] {
        &self.tracks
    }
    #[must_use]
    pub const fn diagnostics(&self) -> &RekordboxXmlDiagnostics {
        &self.diagnostics
    }
    #[must_use]
    pub fn selection_paths(&self) -> &[String] {
        &self.selection_paths
    }
    #[must_use]
    pub const fn include_future_child_playlists(&self) -> bool {
        self.include_future_child_playlists
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RekordboxXmlPlaylist {
    path: String,
    name: String,
    track_ids: Vec<String>,
}

impl RekordboxXmlPlaylist {
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn track_ids(&self) -> &[String] {
        &self.track_ids
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RekordboxXmlTrack {
    source_track_id: String,
    title: String,
    artist: Option<String>,
    average_bpm: Option<String>,
    tonality: Option<String>,
    total_time_seconds: Option<u64>,
    location: String,
    colour: Option<String>,
    tempo_marker_count: usize,
    position_mark_count: usize,
}

impl RekordboxXmlTrack {
    #[must_use]
    pub fn source_track_id(&self) -> &str {
        &self.source_track_id
    }
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    #[must_use]
    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }
    #[must_use]
    pub fn average_bpm(&self) -> Option<&str> {
        self.average_bpm.as_deref()
    }
    #[must_use]
    pub fn tonality(&self) -> Option<&str> {
        self.tonality.as_deref()
    }
    #[must_use]
    pub const fn total_time_seconds(&self) -> Option<u64> {
        self.total_time_seconds
    }
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }
    #[must_use]
    pub fn colour(&self) -> Option<&str> {
        self.colour.as_deref()
    }
    #[must_use]
    pub const fn tempo_marker_count(&self) -> usize {
        self.tempo_marker_count
    }
    #[must_use]
    pub const fn position_mark_count(&self) -> usize {
        self.position_mark_count
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RekordboxXmlDiagnostics {
    pub duplicate_playlist_references: usize,
    pub missing_artist: usize,
    pub missing_bpm: usize,
    pub missing_key: usize,
    pub missing_duration: usize,
    pub missing_beat_grid: usize,
    pub missing_colour: usize,
    pub missing_waveform: usize,
    pub missing_phrases: usize,
}

pub fn load_latest_mirror(
    request: &RekordboxXmlSyncRequest,
) -> Result<RekordboxXmlMirrorSnapshot, RekordboxXmlError> {
    let export_path = latest_export(request.folder())?;
    let metadata = fs::metadata(&export_path)?;
    if metadata.len() == 0 || metadata.len() > MAXIMUM_FILE_SIZE {
        return Err(RekordboxXmlError::InvalidFileSize(metadata.len()));
    }
    let bytes = fs::read(&export_path)?;
    let document: DjPlaylists = quick_xml::de::from_reader(bytes.as_slice())?;
    validate_document(&document)?;

    let mut node_count = 0_usize;
    let mut reference_count = 0_usize;
    let mut duplicate_playlist_references = 0_usize;
    let mut selected = Vec::new();
    for root in &document.playlists.nodes {
        collect_playlists(
            root,
            "",
            false,
            request,
            0,
            &mut node_count,
            &mut reference_count,
            &mut duplicate_playlist_references,
            &mut selected,
        )?;
    }
    if selected.is_empty() {
        return Err(RekordboxXmlError::NoMatchingPlaylists);
    }

    let collection = document
        .collection
        .tracks
        .into_iter()
        .map(|track| (track.track_id.clone(), track))
        .collect::<BTreeMap<_, _>>();
    if collection.len() != document.collection.entries {
        return Err(RekordboxXmlError::CollectionCountMismatch);
    }
    let selected_ids = selected
        .iter()
        .flat_map(|playlist| playlist.track_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut tracks = Vec::with_capacity(selected_ids.len());
    let mut diagnostics = RekordboxXmlDiagnostics {
        duplicate_playlist_references,
        ..RekordboxXmlDiagnostics::default()
    };
    for track_id in selected_ids {
        let track = collection
            .get(&track_id)
            .ok_or_else(|| RekordboxXmlError::MissingCollectionTrack(track_id.clone()))?;
        let normalized = normalize_track(track, &mut diagnostics)?;
        tracks.push(normalized);
    }
    tracks.sort_by(|left, right| left.source_track_id.cmp(&right.source_track_id));
    diagnostics.missing_waveform = tracks.len();
    diagnostics.missing_phrases = tracks.len();

    let content_sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(RekordboxXmlMirrorSnapshot {
        export_path,
        content_sha256,
        product_version: document.product.version,
        collection_track_count: collection.len(),
        playlists: selected,
        tracks,
        diagnostics,
        selection_paths: request.followed_paths.iter().cloned().collect(),
        include_future_child_playlists: request.include_future_child_playlists,
    })
}

fn latest_export(folder: &Path) -> Result<PathBuf, RekordboxXmlError> {
    let mut exports = fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_hidden = path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.starts_with('.'));
            let is_xml = path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("xml"));
            let metadata = entry.metadata().ok()?;
            (!is_hidden && is_xml && metadata.is_file())
                .then_some((metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH), path))
        })
        .collect::<Vec<_>>();
    exports.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    exports
        .into_iter()
        .next()
        .map(|(_, path)| path)
        .ok_or(RekordboxXmlError::NoXmlExports)
}

#[allow(clippy::too_many_arguments)]
fn collect_playlists(
    node: &XmlNode,
    parent_path: &str,
    followed_ancestor: bool,
    request: &RekordboxXmlSyncRequest,
    depth: usize,
    node_count: &mut usize,
    reference_count: &mut usize,
    duplicate_playlist_references: &mut usize,
    selected: &mut Vec<RekordboxXmlPlaylist>,
) -> Result<(), RekordboxXmlError> {
    if depth > MAXIMUM_DEPTH {
        return Err(RekordboxXmlError::PlaylistTreeTooDeep);
    }
    *node_count = node_count
        .checked_add(1)
        .ok_or(RekordboxXmlError::LimitExceeded)?;
    if *node_count > MAXIMUM_PLAYLIST_NODES {
        return Err(RekordboxXmlError::LimitExceeded);
    }
    let is_root_wrapper =
        parent_path.is_empty() && node.node_type == 0 && node.name.eq_ignore_ascii_case("ROOT");
    let path = if is_root_wrapper {
        String::new()
    } else if parent_path.is_empty() {
        node.name.clone()
    } else {
        format!("{parent_path}/{}", node.name)
    };
    if node.name.is_empty()
        || node.name.len() > MAXIMUM_NODE_NAME_BYTES
        || path.len() > MAXIMUM_PATH_BYTES
    {
        return Err(RekordboxXmlError::LimitExceeded);
    }
    let explicitly_followed = request.followed_paths.contains(&path);
    let folder_followed = followed_ancestor || (node.node_type == 0 && explicitly_followed);
    match node.node_type {
        0 => {
            if !node.tracks.is_empty() {
                return Err(RekordboxXmlError::InvalidPlaylistTree);
            }
            for child in &node.nodes {
                collect_playlists(
                    child,
                    &path,
                    folder_followed,
                    request,
                    depth + 1,
                    node_count,
                    reference_count,
                    duplicate_playlist_references,
                    selected,
                )?;
            }
        }
        1 => {
            if !node.nodes.is_empty() {
                return Err(RekordboxXmlError::InvalidPlaylistTree);
            }
            *reference_count = reference_count
                .checked_add(node.tracks.len())
                .ok_or(RekordboxXmlError::LimitExceeded)?;
            if *reference_count > MAXIMUM_TRACK_REFERENCES {
                return Err(RekordboxXmlError::LimitExceeded);
            }
            if node.entries.unwrap_or(0) != node.tracks.len() {
                return Err(RekordboxXmlError::PlaylistCountMismatch(path));
            }
            if explicitly_followed || folder_followed {
                let mut seen = BTreeSet::new();
                let track_ids = node
                    .tracks
                    .iter()
                    .map(|track| track.key.clone())
                    .filter(|track_id| seen.insert(track_id.clone()))
                    .collect::<Vec<_>>();
                *duplicate_playlist_references += node.tracks.len() - track_ids.len();
                selected.push(RekordboxXmlPlaylist {
                    path,
                    name: node.name.clone(),
                    track_ids,
                });
            }
        }
        value => return Err(RekordboxXmlError::UnsupportedNodeType(value)),
    }
    Ok(())
}

fn normalize_track(
    track: &XmlTrack,
    diagnostics: &mut RekordboxXmlDiagnostics,
) -> Result<RekordboxXmlTrack, RekordboxXmlError> {
    for (field, value) in [
        ("TrackID", Some(track.track_id.as_str())),
        ("Name", track.name.as_deref()),
        ("Artist", track.artist.as_deref()),
        ("AverageBpm", track.average_bpm.as_deref()),
        ("Tonality", track.tonality.as_deref()),
        ("Location", track.location.as_deref()),
        ("Colour", track.colour.as_deref()),
    ] {
        if value.is_some_and(|text| text.len() > MAXIMUM_TRACK_FIELD_BYTES) {
            return Err(RekordboxXmlError::TrackFieldTooLong {
                track_id: track.track_id.clone(),
                field,
            });
        }
    }
    let title = required_text(track.name.as_deref(), "Name", &track.track_id)?;
    let location = required_text(track.location.as_deref(), "Location", &track.track_id)?;
    if track.artist.as_deref().is_none_or(str::is_empty) {
        diagnostics.missing_artist += 1;
    }
    if track.average_bpm.as_deref().is_none_or(str::is_empty) {
        diagnostics.missing_bpm += 1;
    }
    if track.tonality.as_deref().is_none_or(str::is_empty) {
        diagnostics.missing_key += 1;
    }
    if track.total_time.unwrap_or(0) == 0 {
        diagnostics.missing_duration += 1;
    }
    if track.tempos.is_empty() {
        diagnostics.missing_beat_grid += 1;
    }
    if track.colour.as_deref().is_none_or(str::is_empty) {
        diagnostics.missing_colour += 1;
    }
    Ok(RekordboxXmlTrack {
        source_track_id: track.track_id.clone(),
        title,
        artist: nonempty(track.artist.clone()),
        average_bpm: nonempty(track.average_bpm.clone()),
        tonality: nonempty(track.tonality.clone()),
        total_time_seconds: track.total_time.filter(|value| *value > 0),
        location,
        colour: nonempty(track.colour.clone()),
        tempo_marker_count: track.tempos.len(),
        position_mark_count: track.position_marks.len(),
    })
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

fn required_text(
    value: Option<&str>,
    field: &'static str,
    track_id: &str,
) -> Result<String, RekordboxXmlError> {
    value
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| RekordboxXmlError::MissingTrackField {
            track_id: track_id.to_owned(),
            field,
        })
}

fn validate_document(document: &DjPlaylists) -> Result<(), RekordboxXmlError> {
    if document.version.len() > 32
        || document.product.name.len() > 128
        || document.product.version.len() > 128
    {
        return Err(RekordboxXmlError::LimitExceeded);
    }
    if document.version != "1.0.0" && document.version != "1,0,0" {
        return Err(RekordboxXmlError::UnsupportedVersion(
            document.version.clone(),
        ));
    }
    if !document.product.name.eq_ignore_ascii_case("rekordbox") {
        return Err(RekordboxXmlError::UnsupportedProduct(
            document.product.name.clone(),
        ));
    }
    if document.collection.tracks.is_empty()
        || document.collection.tracks.len() > MAXIMUM_COLLECTION_TRACKS
    {
        return Err(RekordboxXmlError::LimitExceeded);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum RekordboxXmlError {
    #[error("Rekordbox XML I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rekordbox XML parsing failed: {0}")]
    Parse(#[from] quick_xml::DeError),
    #[error("no XML exports were found in the configured folder")]
    NoXmlExports,
    #[error("the XML export has invalid size {0} bytes")]
    InvalidFileSize(u64),
    #[error("no playlists or folders are followed")]
    NoFollowedPlaylists,
    #[error("none of the followed playlist paths exist in the XML export")]
    NoMatchingPlaylists,
    #[error("unsupported Rekordbox XML version {0}")]
    UnsupportedVersion(String),
    #[error("unsupported XML product {0}")]
    UnsupportedProduct(String),
    #[error("the XML collection entry count does not match its tracks")]
    CollectionCountMismatch,
    #[error("playlist {0} entry count does not match its tracks")]
    PlaylistCountMismatch(String),
    #[error("playlist references collection track {0}, but it is missing")]
    MissingCollectionTrack(String),
    #[error("track {track_id} is missing required field {field}")]
    MissingTrackField {
        track_id: String,
        field: &'static str,
    },
    #[error("track {track_id} field {field} exceeds the safe length limit")]
    TrackFieldTooLong {
        track_id: String,
        field: &'static str,
    },
    #[error("playlist tree contains unsupported node type {0}")]
    UnsupportedNodeType(u8),
    #[error("playlist tree has an invalid folder or playlist structure")]
    InvalidPlaylistTree,
    #[error("playlist tree exceeds the maximum depth")]
    PlaylistTreeTooDeep,
    #[error("XML export exceeds a bounded parser limit")]
    LimitExceeded,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "DJ_PLAYLISTS")]
struct DjPlaylists {
    #[serde(rename = "@Version")]
    version: String,
    #[serde(rename = "PRODUCT")]
    product: XmlProduct,
    #[serde(rename = "COLLECTION")]
    collection: XmlCollection,
    #[serde(rename = "PLAYLISTS")]
    playlists: XmlPlaylists,
}

#[derive(Debug, Deserialize)]
struct XmlProduct {
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "@Version", default)]
    version: String,
}

#[derive(Debug, Deserialize)]
struct XmlCollection {
    #[serde(rename = "@Entries")]
    entries: usize,
    #[serde(rename = "TRACK", default)]
    tracks: Vec<XmlTrack>,
}

#[derive(Debug, Deserialize)]
struct XmlTrack {
    #[serde(rename = "@TrackID")]
    track_id: String,
    #[serde(rename = "@Name")]
    name: Option<String>,
    #[serde(rename = "@Artist")]
    artist: Option<String>,
    #[serde(rename = "@AverageBpm")]
    average_bpm: Option<String>,
    #[serde(rename = "@Tonality")]
    tonality: Option<String>,
    #[serde(rename = "@TotalTime")]
    total_time: Option<u64>,
    #[serde(rename = "@Location")]
    location: Option<String>,
    #[serde(rename = "@Colour")]
    colour: Option<String>,
    #[serde(rename = "TEMPO", default)]
    tempos: Vec<XmlTempo>,
    #[serde(rename = "POSITION_MARK", default)]
    position_marks: Vec<XmlPositionMark>,
}

#[derive(Debug, Deserialize)]
struct XmlTempo {}

#[derive(Debug, Deserialize)]
struct XmlPositionMark {}

#[derive(Debug, Deserialize)]
struct XmlPlaylists {
    #[serde(rename = "NODE", default)]
    nodes: Vec<XmlNode>,
}

#[derive(Debug, Deserialize)]
struct XmlNode {
    #[serde(rename = "@Type")]
    node_type: u8,
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "@Entries")]
    entries: Option<usize>,
    #[serde(rename = "NODE", default)]
    nodes: Vec<XmlNode>,
    #[serde(rename = "TRACK", default)]
    tracks: Vec<XmlTrackReference>,
}

#[derive(Debug, Deserialize)]
struct XmlTrackReference {
    #[serde(rename = "@Key")]
    key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_a_folder_and_deduplicates_shared_tracks() -> Result<(), Box<dyn std::error::Error>> {
        let folder = std::env::temp_dir().join(format!(
            "lumi-rekordbox-xml-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::create_dir_all(&folder)?;
        let path = folder.join("export.xml");
        fs::write(&path, FIXTURE)?;
        let request = RekordboxXmlSyncRequest::try_new(&folder, ["Sets".to_owned()], true)?;
        let snapshot = load_latest_mirror(&request)?;
        assert_eq!(snapshot.playlists().len(), 2);
        assert_eq!(snapshot.tracks().len(), 2);
        assert_eq!(snapshot.diagnostics().missing_waveform, 2);
        assert_eq!(snapshot.diagnostics().missing_phrases, 2);
        assert_eq!(snapshot.content_sha256().len(), 64);
        fs::remove_dir_all(folder)?;
        Ok(())
    }

    #[test]
    fn configured_real_export_can_be_smoke_tested_without_logging_library_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let Ok(folder) = std::env::var("LUMI_REKORDBOX_XML_TEST_FOLDER") else {
            return Ok(());
        };
        let mut paths = std::env::var("LUMI_REKORDBOX_XML_TEST_PATHS")?
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        paths.sort();
        let request = RekordboxXmlSyncRequest::try_new(folder, paths.clone(), true)?;
        let snapshot = load_latest_mirror(&request)?;

        assert!(!snapshot.playlists().is_empty());
        assert!(!snapshot.tracks().is_empty());
        assert!(snapshot.collection_track_count() >= snapshot.tracks().len());
        assert_eq!(snapshot.content_sha256().len(), 64);
        assert_eq!(snapshot.selection_paths(), paths);
        Ok(())
    }

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="7.2.14"/>
  <COLLECTION Entries="2">
    <TRACK TrackID="1" Name="One" Artist="Artist" AverageBpm="128.00" Tonality="Am" TotalTime="120" Location="file://localhost/one.wav"><TEMPO/></TRACK>
    <TRACK TrackID="2" Name="Two" AverageBpm="130.00" Tonality="C" TotalTime="180" Location="file://localhost/two.wav"><TEMPO/><POSITION_MARK/></TRACK>
  </COLLECTION>
  <PLAYLISTS><NODE Type="0" Name="ROOT"><NODE Type="0" Name="Sets"><NODE Type="1" Name="A" Entries="2"><TRACK Key="1"/><TRACK Key="2"/></NODE><NODE Type="1" Name="B" Entries="1"><TRACK Key="2"/></NODE></NODE></NODE></PLAYLISTS>
</DJ_PLAYLISTS>"#;
}
