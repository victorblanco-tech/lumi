use std::error::Error;
use std::fmt;

use crate::TrackId;

/// Stable, provider-neutral identity facts attached to a library-backed deck
/// load. A deck-source adapter may omit these facts for an unknown live track,
/// but it must never substitute a different library record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackIdentityFacts {
    provider_kind: String,
    source_id: String,
    source_track_id: String,
    analysis_revision: String,
    lumi_timeline_revision: u64,
}

impl TrackIdentityFacts {
    pub fn try_new(
        provider_kind: impl Into<String>,
        source_id: impl Into<String>,
        source_track_id: impl Into<String>,
        analysis_revision: impl Into<String>,
        lumi_timeline_revision: u64,
    ) -> Result<Self, TrackValidationError> {
        let provider_kind = provider_kind.into();
        let source_id = source_id.into();
        let source_track_id = source_track_id.into();
        let analysis_revision = analysis_revision.into();
        if provider_kind.trim().is_empty()
            || source_id.trim().is_empty()
            || source_track_id.trim().is_empty()
            || analysis_revision.trim().is_empty()
            || lumi_timeline_revision == 0
        {
            return Err(TrackValidationError::InvalidIdentityFacts);
        }
        Ok(Self {
            provider_kind,
            source_id,
            source_track_id,
            analysis_revision,
            lumi_timeline_revision,
        })
    }

    #[must_use]
    pub fn provider_kind(&self) -> &str {
        &self.provider_kind
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub fn source_track_id(&self) -> &str {
        &self.source_track_id
    }

    #[must_use]
    pub fn analysis_revision(&self) -> &str {
        &self.analysis_revision
    }

    #[must_use]
    pub const fn lumi_timeline_revision(&self) -> u64 {
        self.lumi_timeline_revision
    }
}

/// Canonical normalized sRGB color supplied by a deck or library adapter.
/// Provider-specific color indexes and labels must be translated before this
/// value enters the Lumi domain.
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
pub enum DeckSourceStatus {
    Starting,
    Ready,
    Degraded,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PitchClass {
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyMode {
    Major,
    Minor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MusicalKey {
    pitch_class: PitchClass,
    mode: KeyMode,
}

impl MusicalKey {
    #[must_use]
    pub const fn new(pitch_class: PitchClass, mode: KeyMode) -> Self {
        Self { pitch_class, mode }
    }

    #[must_use]
    pub const fn pitch_class(self) -> PitchClass {
        self.pitch_class
    }

    #[must_use]
    pub const fn mode(self) -> KeyMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhraseKind {
    Intro,
    Verse,
    Build,
    Drop,
    Breakdown,
    Outro,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackPhrase {
    index: u16,
    start_beat: u32,
    end_beat: u32,
    kind: PhraseKind,
}

impl TrackPhrase {
    #[must_use]
    pub const fn new(index: u16, start_beat: u32, end_beat: u32, kind: PhraseKind) -> Self {
        Self {
            index,
            start_beat,
            end_beat,
            kind,
        }
    }

    #[must_use]
    pub const fn index(self) -> u16 {
        self.index
    }

    #[must_use]
    pub const fn start_beat(self) -> u32 {
        self.start_beat
    }

    #[must_use]
    pub const fn end_beat(self) -> u32 {
        self.end_beat
    }

    #[must_use]
    pub const fn kind(self) -> PhraseKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackMetadata {
    id: TrackId,
    title: String,
    artist: String,
    bpm_milli: u32,
    musical_key: MusicalKey,
    color: Option<TrackColor>,
    duration_beats: u32,
    phrases: Vec<TrackPhrase>,
    identity_facts: Option<TrackIdentityFacts>,
}

impl TrackMetadata {
    pub fn try_new(
        id: TrackId,
        title: String,
        artist: String,
        bpm_milli: u32,
        musical_key: MusicalKey,
        duration_beats: u32,
        phrases: Vec<TrackPhrase>,
    ) -> Result<Self, TrackValidationError> {
        Self::try_new_with_color(
            id,
            title,
            artist,
            bpm_milli,
            musical_key,
            None,
            duration_beats,
            phrases,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new_with_color(
        id: TrackId,
        title: String,
        artist: String,
        bpm_milli: u32,
        musical_key: MusicalKey,
        color: Option<TrackColor>,
        duration_beats: u32,
        phrases: Vec<TrackPhrase>,
    ) -> Result<Self, TrackValidationError> {
        if title.trim().is_empty() {
            return Err(TrackValidationError::EmptyTitle);
        }
        if artist.trim().is_empty() {
            return Err(TrackValidationError::EmptyArtist);
        }
        if !(20_000..=300_000).contains(&bpm_milli) {
            return Err(TrackValidationError::BpmOutOfRange(bpm_milli));
        }
        if duration_beats == 0 {
            return Err(TrackValidationError::EmptyDuration);
        }
        if phrases.is_empty() {
            return Err(TrackValidationError::EmptyPhrases);
        }

        let mut previous_end = 0;
        for (expected_index, phrase) in phrases.iter().enumerate() {
            if usize::from(phrase.index()) != expected_index {
                return Err(TrackValidationError::UnorderedPhraseIndex);
            }
            if phrase.start_beat() != previous_end
                || phrase.end_beat() <= phrase.start_beat()
                || phrase.end_beat() > duration_beats
            {
                return Err(TrackValidationError::InvalidPhraseRange);
            }
            previous_end = phrase.end_beat();
        }
        if previous_end != duration_beats {
            return Err(TrackValidationError::IncompletePhraseCoverage);
        }

        Ok(Self {
            id,
            title,
            artist,
            bpm_milli,
            musical_key,
            color,
            duration_beats,
            phrases,
            identity_facts: None,
        })
    }

    #[must_use]
    pub fn with_identity_facts(mut self, identity_facts: TrackIdentityFacts) -> Self {
        self.identity_facts = Some(identity_facts);
        self
    }

    #[must_use]
    pub const fn id(&self) -> TrackId {
        self.id
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
    pub const fn color(&self) -> Option<TrackColor> {
        self.color
    }

    #[must_use]
    pub const fn duration_beats(&self) -> u32 {
        self.duration_beats
    }

    #[must_use]
    pub fn phrases(&self) -> &[TrackPhrase] {
        &self.phrases
    }

    #[must_use]
    pub const fn identity_facts(&self) -> Option<&TrackIdentityFacts> {
        self.identity_facts.as_ref()
    }

    #[must_use]
    pub fn phrase(&self, index: u16) -> Option<TrackPhrase> {
        self.phrases.get(usize::from(index)).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackValidationError {
    EmptyTitle,
    EmptyArtist,
    BpmOutOfRange(u32),
    EmptyDuration,
    EmptyPhrases,
    UnorderedPhraseIndex,
    InvalidPhraseRange,
    IncompletePhraseCoverage,
    InvalidIdentityFacts,
}

impl fmt::Display for TrackValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => formatter.write_str("track title may not be empty"),
            Self::EmptyArtist => formatter.write_str("track artist may not be empty"),
            Self::BpmOutOfRange(value) => write!(formatter, "track BPM {value} is outside range"),
            Self::EmptyDuration => formatter.write_str("track duration must be greater than zero"),
            Self::EmptyPhrases => formatter.write_str("track phrases may not be empty"),
            Self::UnorderedPhraseIndex => {
                formatter.write_str("track phrase indexes must be contiguous and ordered")
            }
            Self::InvalidPhraseRange => {
                formatter.write_str("track phrases must be contiguous and within duration")
            }
            Self::IncompletePhraseCoverage => {
                formatter.write_str("track phrases must cover the complete track")
            }
            Self::InvalidIdentityFacts => {
                formatter.write_str("track identity facts must be complete and revisioned")
            }
        }
    }
}

impl Error for TrackValidationError {}
