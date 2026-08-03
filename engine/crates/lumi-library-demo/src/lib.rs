//! Deterministic, license-safe music-library source for development and CI.

#![forbid(unsafe_code)]

use lumi_domain::{KeyMode, MusicalKey, PitchClass};
use lumi_library::{
    BeatGrid, BeatGridValidationError, BeatMarker, ImportedLibraryBaseline, ImportedPlaylist,
    ImportedTrackAnalysis, LibraryBaselineValidationError, LibrarySourceId, RawPhraseObservation,
    SourcePlaylistId, SourceRevision, SourceTrackId, TextIdentifierError, TrackColor,
    TrackValidationError, WaveformPoint,
};
use lumi_library_source::{LibrarySourceCapabilities, MusicLibrarySourceProvider};
use serde::Deserialize;
use thiserror::Error;

const DEMO_LIBRARY_FIXTURE: &str =
    include_str!("../../../../fixtures/demo-library-v1/library.json");
const MILLIS_PER_MINUTE_TIMES_MILLI_BPM: u64 = 60_000_000;
const DEMO_AUDIO_SAMPLE_RATE_HZ: u32 = 44_100;
const MAX_AUDIO_SEGMENT_MILLIS: u32 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemoLibrarySize {
    Curated,
    Scaled(u32),
}

pub struct DemoLibrarySourceProvider {
    size: DemoLibrarySize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoAudioSegment {
    sample_rate_hz: u32,
    start_millis: u64,
    samples: Vec<i16>,
}

impl DemoAudioSegment {
    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    #[must_use]
    pub const fn channel_count(&self) -> u8 {
        1
    }

    #[must_use]
    pub const fn start_millis(&self) -> u64 {
        self.start_millis
    }

    #[must_use]
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
}

impl DemoLibrarySourceProvider {
    #[must_use]
    pub const fn curated() -> Self {
        Self {
            size: DemoLibrarySize::Curated,
        }
    }

    pub fn scaled(track_count: u32) -> Result<Self, DemoLibraryError> {
        if track_count == 0 || track_count > 10_000 {
            return Err(DemoLibraryError::InvalidTrackCount(track_count));
        }
        Ok(Self {
            size: DemoLibrarySize::Scaled(track_count),
        })
    }

    pub fn render_audio_segment(
        &self,
        audio_uri: &str,
        start_millis: u64,
        duration_millis: u32,
    ) -> Result<DemoAudioSegment, DemoLibraryError> {
        if duration_millis == 0 || duration_millis > MAX_AUDIO_SEGMENT_MILLIS {
            return Err(DemoLibraryError::InvalidAudioSegmentDuration(
                duration_millis,
            ));
        }
        let baseline = self.load_baseline()?;
        let track = baseline
            .tracks()
            .iter()
            .find(|track| track.audio_uri() == audio_uri)
            .ok_or_else(|| DemoLibraryError::UnknownAudioUri(audio_uri.to_owned()))?;
        if start_millis >= track.duration_millis() {
            return Err(DemoLibraryError::AudioSegmentOutsideTrack);
        }
        let available_millis = track.duration_millis() - start_millis;
        let rendered_millis = available_millis.min(u64::from(duration_millis));
        let sample_count = rendered_millis
            .checked_mul(u64::from(DEMO_AUDIO_SAMPLE_RATE_HZ))
            .ok_or(DemoLibraryError::ArithmeticOverflow)?
            / 1_000;
        let start_sample = start_millis
            .checked_mul(u64::from(DEMO_AUDIO_SAMPLE_RATE_HZ))
            .ok_or(DemoLibraryError::ArithmeticOverflow)?
            / 1_000;
        let seed = audio_uri.bytes().fold(2_166_136_261_u32, |state, byte| {
            state.wrapping_mul(16_777_619) ^ u32::from(byte)
        });
        let frequency_hz = 110_u64 + u64::from(seed % 220);
        let period = (u64::from(DEMO_AUDIO_SAMPLE_RATE_HZ) / frequency_hz).max(2);
        let mut samples = Vec::with_capacity(
            usize::try_from(sample_count).map_err(|_| DemoLibraryError::ArithmeticOverflow)?,
        );
        for offset in 0..sample_count {
            let phase = (start_sample + offset) % period;
            let rising = phase.saturating_mul(2).min(period);
            let triangle = if phase <= period / 2 {
                rising
            } else {
                (period - phase).saturating_mul(2)
            };
            let centered = i64::try_from(triangle)
                .map_err(|_| DemoLibraryError::ArithmeticOverflow)?
                .saturating_mul(12_000)
                / i64::try_from(period).map_err(|_| DemoLibraryError::ArithmeticOverflow)?
                - 6_000;
            samples
                .push(i16::try_from(centered).map_err(|_| DemoLibraryError::ArithmeticOverflow)?);
        }
        Ok(DemoAudioSegment {
            sample_rate_hz: DEMO_AUDIO_SAMPLE_RATE_HZ,
            start_millis,
            samples,
        })
    }

    fn curated_baseline(&self) -> Result<ImportedLibraryBaseline, DemoLibraryError> {
        let fixture: DemoLibraryFixture = serde_json::from_str(DEMO_LIBRARY_FIXTURE)?;
        if fixture.schema_version != 1 {
            return Err(DemoLibraryError::UnsupportedFixtureVersion(
                fixture.schema_version,
            ));
        }
        let tracks = fixture
            .tracks
            .into_iter()
            .map(track_from_fixture)
            .collect::<Result<Vec<_>, DemoLibraryError>>()?;
        let playlists = fixture
            .playlists
            .into_iter()
            .map(|playlist| {
                ImportedPlaylist::try_new(
                    SourcePlaylistId::try_new(playlist.source_playlist_id)?,
                    playlist.name,
                    playlist
                        .track_ids
                        .into_iter()
                        .map(SourceTrackId::try_new)
                        .collect::<Result<Vec<_>, TextIdentifierError>>()?,
                )
                .map_err(DemoLibraryError::InvalidBaseline)
            })
            .collect::<Result<Vec<_>, DemoLibraryError>>()?;
        ImportedLibraryBaseline::try_new(
            LibrarySourceId::try_new(fixture.source.id)?,
            fixture.source.kind,
            fixture.source.display_name,
            SourceRevision::try_new(fixture.source.revision)?,
            tracks,
            playlists,
        )
        .map_err(DemoLibraryError::InvalidBaseline)
    }

    fn scaled_baseline(
        &self,
        track_count: u32,
    ) -> Result<ImportedLibraryBaseline, DemoLibraryError> {
        let mut tracks = Vec::with_capacity(
            usize::try_from(track_count)
                .map_err(|_| DemoLibraryError::InvalidTrackCount(track_count))?,
        );
        for index in 0..track_count {
            let bpm_milli = 118_000 + (index % 28) * 1_000;
            let beat_grid = generated_beat_grid(bpm_milli, 2)?;
            let waveform = generated_waveform(index.wrapping_add(1), 8);
            tracks.push(ImportedTrackAnalysis::try_new(
                SourceTrackId::try_new(format!("scale-{index:05}"))?,
                SourceRevision::try_new("analysis-v1")?,
                format!("Demo Track {:05}", index + 1),
                "Lumi Procedural Audio",
                bpm_milli,
                MusicalKey::new(PitchClass::A, KeyMode::Minor),
                duration_millis(&beat_grid, bpm_milli),
                Some(TrackColor::new(
                    u8::try_from(index % 256).unwrap_or(0),
                    u8::try_from((index * 3) % 256).unwrap_or(0),
                    u8::try_from((index * 7) % 256).unwrap_or(0),
                )),
                format!("lumi-demo://audio/scale-{index:05}"),
                beat_grid,
                waveform,
                vec![RawPhraseObservation::try_new(0, 8, "Demo")?],
            )?);
        }
        ImportedLibraryBaseline::try_new(
            LibrarySourceId::try_new("lumi-demo-scale")?,
            "demo",
            "Lumi Scale Fixture",
            SourceRevision::try_new(format!("scale-v1-{track_count}"))?,
            tracks,
            vec![
                ImportedPlaylist::try_new(
                    SourcePlaylistId::try_new("all-scale-tracks")?,
                    "All Scale Tracks",
                    (0..track_count)
                        .map(|index| SourceTrackId::try_new(format!("scale-{index:05}")))
                        .collect::<Result<Vec<_>, TextIdentifierError>>()?,
                )
                .map_err(DemoLibraryError::InvalidBaseline)?,
            ],
        )
        .map_err(DemoLibraryError::InvalidBaseline)
    }
}

impl Default for DemoLibrarySourceProvider {
    fn default() -> Self {
        Self::curated()
    }
}

impl MusicLibrarySourceProvider for DemoLibrarySourceProvider {
    type Error = DemoLibraryError;

    fn provider_kind(&self) -> &'static str {
        "demo"
    }

    fn capabilities(&self) -> LibrarySourceCapabilities {
        LibrarySourceCapabilities::complete_analysis()
    }

    fn load_baseline(&self) -> Result<ImportedLibraryBaseline, Self::Error> {
        match self.size {
            DemoLibrarySize::Curated => self.curated_baseline(),
            DemoLibrarySize::Scaled(track_count) => self.scaled_baseline(track_count),
        }
    }
}

fn track_from_fixture(
    fixture: DemoTrackFixture,
) -> Result<ImportedTrackAnalysis, DemoLibraryError> {
    let beat_grid = generated_beat_grid(fixture.bpm_milli, fixture.beat_grid.bar_count)?;
    let total_beats = u32::try_from(beat_grid.markers().len())
        .map_err(|_| DemoLibraryError::ArithmeticOverflow)?;
    let raw_phrases = fixture
        .raw_phrases
        .into_iter()
        .map(|phrase| {
            let start_beat = phrase
                .start_bar
                .checked_mul(u32::from(beat_grid.beats_per_bar()))
                .ok_or(DemoLibraryError::ArithmeticOverflow)?;
            let end_beat = phrase
                .end_bar
                .checked_mul(u32::from(beat_grid.beats_per_bar()))
                .ok_or(DemoLibraryError::ArithmeticOverflow)?;
            if end_beat > total_beats {
                return Err(DemoLibraryError::InvalidPhraseRange);
            }
            RawPhraseObservation::try_new(start_beat, end_beat, phrase.label)
                .map_err(DemoLibraryError::InvalidTrack)
        })
        .collect::<Result<Vec<_>, DemoLibraryError>>()?;
    ImportedTrackAnalysis::try_new(
        SourceTrackId::try_new(fixture.source_track_id)?,
        SourceRevision::try_new(fixture.analysis_revision)?,
        fixture.title,
        fixture.artist,
        fixture.bpm_milli,
        MusicalKey::new(
            pitch_class(&fixture.key.pitch_class)?,
            key_mode(&fixture.key.mode)?,
        ),
        duration_millis(&beat_grid, fixture.bpm_milli),
        Some(TrackColor::new(
            fixture.color.red,
            fixture.color.green,
            fixture.color.blue,
        )),
        fixture.audio_uri,
        beat_grid,
        generated_waveform(fixture.waveform_seed, 128),
        raw_phrases,
    )
    .map_err(DemoLibraryError::InvalidTrack)
}

fn generated_beat_grid(bpm_milli: u32, bar_count: u32) -> Result<BeatGrid, DemoLibraryError> {
    let beats_per_bar = 4_u8;
    let beat_count = bar_count
        .checked_mul(u32::from(beats_per_bar))
        .ok_or(DemoLibraryError::ArithmeticOverflow)?;
    let mut markers = Vec::with_capacity(
        usize::try_from(beat_count).map_err(|_| DemoLibraryError::ArithmeticOverflow)?,
    );
    for beat_index in 0..beat_count {
        let time_millis = u64::from(beat_index)
            .checked_mul(MILLIS_PER_MINUTE_TIMES_MILLI_BPM)
            .ok_or(DemoLibraryError::ArithmeticOverflow)?
            / u64::from(bpm_milli);
        let bar_index = beat_index / u32::from(beats_per_bar) + 1;
        let beat_in_bar = u8::try_from(beat_index % u32::from(beats_per_bar) + 1)
            .map_err(|_| DemoLibraryError::ArithmeticOverflow)?;
        markers.push(BeatMarker::new(
            beat_index,
            time_millis,
            bar_index,
            beat_in_bar,
        ));
    }
    BeatGrid::try_new(beats_per_bar, markers).map_err(DemoLibraryError::InvalidBeatGrid)
}

fn duration_millis(beat_grid: &BeatGrid, bpm_milli: u32) -> u64 {
    u64::try_from(beat_grid.markers().len())
        .unwrap_or(u64::MAX)
        .saturating_mul(MILLIS_PER_MINUTE_TIMES_MILLI_BPM)
        / u64::from(bpm_milli)
}

fn generated_waveform(mut state: u32, point_count: usize) -> Vec<WaveformPoint> {
    let mut points = Vec::with_capacity(point_count);
    for _ in 0..point_count {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let low = u8::try_from((state >> 16) & 0xff).unwrap_or(0);
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let mid = u8::try_from((state >> 16) & 0xff).unwrap_or(0);
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let high = u8::try_from((state >> 16) & 0xff).unwrap_or(0);
        points.push(WaveformPoint::new(low, mid, high));
    }
    points
}

fn pitch_class(value: &str) -> Result<PitchClass, DemoLibraryError> {
    match value {
        "c" => Ok(PitchClass::C),
        "cSharp" => Ok(PitchClass::CSharp),
        "d" => Ok(PitchClass::D),
        "dSharp" => Ok(PitchClass::DSharp),
        "e" => Ok(PitchClass::E),
        "f" => Ok(PitchClass::F),
        "fSharp" => Ok(PitchClass::FSharp),
        "g" => Ok(PitchClass::G),
        "gSharp" => Ok(PitchClass::GSharp),
        "a" => Ok(PitchClass::A),
        "aSharp" => Ok(PitchClass::ASharp),
        "b" => Ok(PitchClass::B),
        _ => Err(DemoLibraryError::UnknownPitchClass(value.to_owned())),
    }
}

fn key_mode(value: &str) -> Result<KeyMode, DemoLibraryError> {
    match value {
        "major" => Ok(KeyMode::Major),
        "minor" => Ok(KeyMode::Minor),
        _ => Err(DemoLibraryError::UnknownKeyMode(value.to_owned())),
    }
}

#[derive(Debug, Error)]
pub enum DemoLibraryError {
    #[error("invalid demo track count {0}; expected 1..=10000")]
    InvalidTrackCount(u32),
    #[error("unsupported demo fixture schema version {0}")]
    UnsupportedFixtureVersion(u16),
    #[error("demo fixture JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("demo identifier is invalid: {0}")]
    InvalidIdentifier(#[from] TextIdentifierError),
    #[error("demo beat grid is invalid: {0}")]
    InvalidBeatGrid(BeatGridValidationError),
    #[error("demo track is invalid: {0}")]
    InvalidTrack(#[from] TrackValidationError),
    #[error("demo baseline is invalid: {0}")]
    InvalidBaseline(LibraryBaselineValidationError),
    #[error("demo phrase falls outside the generated beat grid")]
    InvalidPhraseRange,
    #[error("demo fixture arithmetic overflow")]
    ArithmeticOverflow,
    #[error("demo audio segment duration {0} ms is outside 1..=30000")]
    InvalidAudioSegmentDuration(u32),
    #[error("unknown demo audio URI {0}")]
    UnknownAudioUri(String),
    #[error("demo audio segment starts outside the track")]
    AudioSegmentOutsideTrack,
    #[error("unknown demo pitch class {0}")]
    UnknownPitchClass(String),
    #[error("unknown demo key mode {0}")]
    UnknownKeyMode(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoLibraryFixture {
    schema_version: u16,
    source: DemoSourceFixture,
    tracks: Vec<DemoTrackFixture>,
    playlists: Vec<DemoPlaylistFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoPlaylistFixture {
    source_playlist_id: String,
    name: String,
    track_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoSourceFixture {
    id: String,
    kind: String,
    display_name: String,
    revision: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoTrackFixture {
    source_track_id: String,
    analysis_revision: String,
    title: String,
    artist: String,
    bpm_milli: u32,
    key: DemoKeyFixture,
    color: DemoColorFixture,
    audio_uri: String,
    beat_grid: DemoBeatGridFixture,
    waveform_seed: u32,
    raw_phrases: Vec<DemoPhraseFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoKeyFixture {
    pitch_class: String,
    mode: String,
}

#[derive(Deserialize)]
struct DemoColorFixture {
    red: u8,
    green: u8,
    blue: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoBeatGridFixture {
    bar_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoPhraseFixture {
    start_bar: u32,
    end_bar: u32,
    label: String,
}
