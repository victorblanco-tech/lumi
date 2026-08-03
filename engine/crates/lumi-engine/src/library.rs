use lumi_domain::{KeyMode, PitchClass, TrackId};
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
    editor_track_id: Option<TrackId>,
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
            editor_track_id: None,
        })
    }

    pub fn query(&mut self, search: String, playlist_id: Option<u64>, offset: u32, limit: u16) {
        self.search = search;
        self.playlist_id = playlist_id.map(PlaylistId::new);
        self.offset = offset;
        self.limit = limit;
    }

    pub fn open_editor(&mut self, track_id: u64) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        if self.repository.track(track_id)?.is_none() {
            return Err(LibraryWorkerError::UnknownTrack(track_id.value()));
        }
        self.editor_track_id = Some(track_id);
        Ok(())
    }

    pub const fn close_editor(&mut self) {
        self.editor_track_id = None;
    }

    pub fn snapshot_json(&self) -> Result<Value, LibraryWorkerError> {
        let request = TrackPageRequest::try_new(self.offset, self.limit)?;
        let query = LibraryTrackQuery::try_new(self.search.clone(), self.playlist_id, request)?;
        let page = self.repository.query_tracks(&query)?;
        let collection_total = self
            .repository
            .query_tracks(&LibraryTrackQuery::try_new(
                "",
                None,
                TrackPageRequest::try_new(0, 1)?,
            )?)?
            .total();
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
            "collectionTotal": collection_total,
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
            "editor": self.editor_json()?,
        }))
    }

    fn editor_json(&self) -> Result<Value, LibraryWorkerError> {
        let Some(track_id) = self.editor_track_id else {
            return Ok(Value::Null);
        };
        let track = self
            .repository
            .track(track_id)?
            .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
        Ok(json!({
            "track": track_json(track.summary()),
            "audioUri": track.audio_uri(),
            "beatGrid": {
                "beatsPerBar": track.beat_grid().beats_per_bar(),
                "markers": track.beat_grid().markers().iter().map(|marker| json!({
                    "beatIndex": marker.beat_index(),
                    "timeMillis": marker.time_millis(),
                    "barIndex": marker.bar_index(),
                    "beatInBar": marker.beat_in_bar(),
                })).collect::<Vec<_>>(),
            },
            "waveform": track.waveform().iter().map(|point| json!({
                "low": point.low(),
                "mid": point.mid(),
                "high": point.high(),
            })).collect::<Vec<_>>(),
            "phrases": track.raw_phrases().iter().enumerate().map(|(index, phrase)| json!({
                "id": index,
                "startBeat": phrase.start_beat(),
                "endBeat": phrase.end_beat(),
                "role": phrase.source_label(),
                "origin": "source",
            })).collect::<Vec<_>>(),
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
    #[error("library track {0} does not exist")]
    UnknownTrack(u64),
}

#[cfg(test)]
mod tests {
    use super::LibraryWorker;

    #[test]
    fn collection_total_is_independent_from_the_active_playlist()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.query(String::new(), Some(2), 0, 50);

        let snapshot = worker.snapshot_json()?;

        assert_eq!(snapshot["collectionTotal"], 3);
        assert_eq!(snapshot["page"]["total"], 2);
        Ok(())
    }

    #[test]
    fn editor_snapshot_exposes_read_only_analysis_and_closes_cleanly()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let collection = worker.snapshot_json()?;
        let track_id = collection["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("demo track ID is missing")?;

        worker.open_editor(track_id)?;
        let opened = worker.snapshot_json()?;
        assert_eq!(opened["editor"]["track"]["id"], track_id);
        assert_eq!(opened["editor"]["beatGrid"]["beatsPerBar"], 4);
        assert!(
            opened["editor"]["beatGrid"]["markers"]
                .as_array()
                .is_some_and(|markers| !markers.is_empty())
        );
        assert!(
            opened["editor"]["waveform"]
                .as_array()
                .is_some_and(|points| !points.is_empty())
        );
        assert!(
            opened["editor"]["phrases"]
                .as_array()
                .is_some_and(|phrases| !phrases.is_empty())
        );

        worker.close_editor();
        assert!(worker.snapshot_json()?["editor"].is_null());
        Ok(())
    }

    #[test]
    fn unknown_editor_track_is_rejected_without_changing_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        assert!(worker.open_editor(u64::MAX).is_err());
        assert!(worker.snapshot_json()?["editor"].is_null());
        Ok(())
    }
}
