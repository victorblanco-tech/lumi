use std::error::Error;

use lumi_domain::{MusicalKey, TrackId};

use crate::{
    AutoloopCatalog, BeatGrid, ImportedLibraryBaseline, LibrarySourceId, LumiPhraseTimeline,
    PhraseRoleCatalog, PhraseRoleUsage, PlaylistId, RawPhraseObservation, SourceMirrorDiff,
    SourceMirrorSnapshot, SourceMirrorSummary, SourcePlaylistId, SourceRevision, SourceTrackId,
    TimelineRevision, TimelineRevisionOrigin, TrackColor, WaveformPoint,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportResult {
    pub inserted: u32,
    pub updated: u32,
    pub unchanged: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibrarySourceSummary {
    id: LibrarySourceId,
    kind: String,
    display_name: String,
    revision: SourceRevision,
}

impl LibrarySourceSummary {
    #[must_use]
    pub const fn new(
        id: LibrarySourceId,
        kind: String,
        display_name: String,
        revision: SourceRevision,
    ) -> Self {
        Self {
            id,
            kind,
            display_name,
            revision,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &LibrarySourceId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn revision(&self) -> &SourceRevision {
        &self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackPageRequest {
    offset: u32,
    limit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryTrackQuery {
    search: String,
    playlist_id: Option<PlaylistId>,
    page: TrackPageRequest,
}

impl LibraryTrackQuery {
    pub fn try_new(
        search: impl Into<String>,
        playlist_id: Option<PlaylistId>,
        page: TrackPageRequest,
    ) -> Result<Self, TrackPageRequestError> {
        let search = search.into();
        if search.len() > 200 {
            return Err(TrackPageRequestError::SearchTooLong);
        }
        Ok(Self {
            search: search.trim().to_owned(),
            playlist_id,
            page,
        })
    }

    #[must_use]
    pub fn search(&self) -> &str {
        &self.search
    }

    #[must_use]
    pub const fn playlist_id(&self) -> Option<PlaylistId> {
        self.playlist_id
    }

    #[must_use]
    pub const fn page(&self) -> TrackPageRequest {
        self.page
    }
}

impl TrackPageRequest {
    pub fn try_new(offset: u32, limit: u16) -> Result<Self, TrackPageRequestError> {
        if limit == 0 || limit > 200 {
            return Err(TrackPageRequestError::InvalidLimit(limit));
        }
        Ok(Self { offset, limit })
    }

    #[must_use]
    pub const fn offset(self) -> u32 {
        self.offset
    }

    #[must_use]
    pub const fn limit(self) -> u16 {
        self.limit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackPageRequestError {
    InvalidLimit(u16),
    SearchTooLong,
}

impl std::fmt::Display for TrackPageRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLimit(value) => write!(formatter, "page limit {value} is outside 1..=200"),
            Self::SearchTooLong => formatter.write_str("library search exceeds 200 bytes"),
        }
    }
}

impl Error for TrackPageRequestError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackSummary {
    id: TrackId,
    source_track_id: SourceTrackId,
    title: String,
    artist: String,
    bpm_milli: u32,
    musical_key: MusicalKey,
    duration_millis: u64,
    color: Option<TrackColor>,
    source_revision: SourceRevision,
    timeline_revision: Option<TimelineRevision>,
}

impl TrackSummary {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: TrackId,
        source_track_id: SourceTrackId,
        title: String,
        artist: String,
        bpm_milli: u32,
        musical_key: MusicalKey,
        duration_millis: u64,
        color: Option<TrackColor>,
        source_revision: SourceRevision,
        timeline_revision: Option<TimelineRevision>,
    ) -> Self {
        Self {
            id,
            source_track_id,
            title,
            artist,
            bpm_milli,
            musical_key,
            duration_millis,
            color,
            source_revision,
            timeline_revision,
        }
    }

    #[must_use]
    pub const fn id(&self) -> TrackId {
        self.id
    }
    #[must_use]
    pub const fn source_track_id(&self) -> &SourceTrackId {
        &self.source_track_id
    }
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    #[must_use]
    pub fn artist(&self) -> &str {
        &self.artist
    }
    #[must_use]
    pub const fn bpm_milli(&self) -> u32 {
        self.bpm_milli
    }
    #[must_use]
    pub const fn musical_key(&self) -> MusicalKey {
        self.musical_key
    }
    #[must_use]
    pub const fn duration_millis(&self) -> u64 {
        self.duration_millis
    }
    #[must_use]
    pub const fn color(&self) -> Option<TrackColor> {
        self.color
    }
    #[must_use]
    pub const fn source_revision(&self) -> &SourceRevision {
        &self.source_revision
    }
    #[must_use]
    pub const fn timeline_revision(&self) -> Option<TimelineRevision> {
        self.timeline_revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackPage {
    total: u64,
    offset: u32,
    tracks: Vec<TrackSummary>,
}

impl TrackPage {
    #[must_use]
    pub const fn new(total: u64, offset: u32, tracks: Vec<TrackSummary>) -> Self {
        Self {
            total,
            offset,
            tracks,
        }
    }
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }
    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.offset
    }
    #[must_use]
    pub fn tracks(&self) -> &[TrackSummary] {
        &self.tracks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaylistSummary {
    id: PlaylistId,
    source_playlist_id: SourcePlaylistId,
    name: String,
    track_count: u64,
}

impl PlaylistSummary {
    #[must_use]
    pub const fn new(
        id: PlaylistId,
        source_playlist_id: SourcePlaylistId,
        name: String,
        track_count: u64,
    ) -> Self {
        Self {
            id,
            source_playlist_id,
            name,
            track_count,
        }
    }

    #[must_use]
    pub const fn id(&self) -> PlaylistId {
        self.id
    }

    #[must_use]
    pub const fn source_playlist_id(&self) -> &SourcePlaylistId {
        &self.source_playlist_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn track_count(&self) -> u64 {
        self.track_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaylistPage {
    total: u64,
    offset: u32,
    playlists: Vec<PlaylistSummary>,
}

impl PlaylistPage {
    #[must_use]
    pub const fn new(total: u64, offset: u32, playlists: Vec<PlaylistSummary>) -> Self {
        Self {
            total,
            offset,
            playlists,
        }
    }

    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }

    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    #[must_use]
    pub fn playlists(&self) -> &[PlaylistSummary] {
        &self.playlists
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineRevisionSummary {
    revision: TimelineRevision,
    baseline_revision: SourceRevision,
    total_beats: u32,
    origin: TimelineRevisionOrigin,
    reason: crate::TimelineRevisionReason,
    parent_revision: Option<TimelineRevision>,
    restored_from: Option<TimelineRevision>,
    phrase_count: u32,
}

impl TimelineRevisionSummary {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        revision: TimelineRevision,
        baseline_revision: SourceRevision,
        total_beats: u32,
        origin: TimelineRevisionOrigin,
        reason: crate::TimelineRevisionReason,
        parent_revision: Option<TimelineRevision>,
        restored_from: Option<TimelineRevision>,
        phrase_count: u32,
    ) -> Self {
        Self {
            revision,
            baseline_revision,
            total_beats,
            origin,
            reason,
            parent_revision,
            restored_from,
            phrase_count,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> TimelineRevision {
        self.revision
    }

    #[must_use]
    pub const fn baseline_revision(&self) -> &SourceRevision {
        &self.baseline_revision
    }

    #[must_use]
    pub const fn total_beats(&self) -> u32 {
        self.total_beats
    }

    #[must_use]
    pub const fn origin(&self) -> TimelineRevisionOrigin {
        self.origin
    }

    #[must_use]
    pub const fn reason(&self) -> crate::TimelineRevisionReason {
        self.reason
    }

    #[must_use]
    pub const fn parent_revision(&self) -> Option<TimelineRevision> {
        self.parent_revision
    }

    #[must_use]
    pub const fn restored_from(&self) -> Option<TimelineRevision> {
        self.restored_from
    }

    #[must_use]
    pub const fn phrase_count(&self) -> u32 {
        self.phrase_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineRevisionPage {
    total: u64,
    offset: u32,
    revisions: Vec<TimelineRevisionSummary>,
}

impl TimelineRevisionPage {
    #[must_use]
    pub const fn new(total: u64, offset: u32, revisions: Vec<TimelineRevisionSummary>) -> Self {
        Self {
            total,
            offset,
            revisions,
        }
    }

    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }

    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    #[must_use]
    pub fn revisions(&self) -> &[TimelineRevisionSummary] {
        &self.revisions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredTrack {
    summary: TrackSummary,
    audio_uri: String,
    beat_grid: BeatGrid,
    waveform: Vec<WaveformPoint>,
    raw_phrases: Vec<RawPhraseObservation>,
    hot_cues: Vec<crate::HotCue>,
}

impl StoredTrack {
    #[must_use]
    pub const fn new(
        summary: TrackSummary,
        audio_uri: String,
        beat_grid: BeatGrid,
        waveform: Vec<WaveformPoint>,
        raw_phrases: Vec<RawPhraseObservation>,
    ) -> Self {
        Self {
            summary,
            audio_uri,
            beat_grid,
            waveform,
            raw_phrases,
            hot_cues: Vec::new(),
        }
    }
    #[must_use]
    pub fn with_hot_cues(mut self, hot_cues: Vec<crate::HotCue>) -> Self {
        self.hot_cues = hot_cues;
        self
    }
    #[must_use]
    pub const fn summary(&self) -> &TrackSummary {
        &self.summary
    }
    #[must_use]
    pub fn audio_uri(&self) -> &str {
        &self.audio_uri
    }
    #[must_use]
    pub const fn beat_grid(&self) -> &BeatGrid {
        &self.beat_grid
    }
    #[must_use]
    pub fn waveform(&self) -> &[WaveformPoint] {
        &self.waveform
    }
    #[must_use]
    pub fn raw_phrases(&self) -> &[RawPhraseObservation] {
        &self.raw_phrases
    }
    #[must_use]
    pub fn hot_cues(&self) -> &[crate::HotCue] {
        &self.hot_cues
    }
}

pub trait LibraryRepository {
    type Error: Error + Send + Sync + 'static;

    fn schema_version(&self) -> Result<u32, Self::Error>;
    fn import_baseline(
        &mut self,
        baseline: &ImportedLibraryBaseline,
    ) -> Result<ImportResult, Self::Error>;
    fn preview_source_mirror(
        &self,
        snapshot: &SourceMirrorSnapshot,
    ) -> Result<SourceMirrorDiff, Self::Error>;
    fn apply_source_mirror(
        &mut self,
        snapshot: &SourceMirrorSnapshot,
    ) -> Result<SourceMirrorDiff, Self::Error>;
    fn source_mirror_summary(
        &self,
        id: &LibrarySourceId,
    ) -> Result<Option<SourceMirrorSummary>, Self::Error>;
    fn library_source(
        &self,
        id: &LibrarySourceId,
    ) -> Result<Option<LibrarySourceSummary>, Self::Error>;
    fn complete_source_refresh(
        &mut self,
        baseline: &ImportedLibraryBaseline,
    ) -> Result<(), Self::Error>;
    fn restore_source_checkpoint(
        &mut self,
        baseline: &ImportedLibraryBaseline,
    ) -> Result<(), Self::Error>;
    fn reconcile_track(
        &mut self,
        baseline: &ImportedLibraryBaseline,
        incoming: &crate::ImportedTrackAnalysis,
        timeline: &LumiPhraseTimeline,
        expected_head: TimelineRevision,
    ) -> Result<(), Self::Error>;
    fn refresh_track_without_timeline(
        &mut self,
        baseline: &ImportedLibraryBaseline,
        incoming: &crate::ImportedTrackAnalysis,
        expected_analysis_revision: &crate::SourceRevision,
    ) -> Result<(), Self::Error>;
    fn page_tracks(&self, request: TrackPageRequest) -> Result<TrackPage, Self::Error>;
    fn query_tracks(&self, query: &LibraryTrackQuery) -> Result<TrackPage, Self::Error>;
    fn page_playlists(&self, request: TrackPageRequest) -> Result<PlaylistPage, Self::Error>;
    fn page_playlist_tracks(
        &self,
        playlist_id: PlaylistId,
        request: TrackPageRequest,
    ) -> Result<TrackPage, Self::Error>;
    fn track(&self, id: TrackId) -> Result<Option<StoredTrack>, Self::Error>;
    fn phrase_role_catalog(&self) -> Result<PhraseRoleCatalog, Self::Error>;
    fn initialize_phrase_role_catalog(
        &mut self,
        catalog: &PhraseRoleCatalog,
    ) -> Result<(), Self::Error>;
    fn replace_phrase_role_catalog(
        &mut self,
        catalog: &PhraseRoleCatalog,
        expected_revision: u64,
    ) -> Result<(), Self::Error>;
    fn phrase_role_usages(&self) -> Result<Vec<PhraseRoleUsage>, Self::Error>;
    fn autoloop_catalog(&self) -> Result<AutoloopCatalog, Self::Error>;
    fn initialize_autoloop_catalog(&mut self, catalog: &AutoloopCatalog)
    -> Result<(), Self::Error>;
    fn replace_autoloop_catalog(
        &mut self,
        catalog: &AutoloopCatalog,
        expected_revision: u64,
    ) -> Result<(), Self::Error>;
    fn append_timeline_revision(
        &mut self,
        timeline: &LumiPhraseTimeline,
        expected_head: Option<TimelineRevision>,
    ) -> Result<(), Self::Error>;
    fn timeline_head(&self, track_id: TrackId) -> Result<Option<LumiPhraseTimeline>, Self::Error>;
    fn timeline_revision(
        &self,
        track_id: TrackId,
        revision: TimelineRevision,
    ) -> Result<Option<LumiPhraseTimeline>, Self::Error>;
    fn timeline_revisions(
        &self,
        track_id: TrackId,
        request: TrackPageRequest,
    ) -> Result<TimelineRevisionPage, Self::Error>;
}
