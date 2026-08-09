//! Application-owned port for normalized music-library source adapters.

#![forbid(unsafe_code)]

use std::error::Error;

use lumi_library::ImportedLibraryBaseline;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibrarySourceCapabilities {
    pub playlists: bool,
    pub color: bool,
    pub beat_grid: bool,
    pub waveform: bool,
    pub raw_phrases: bool,
    pub local_audio: bool,
}

impl LibrarySourceCapabilities {
    #[must_use]
    pub const fn complete_analysis() -> Self {
        Self {
            playlists: true,
            color: true,
            beat_grid: true,
            waveform: true,
            raw_phrases: true,
            local_audio: true,
        }
    }
}

pub trait MusicLibrarySourceProvider {
    type Error: Error + Send + Sync + 'static;

    fn provider_kind(&self) -> &'static str;
    fn capabilities(&self) -> LibrarySourceCapabilities;
    fn load_baseline(&self) -> Result<ImportedLibraryBaseline, Self::Error>;
}
