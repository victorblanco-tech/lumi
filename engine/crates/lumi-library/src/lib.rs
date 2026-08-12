//! Provider-neutral music-library model and application-owned repository port.
//!
//! This crate owns canonical Lumi library concepts. It deliberately contains no
//! SQL, Rekordbox, filesystem, serialization, async runtime, or UI types.

#![forbid(unsafe_code)]

mod autoloop_catalog;
mod baseline;
mod identifiers;
mod mirror;
mod phrase_roles;
mod reconciliation;
mod repository;
mod timeline;

pub use autoloop_catalog::{
    AUTOLOOP_CATALOG_DEFAULTS_VERSION, AutoloopCatalog, AutoloopCatalogError, AutoloopMatrixCell,
    AutoloopResolution, AutoloopResolutionReason, AutoloopTheme, AutoloopVariant,
    AutoloopVariantMove, MissingAutoloopCell,
};
pub use baseline::{
    BeatGrid, BeatGridValidationError, BeatMarker, HotCue, ImportedLibraryBaseline,
    ImportedPlaylist, ImportedTrackAnalysis, LibraryBaselineValidationError, RawPhraseObservation,
    TrackColor, TrackValidationError, WaveformPoint,
};
pub use identifiers::{
    AutoloopEntryId, LibrarySourceId, PhraseRoleId, PlaylistId, SourcePlaylistId, SourceRevision,
    SourceTrackId, TextIdentifierError, TimelineRevision, VariantId,
};
pub use mirror::{
    SourceMirrorDiff, SourceMirrorPlaylist, SourceMirrorSnapshot, SourceMirrorSummary,
    SourceMirrorTrack, SourceMirrorValidationError,
};
pub use phrase_roles::{
    PHRASE_ROLE_DEFAULTS_VERSION, PhraseRole, PhraseRoleCatalog, PhraseRoleCatalogError,
    PhraseRoleMove, PhraseRoleTrackUsage, PhraseRoleUsage, SourcePhraseMapping,
    normalize_source_label,
};
pub use reconciliation::{
    PhraseConflict, PhraseConflictChoice, ReconcileError, ReconcilePreview, ReconcileSide,
    ReconcileStrategy, SourceChangeClass, SourceTrackDiff, reconcile_timeline,
};
pub use repository::{
    ImportResult, LibraryRepository, LibrarySourceSummary, LibraryTrackQuery, PlaylistPage,
    PlaylistSummary, StoredTrack, TimelineRevisionPage, TimelineRevisionSummary, TrackPage,
    TrackPageRequest, TrackPageRequestError, TrackSummary,
};
pub use timeline::{
    LumiPhraseTimeline, PhraseAbsorption, PhraseInstance, PhraseLoopStrategy, ThemeSpecificVariant,
    TimelineEditCommand, TimelineEditError, TimelineRevisionOrigin, TimelineRevisionReason,
    TimelineValidationError,
};
