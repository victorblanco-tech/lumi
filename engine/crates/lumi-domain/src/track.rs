use std::error::Error;
use std::fmt;

use crate::TrackId;

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
    duration_beats: u32,
    phrases: Vec<TrackPhrase>,
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
            duration_beats,
            phrases,
        })
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
    pub const fn duration_beats(&self) -> u32 {
        self.duration_beats
    }

    #[must_use]
    pub fn phrases(&self) -> &[TrackPhrase] {
        &self.phrases
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
        }
    }
}

impl Error for TrackValidationError {}
