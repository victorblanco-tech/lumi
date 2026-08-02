use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{CueId, DeckId, PlanId, PlanRevision, SceneId, ThemeId, TrackLoadId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticLightingAction {
    SelectTheme(ThemeId),
    SelectScene(SceneId),
    ActivateLoop { bank: u8, slot: u8 },
    HoldCurrentLook,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingCue {
    id: CueId,
    phrase_index: u16,
    action: SemanticLightingAction,
}

impl LightingCue {
    #[must_use]
    pub const fn new(id: CueId, phrase_index: u16, action: SemanticLightingAction) -> Self {
        Self {
            id,
            phrase_index,
            action,
        }
    }

    #[must_use]
    pub const fn id(&self) -> CueId {
        self.id
    }

    #[must_use]
    pub const fn phrase_index(&self) -> u16 {
        self.phrase_index
    }

    #[must_use]
    pub const fn action(&self) -> &SemanticLightingAction {
        &self.action
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingPlan {
    id: PlanId,
    deck_id: DeckId,
    track_load_id: TrackLoadId,
    revision: PlanRevision,
    cues: Vec<LightingCue>,
}

impl LightingPlan {
    pub fn try_new(
        id: PlanId,
        deck_id: DeckId,
        track_load_id: TrackLoadId,
        revision: PlanRevision,
        cues: Vec<LightingCue>,
    ) -> Result<Self, PlanValidationError> {
        if revision.value() == 0 {
            return Err(PlanValidationError::ZeroRevision);
        }
        if cues.is_empty() {
            return Err(PlanValidationError::EmptyCues);
        }

        let mut cue_ids = BTreeSet::new();
        let mut previous_phrase = None;
        for cue in &cues {
            if !cue_ids.insert(cue.id()) {
                return Err(PlanValidationError::DuplicateCueId(cue.id()));
            }
            if previous_phrase.is_some_and(|previous| cue.phrase_index() <= previous) {
                return Err(PlanValidationError::UnorderedPhraseIndex);
            }
            previous_phrase = Some(cue.phrase_index());
        }

        Ok(Self {
            id,
            deck_id,
            track_load_id,
            revision,
            cues,
        })
    }

    #[must_use]
    pub const fn id(&self) -> PlanId {
        self.id
    }

    #[must_use]
    pub const fn deck_id(&self) -> DeckId {
        self.deck_id
    }

    #[must_use]
    pub const fn track_load_id(&self) -> TrackLoadId {
        self.track_load_id
    }

    #[must_use]
    pub const fn revision(&self) -> PlanRevision {
        self.revision
    }

    #[must_use]
    pub fn cues(&self) -> &[LightingCue] {
        &self.cues
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanValidationError {
    ZeroRevision,
    EmptyCues,
    DuplicateCueId(CueId),
    UnorderedPhraseIndex,
}

impl fmt::Display for PlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRevision => formatter.write_str("a plan revision must be greater than zero"),
            Self::EmptyCues => formatter.write_str("a lighting plan must contain at least one cue"),
            Self::DuplicateCueId(id) => write!(formatter, "cue ID {} is duplicated", id.value()),
            Self::UnorderedPhraseIndex => {
                formatter.write_str("cue phrase indexes must be strictly increasing")
            }
        }
    }
}

impl Error for PlanValidationError {}
