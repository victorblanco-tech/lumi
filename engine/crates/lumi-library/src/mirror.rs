use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{LibrarySourceId, SourceRevision, SourceTrackId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMirrorTrack {
    source_track_id: SourceTrackId,
    title: String,
    artist: Option<String>,
    average_bpm: Option<String>,
    musical_key: Option<String>,
    duration_millis: Option<u64>,
    color: Option<String>,
    audio_uri: String,
}

impl SourceMirrorTrack {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        source_track_id: SourceTrackId,
        title: impl Into<String>,
        artist: Option<String>,
        average_bpm: Option<String>,
        musical_key: Option<String>,
        duration_millis: Option<u64>,
        color: Option<String>,
        audio_uri: impl Into<String>,
    ) -> Result<Self, SourceMirrorValidationError> {
        let title = title.into();
        let audio_uri = audio_uri.into();
        if title.trim().is_empty() {
            return Err(SourceMirrorValidationError::EmptyTrackTitle);
        }
        if audio_uri.trim().is_empty() {
            return Err(SourceMirrorValidationError::EmptyAudioUri);
        }
        if duration_millis == Some(0) {
            return Err(SourceMirrorValidationError::InvalidDuration);
        }
        for value in [
            Some(title.as_str()),
            artist.as_deref(),
            average_bpm.as_deref(),
            musical_key.as_deref(),
            color.as_deref(),
            Some(audio_uri.as_str()),
        ]
        .into_iter()
        .flatten()
        {
            if value.len() > 32_768 {
                return Err(SourceMirrorValidationError::TextTooLong);
            }
        }
        Ok(Self {
            source_track_id,
            title,
            artist,
            average_bpm,
            musical_key,
            duration_millis,
            color,
            audio_uri,
        })
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
    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }
    #[must_use]
    pub fn average_bpm(&self) -> Option<&str> {
        self.average_bpm.as_deref()
    }
    #[must_use]
    pub fn musical_key(&self) -> Option<&str> {
        self.musical_key.as_deref()
    }
    #[must_use]
    pub const fn duration_millis(&self) -> Option<u64> {
        self.duration_millis
    }
    #[must_use]
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }
    #[must_use]
    pub fn audio_uri(&self) -> &str {
        &self.audio_uri
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMirrorPlaylist {
    source_playlist_id: String,
    name: String,
    track_ids: Vec<SourceTrackId>,
}

impl SourceMirrorPlaylist {
    pub fn try_new(
        source_playlist_id: impl Into<String>,
        name: impl Into<String>,
        track_ids: Vec<SourceTrackId>,
    ) -> Result<Self, SourceMirrorValidationError> {
        let source_playlist_id = source_playlist_id.into();
        let name = name.into();
        if source_playlist_id.trim().is_empty() || source_playlist_id.len() > 2_048 {
            return Err(SourceMirrorValidationError::InvalidPlaylistIdentity);
        }
        if name.trim().is_empty() || name.len() > 512 {
            return Err(SourceMirrorValidationError::InvalidPlaylistName);
        }
        if track_ids.iter().collect::<BTreeSet<_>>().len() != track_ids.len() {
            return Err(SourceMirrorValidationError::DuplicatePlaylistTrack);
        }
        Ok(Self {
            source_playlist_id,
            name,
            track_ids,
        })
    }

    #[must_use]
    pub fn source_playlist_id(&self) -> &str {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMirrorSnapshot {
    source_id: LibrarySourceId,
    source_kind: String,
    display_name: String,
    source_revision: SourceRevision,
    tracks: Vec<SourceMirrorTrack>,
    playlists: Vec<SourceMirrorPlaylist>,
}

impl SourceMirrorSnapshot {
    pub fn try_new(
        source_id: LibrarySourceId,
        source_kind: impl Into<String>,
        display_name: impl Into<String>,
        source_revision: SourceRevision,
        tracks: Vec<SourceMirrorTrack>,
        playlists: Vec<SourceMirrorPlaylist>,
    ) -> Result<Self, SourceMirrorValidationError> {
        let source_kind = source_kind.into();
        let display_name = display_name.into();
        if source_kind.trim().is_empty() || display_name.trim().is_empty() {
            return Err(SourceMirrorValidationError::InvalidSource);
        }
        if tracks.is_empty() {
            return Err(SourceMirrorValidationError::EmptyTracks);
        }
        let track_ids = tracks
            .iter()
            .map(SourceMirrorTrack::source_track_id)
            .collect::<BTreeSet<_>>();
        if track_ids.len() != tracks.len() {
            return Err(SourceMirrorValidationError::DuplicateTrackIdentity);
        }
        let playlist_ids = playlists
            .iter()
            .map(SourceMirrorPlaylist::source_playlist_id)
            .collect::<BTreeSet<_>>();
        if playlist_ids.len() != playlists.len() {
            return Err(SourceMirrorValidationError::DuplicatePlaylistIdentity);
        }
        if playlists
            .iter()
            .flat_map(SourceMirrorPlaylist::track_ids)
            .any(|track_id| !track_ids.contains(track_id))
        {
            return Err(SourceMirrorValidationError::UnknownPlaylistTrack);
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
    pub fn tracks(&self) -> &[SourceMirrorTrack] {
        &self.tracks
    }
    #[must_use]
    pub fn playlists(&self) -> &[SourceMirrorPlaylist] {
        &self.playlists
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceMirrorDiff {
    pub inserted: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub archived: u32,
    pub restored: u32,
    pub active_tracks: u32,
    pub archived_tracks: u32,
    pub playlists: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMirrorSummary {
    source_id: LibrarySourceId,
    source_kind: String,
    display_name: String,
    source_revision: SourceRevision,
    active_tracks: u32,
    archived_tracks: u32,
    playlists: u32,
}

impl SourceMirrorSummary {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        source_id: LibrarySourceId,
        source_kind: String,
        display_name: String,
        source_revision: SourceRevision,
        active_tracks: u32,
        archived_tracks: u32,
        playlists: u32,
    ) -> Self {
        Self {
            source_id,
            source_kind,
            display_name,
            source_revision,
            active_tracks,
            archived_tracks,
            playlists,
        }
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
    pub const fn active_tracks(&self) -> u32 {
        self.active_tracks
    }
    #[must_use]
    pub const fn archived_tracks(&self) -> u32 {
        self.archived_tracks
    }
    #[must_use]
    pub const fn playlists(&self) -> u32 {
        self.playlists
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMirrorValidationError {
    InvalidSource,
    EmptyTracks,
    EmptyTrackTitle,
    EmptyAudioUri,
    InvalidDuration,
    TextTooLong,
    InvalidPlaylistIdentity,
    InvalidPlaylistName,
    DuplicateTrackIdentity,
    DuplicatePlaylistIdentity,
    DuplicatePlaylistTrack,
    UnknownPlaylistTrack,
}

impl fmt::Display for SourceMirrorValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid source mirror: {self:?}")
    }
}

impl Error for SourceMirrorValidationError {}
