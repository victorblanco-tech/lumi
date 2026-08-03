//! Provider-neutral music-library model and application-owned repository port.
//!
//! This crate owns canonical Lumi library concepts. It deliberately contains no
//! SQL, Rekordbox, filesystem, serialization, async runtime, or UI types.

#![forbid(unsafe_code)]

mod baseline;
mod identifiers;
mod repository;
mod timeline;

pub use baseline::{
    BeatGrid, BeatGridValidationError, BeatMarker, ImportedLibraryBaseline, ImportedPlaylist,
    ImportedTrackAnalysis, LibraryBaselineValidationError, RawPhraseObservation, TrackColor,
    TrackValidationError, WaveformPoint,
};
pub use identifiers::{
    LibrarySourceId, PhraseRoleId, PlaylistId, SourcePlaylistId, SourceRevision, SourceTrackId,
    TextIdentifierError, TimelineRevision,
};
pub use repository::{
    ImportResult, LibraryRepository, PhraseRole, PlaylistPage, PlaylistSummary, StoredTrack,
    TimelineRevisionPage, TimelineRevisionSummary, TrackPage, TrackPageRequest,
    TrackPageRequestError, TrackSummary,
};
pub use timeline::{
    LumiPhraseTimeline, PhraseInstance, TimelineRevisionOrigin, TimelineValidationError,
};
