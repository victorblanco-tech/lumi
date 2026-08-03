use std::error::Error;
use std::fmt;

use lumi_domain::TrackId;

use crate::{PhraseRoleId, SourceRevision, TimelineRevision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineRevisionOrigin {
    SourceImport,
    UserEdit,
    SourceReconcile,
    RevisionRestore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseInstance {
    index: u16,
    start_bar: u32,
    end_bar: u32,
    role_id: PhraseRoleId,
}

impl PhraseInstance {
    #[must_use]
    pub const fn new(index: u16, start_bar: u32, end_bar: u32, role_id: PhraseRoleId) -> Self {
        Self {
            index,
            start_bar,
            end_bar,
            role_id,
        }
    }

    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    #[must_use]
    pub const fn start_bar(&self) -> u32 {
        self.start_bar
    }

    #[must_use]
    pub const fn end_bar(&self) -> u32 {
        self.end_bar
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LumiPhraseTimeline {
    track_id: TrackId,
    revision: TimelineRevision,
    baseline_revision: SourceRevision,
    total_bars: u32,
    origin: TimelineRevisionOrigin,
    phrases: Vec<PhraseInstance>,
}

impl LumiPhraseTimeline {
    pub fn try_new(
        track_id: TrackId,
        revision: TimelineRevision,
        baseline_revision: SourceRevision,
        total_bars: u32,
        origin: TimelineRevisionOrigin,
        phrases: Vec<PhraseInstance>,
    ) -> Result<Self, TimelineValidationError> {
        if track_id.value() == 0 {
            return Err(TimelineValidationError::InvalidTrackId);
        }
        if total_bars == 0 {
            return Err(TimelineValidationError::EmptyDuration);
        }
        if phrases.is_empty() {
            return Err(TimelineValidationError::EmptyPhrases);
        }
        let mut previous_end = 0;
        for (expected_index, phrase) in phrases.iter().enumerate() {
            if usize::from(phrase.index()) != expected_index {
                return Err(TimelineValidationError::UnorderedPhraseIndex);
            }
            if phrase.start_bar() != previous_end
                || phrase.end_bar() <= phrase.start_bar()
                || phrase.end_bar() > total_bars
            {
                return Err(TimelineValidationError::InvalidPhraseCoverage);
            }
            previous_end = phrase.end_bar();
        }
        if previous_end != total_bars {
            return Err(TimelineValidationError::InvalidPhraseCoverage);
        }
        Ok(Self {
            track_id,
            revision,
            baseline_revision,
            total_bars,
            origin,
            phrases,
        })
    }

    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.track_id
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
    pub const fn total_bars(&self) -> u32 {
        self.total_bars
    }

    #[must_use]
    pub const fn origin(&self) -> TimelineRevisionOrigin {
        self.origin
    }

    #[must_use]
    pub fn phrases(&self) -> &[PhraseInstance] {
        &self.phrases
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineValidationError {
    InvalidTrackId,
    EmptyDuration,
    EmptyPhrases,
    UnorderedPhraseIndex,
    InvalidPhraseCoverage,
}

impl fmt::Display for TimelineValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTrackId => formatter.write_str("timeline track id must be positive"),
            Self::EmptyDuration => formatter.write_str("timeline must contain at least one bar"),
            Self::EmptyPhrases => formatter.write_str("timeline must contain phrases"),
            Self::UnorderedPhraseIndex => {
                formatter.write_str("phrase indexes must be contiguous and ordered")
            }
            Self::InvalidPhraseCoverage => {
                formatter.write_str("phrases must cover every complete bar without gaps")
            }
        }
    }
}

impl Error for TimelineValidationError {}
