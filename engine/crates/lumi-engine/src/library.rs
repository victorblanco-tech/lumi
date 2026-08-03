use lumi_domain::{KeyMode, PitchClass};
use lumi_library::{
    LibraryRepository, LibraryTrackQuery, PlaylistId, TrackPageRequest, TrackSummary,
};
use lumi_library_demo::{DemoLibraryError, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider as _;
use lumi_library_sqlite::{SqliteLibraryError, SqliteLibraryRepository};
use serde_json::{Value, json};
use thiserror::Error;

const DEFAULT_PAGE_LIMIT: u16 = 50;

pub struct LibraryWorker {
    repository: SqliteLibraryRepository,
    source_id: String,
    source_kind: String,
    source_name: String,
    source_revision: String,
    search: String,
    playlist_id: Option<PlaylistId>,
    offset: u32,
    limit: u16,
}

impl LibraryWorker {
    pub fn demo() -> Result<Self, LibraryWorkerError> {
        let provider = DemoLibrarySourceProvider::curated();
        let baseline = provider.load_baseline()?;
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(&baseline)?;
        Ok(Self {
            repository,
            source_id: baseline.source_id().as_str().to_owned(),
            source_kind: baseline.source_kind().to_owned(),
            source_name: baseline.display_name().to_owned(),
            source_revision: baseline.source_revision().as_str().to_owned(),
            search: String::new(),
            playlist_id: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        })
    }

    pub fn query(&mut self, search: String, playlist_id: Option<u64>, offset: u32, limit: u16) {
        self.search = search;
        self.playlist_id = playlist_id.map(PlaylistId::new);
        self.offset = offset;
        self.limit = limit;
    }

    pub fn snapshot_json(&self) -> Result<Value, LibraryWorkerError> {
        let request = TrackPageRequest::try_new(self.offset, self.limit)?;
        let query = LibraryTrackQuery::try_new(self.search.clone(), self.playlist_id, request)?;
        let page = self.repository.query_tracks(&query)?;
        let playlist_page = self
            .repository
            .page_playlists(TrackPageRequest::try_new(0, 200)?)?;
        Ok(json!({
            "condition": if page.total() == 0 && self.search.is_empty() && self.playlist_id.is_none() {
                "empty"
            } else {
                "ready"
            },
            "providerKind": self.source_kind,
            "source": {
                "id": self.source_id,
                "name": self.source_name,
                "revision": self.source_revision,
                "status": "current",
            },
            "capabilities": {
                "playlists": true,
                "color": true,
                "beatGrid": true,
                "waveform": true,
                "rawPhrases": true,
                "localAudio": true,
            },
            "query": {
                "search": self.search,
                "playlistId": self.playlist_id.map(|id| id.value()),
                "offset": page.offset(),
                "limit": self.limit,
            },
            "playlists": playlist_page.playlists().iter().map(|playlist| json!({
                "id": playlist.id().value(),
                "sourcePlaylistId": playlist.source_playlist_id().as_str(),
                "name": playlist.name(),
                "trackCount": playlist.track_count(),
            })).collect::<Vec<_>>(),
            "page": {
                "total": page.total(),
                "offset": page.offset(),
                "tracks": page.tracks().iter().map(track_json).collect::<Vec<_>>(),
            },
        }))
    }
}

fn track_json(track: &TrackSummary) -> Value {
    json!({
        "id": track.id().value(),
        "sourceTrackId": track.source_track_id().as_str(),
        "title": track.title(),
        "artist": track.artist(),
        "bpmMilli": track.bpm_milli(),
        "key": {
            "pitchClass": pitch_class_name(track.musical_key().pitch_class()),
            "mode": key_mode_name(track.musical_key().mode()),
        },
        "durationMillis": track.duration_millis(),
        "colorRgb": track.color().map(|color| color.rgb_u32()),
        "analysisRevision": track.source_revision().as_str(),
        "timelineRevision": track.timeline_revision().map(|revision| revision.value()),
        "readiness": {
            "status": "ready",
            "missingCapabilities": [],
            "warnings": [],
        },
    })
}

fn pitch_class_name(value: PitchClass) -> &'static str {
    match value {
        PitchClass::C => "c",
        PitchClass::CSharp => "cSharp",
        PitchClass::D => "d",
        PitchClass::DSharp => "dSharp",
        PitchClass::E => "e",
        PitchClass::F => "f",
        PitchClass::FSharp => "fSharp",
        PitchClass::G => "g",
        PitchClass::GSharp => "gSharp",
        PitchClass::A => "a",
        PitchClass::ASharp => "aSharp",
        PitchClass::B => "b",
    }
}

fn key_mode_name(value: KeyMode) -> &'static str {
    match value {
        KeyMode::Major => "major",
        KeyMode::Minor => "minor",
    }
}

#[derive(Debug, Error)]
pub enum LibraryWorkerError {
    #[error("demo library failed: {0}")]
    Demo(#[from] DemoLibraryError),
    #[error("library persistence failed: {0}")]
    Persistence(#[from] SqliteLibraryError),
    #[error("library query is invalid: {0}")]
    Query(#[from] lumi_library::TrackPageRequestError),
}
