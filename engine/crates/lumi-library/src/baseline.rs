use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use lumi_domain::MusicalKey;

use crate::{LibrarySourceId, SourcePlaylistId, SourceRevision, SourceTrackId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackColor {
    red: u8,
    green: u8,
    blue: u8,
}

impl TrackColor {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }

    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }

    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }

    #[must_use]
    pub const fn rgb_u32(self) -> u32 {
        (self.red as u32) << 16 | (self.green as u32) << 8 | self.blue as u32
    }

    #[must_use]
    pub const fn from_rgb_u32(value: u32) -> Self {
        Self {
            red: ((value >> 16) & 0xff) as u8,
            green: ((value >> 8) & 0xff) as u8,
            blue: (value & 0xff) as u8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeatMarker {
    beat_index: u32,
    time_millis: u64,
    bar_index: u32,
    beat_in_bar: u8,
}

impl BeatMarker {
    #[must_use]
    pub const fn new(beat_index: u32, time_millis: u64, bar_index: u32, beat_in_bar: u8) -> Self {
        Self {
            beat_index,
            time_millis,
            bar_index,
            beat_in_bar,
        }
    }

    #[must_use]
    pub const fn beat_index(self) -> u32 {
        self.beat_index
    }

    #[must_use]
    pub const fn time_millis(self) -> u64 {
        self.time_millis
    }

    #[must_use]
    pub const fn bar_index(self) -> u32 {
        self.bar_index
    }

    #[must_use]
    pub const fn beat_in_bar(self) -> u8 {
        self.beat_in_bar
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeatGrid {
    beats_per_bar: u8,
    markers: Vec<BeatMarker>,
}

impl BeatGrid {
    pub fn try_new(
        beats_per_bar: u8,
        markers: Vec<BeatMarker>,
    ) -> Result<Self, BeatGridValidationError> {
        if !(1..=16).contains(&beats_per_bar) {
            return Err(BeatGridValidationError::InvalidBeatsPerBar(beats_per_bar));
        }
        if markers.is_empty() {
            return Err(BeatGridValidationError::Empty);
        }
        if !markers.len().is_multiple_of(usize::from(beats_per_bar)) {
            return Err(BeatGridValidationError::IncompleteBar);
        }
        let mut previous_time = None;
        for (index, marker) in markers.iter().copied().enumerate() {
            let expected_index =
                u32::try_from(index).map_err(|_| BeatGridValidationError::TooManyMarkers)?;
            let expected_bar = expected_index / u32::from(beats_per_bar) + 1;
            let expected_beat = u8::try_from(expected_index % u32::from(beats_per_bar) + 1)
                .map_err(|_| BeatGridValidationError::TooManyMarkers)?;
            if marker.beat_index() != expected_index
                || marker.bar_index() != expected_bar
                || marker.beat_in_bar() != expected_beat
            {
                return Err(BeatGridValidationError::InvalidMarkerOrder);
            }
            if previous_time.is_some_and(|time| marker.time_millis() <= time) {
                return Err(BeatGridValidationError::NonIncreasingTime);
            }
            previous_time = Some(marker.time_millis());
        }
        Ok(Self {
            beats_per_bar,
            markers,
        })
    }

    #[must_use]
    pub const fn beats_per_bar(&self) -> u8 {
        self.beats_per_bar
    }

    #[must_use]
    pub fn markers(&self) -> &[BeatMarker] {
        &self.markers
    }

    #[must_use]
    pub fn total_bars(&self) -> u32 {
        u32::try_from(self.markers.len() / usize::from(self.beats_per_bar)).unwrap_or(u32::MAX)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeatGridValidationError {
    InvalidBeatsPerBar(u8),
    Empty,
    IncompleteBar,
    TooManyMarkers,
    InvalidMarkerOrder,
    NonIncreasingTime,
}

impl fmt::Display for BeatGridValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBeatsPerBar(value) => write!(formatter, "invalid beats per bar {value}"),
            Self::Empty => formatter.write_str("beat grid may not be empty"),
            Self::IncompleteBar => formatter.write_str("beat grid must end on a complete bar"),
            Self::TooManyMarkers => formatter.write_str("beat grid contains too many markers"),
            Self::InvalidMarkerOrder => {
                formatter.write_str("beat markers must be contiguous and bar-aligned")
            }
            Self::NonIncreasingTime => {
                formatter.write_str("beat marker times must increase strictly")
            }
        }
    }
}

impl Error for BeatGridValidationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaveformPoint {
    low: u8,
    mid: u8,
    high: u8,
}

impl WaveformPoint {
    #[must_use]
    pub const fn new(low: u8, mid: u8, high: u8) -> Self {
        Self { low, mid, high }
    }

    #[must_use]
    pub const fn low(self) -> u8 {
        self.low
    }

    #[must_use]
    pub const fn mid(self) -> u8 {
        self.mid
    }

    #[must_use]
    pub const fn high(self) -> u8 {
        self.high
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawPhraseObservation {
    start_beat: u32,
    end_beat: u32,
    source_label: String,
}

impl RawPhraseObservation {
    pub fn try_new(
        start_beat: u32,
        end_beat: u32,
        source_label: impl Into<String>,
    ) -> Result<Self, TrackValidationError> {
        let source_label = source_label.into();
        if source_label.trim().is_empty() {
            return Err(TrackValidationError::EmptyRawPhraseLabel);
        }
        if end_beat <= start_beat {
            return Err(TrackValidationError::InvalidRawPhraseRange);
        }
        Ok(Self {
            start_beat,
            end_beat,
            source_label,
        })
    }

    #[must_use]
    pub const fn start_beat(&self) -> u32 {
        self.start_beat
    }

    #[must_use]
    pub const fn end_beat(&self) -> u32 {
        self.end_beat
    }

    #[must_use]
    pub fn source_label(&self) -> &str {
        &self.source_label
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedTrackAnalysis {
    source_track_id: SourceTrackId,
    analysis_revision: SourceRevision,
    title: String,
    artist: String,
    bpm_milli: u32,
    musical_key: MusicalKey,
    duration_millis: u64,
    color: Option<TrackColor>,
    audio_uri: String,
    beat_grid: BeatGrid,
    waveform: Vec<WaveformPoint>,
    raw_phrases: Vec<RawPhraseObservation>,
}

impl ImportedTrackAnalysis {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        source_track_id: SourceTrackId,
        analysis_revision: SourceRevision,
        title: impl Into<String>,
        artist: impl Into<String>,
        bpm_milli: u32,
        musical_key: MusicalKey,
        duration_millis: u64,
        color: Option<TrackColor>,
        audio_uri: impl Into<String>,
        beat_grid: BeatGrid,
        waveform: Vec<WaveformPoint>,
        raw_phrases: Vec<RawPhraseObservation>,
    ) -> Result<Self, TrackValidationError> {
        let title = title.into();
        let artist = artist.into();
        let audio_uri = audio_uri.into();
        if title.trim().is_empty() {
            return Err(TrackValidationError::EmptyTitle);
        }
        if artist.trim().is_empty() {
            return Err(TrackValidationError::EmptyArtist);
        }
        if !(20_000..=300_000).contains(&bpm_milli) {
            return Err(TrackValidationError::BpmOutOfRange(bpm_milli));
        }
        if duration_millis == 0 {
            return Err(TrackValidationError::EmptyDuration);
        }
        if audio_uri.trim().is_empty() {
            return Err(TrackValidationError::EmptyAudioUri);
        }
        if waveform.is_empty() {
            return Err(TrackValidationError::EmptyWaveform);
        }
        let total_beats = u32::try_from(beat_grid.markers().len())
            .map_err(|_| TrackValidationError::TooManyBeats)?;
        if raw_phrases
            .iter()
            .any(|phrase| phrase.start_beat() >= total_beats || phrase.end_beat() > total_beats)
        {
            return Err(TrackValidationError::InvalidRawPhraseRange);
        }
        Ok(Self {
            source_track_id,
            analysis_revision,
            title,
            artist,
            bpm_milli,
            musical_key,
            duration_millis,
            color,
            audio_uri,
            beat_grid,
            waveform,
            raw_phrases,
        })
    }

    #[must_use]
    pub const fn source_track_id(&self) -> &SourceTrackId {
        &self.source_track_id
    }
    #[must_use]
    pub const fn analysis_revision(&self) -> &SourceRevision {
        &self.analysis_revision
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackValidationError {
    EmptyTitle,
    EmptyArtist,
    BpmOutOfRange(u32),
    EmptyDuration,
    EmptyAudioUri,
    EmptyWaveform,
    TooManyBeats,
    EmptyRawPhraseLabel,
    InvalidRawPhraseRange,
}

impl fmt::Display for TrackValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => formatter.write_str("track title may not be empty"),
            Self::EmptyArtist => formatter.write_str("track artist may not be empty"),
            Self::BpmOutOfRange(value) => write!(formatter, "track BPM {value} is outside range"),
            Self::EmptyDuration => formatter.write_str("track duration must be positive"),
            Self::EmptyAudioUri => formatter.write_str("audio URI may not be empty"),
            Self::EmptyWaveform => formatter.write_str("waveform may not be empty"),
            Self::TooManyBeats => formatter.write_str("track contains too many beats"),
            Self::EmptyRawPhraseLabel => formatter.write_str("raw phrase label may not be empty"),
            Self::InvalidRawPhraseRange => formatter.write_str("raw phrase range is invalid"),
        }
    }
}

impl Error for TrackValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedLibraryBaseline {
    source_id: LibrarySourceId,
    source_kind: String,
    display_name: String,
    source_revision: SourceRevision,
    tracks: Vec<ImportedTrackAnalysis>,
    playlists: Vec<ImportedPlaylist>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedPlaylist {
    source_playlist_id: SourcePlaylistId,
    name: String,
    track_ids: Vec<SourceTrackId>,
}

impl ImportedPlaylist {
    pub fn try_new(
        source_playlist_id: SourcePlaylistId,
        name: impl Into<String>,
        track_ids: Vec<SourceTrackId>,
    ) -> Result<Self, LibraryBaselineValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(LibraryBaselineValidationError::EmptyPlaylistName);
        }
        let unique_track_ids = track_ids.iter().collect::<BTreeSet<_>>();
        if unique_track_ids.len() != track_ids.len() {
            return Err(LibraryBaselineValidationError::DuplicatePlaylistTrack);
        }
        Ok(Self {
            source_playlist_id,
            name,
            track_ids,
        })
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
    pub fn track_ids(&self) -> &[SourceTrackId] {
        &self.track_ids
    }
}

impl ImportedLibraryBaseline {
    pub fn try_new(
        source_id: LibrarySourceId,
        source_kind: impl Into<String>,
        display_name: impl Into<String>,
        source_revision: SourceRevision,
        tracks: Vec<ImportedTrackAnalysis>,
        playlists: Vec<ImportedPlaylist>,
    ) -> Result<Self, LibraryBaselineValidationError> {
        let source_kind = source_kind.into();
        let display_name = display_name.into();
        if source_kind.trim().is_empty() {
            return Err(LibraryBaselineValidationError::EmptySourceKind);
        }
        if display_name.trim().is_empty() {
            return Err(LibraryBaselineValidationError::EmptyDisplayName);
        }
        if tracks.is_empty() {
            return Err(LibraryBaselineValidationError::EmptyTracks);
        }
        let mut ids = BTreeSet::new();
        for track in &tracks {
            if !ids.insert(track.source_track_id().clone()) {
                return Err(LibraryBaselineValidationError::DuplicateTrackIdentity);
            }
        }
        let mut playlist_ids = BTreeSet::new();
        for playlist in &playlists {
            if !playlist_ids.insert(playlist.source_playlist_id().clone()) {
                return Err(LibraryBaselineValidationError::DuplicatePlaylistIdentity);
            }
            if playlist
                .track_ids()
                .iter()
                .any(|track_id| !ids.contains(track_id))
            {
                return Err(LibraryBaselineValidationError::UnknownPlaylistTrack);
            }
        }
        Ok(Self {
            source_id,
            source_kind,
            display_name,
            source_revision,
            tracks,
            playlists,
        })
    }

    #[must_use]
    pub const fn source_id(&self) -> &LibrarySourceId {
        &self.source_id
    }
    #[must_use]
    pub fn source_kind(&self) -> &str {
        &self.source_kind
    }
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    #[must_use]
    pub const fn source_revision(&self) -> &SourceRevision {
        &self.source_revision
    }
    #[must_use]
    pub fn tracks(&self) -> &[ImportedTrackAnalysis] {
        &self.tracks
    }
    #[must_use]
    pub fn playlists(&self) -> &[ImportedPlaylist] {
        &self.playlists
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryBaselineValidationError {
    EmptySourceKind,
    EmptyDisplayName,
    EmptyTracks,
    DuplicateTrackIdentity,
    EmptyPlaylistName,
    DuplicatePlaylistIdentity,
    DuplicatePlaylistTrack,
    UnknownPlaylistTrack,
}

impl fmt::Display for LibraryBaselineValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceKind => formatter.write_str("source kind may not be empty"),
            Self::EmptyDisplayName => formatter.write_str("display name may not be empty"),
            Self::EmptyTracks => formatter.write_str("library baseline may not be empty"),
            Self::DuplicateTrackIdentity => {
                formatter.write_str("source track identities must be unique")
            }
            Self::EmptyPlaylistName => formatter.write_str("playlist name may not be empty"),
            Self::DuplicatePlaylistIdentity => {
                formatter.write_str("source playlist identities must be unique")
            }
            Self::DuplicatePlaylistTrack => {
                formatter.write_str("a playlist may contain a track only once")
            }
            Self::UnknownPlaylistTrack => {
                formatter.write_str("playlist references a track outside the baseline")
            }
        }
    }
}

impl Error for LibraryBaselineValidationError {}
