use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    CueId, DeckId, PhraseKind, PlanConfigurationRevision, PlanId, PlanRevision, SceneId, ThemeId,
    TrackId, TrackLoadId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanStatus {
    Ready,
    Fallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCategory {
    Ambient,
    Groove,
    Build,
    Impact,
    Break,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopSelection {
    bank: u8,
    slot: u8,
}

impl LoopSelection {
    #[must_use]
    pub const fn new(bank: u8, slot: u8) -> Self {
        Self { bank, slot }
    }

    #[must_use]
    pub const fn bank(self) -> u8 {
        self.bank
    }

    #[must_use]
    pub const fn slot(self) -> u8 {
        self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingLook {
    theme_id: ThemeId,
    theme_name: String,
    scene_id: SceneId,
    scene_name: String,
    category: SceneCategory,
    loop_selection: LoopSelection,
}

impl LightingLook {
    pub fn try_new(
        theme_id: ThemeId,
        theme_name: String,
        scene_id: SceneId,
        scene_name: String,
        category: SceneCategory,
        loop_selection: LoopSelection,
    ) -> Result<Self, PlanValidationError> {
        if theme_name.trim().is_empty() {
            return Err(PlanValidationError::EmptyThemeName);
        }
        if scene_name.trim().is_empty() {
            return Err(PlanValidationError::EmptySceneName);
        }
        Ok(Self {
            theme_id,
            theme_name,
            scene_id,
            scene_name,
            category,
            loop_selection,
        })
    }

    #[must_use]
    pub const fn theme_id(&self) -> ThemeId {
        self.theme_id
    }

    #[must_use]
    pub fn theme_name(&self) -> &str {
        &self.theme_name
    }

    #[must_use]
    pub const fn scene_id(&self) -> SceneId {
        self.scene_id
    }

    #[must_use]
    pub fn scene_name(&self) -> &str {
        &self.scene_name
    }

    #[must_use]
    pub const fn category(&self) -> SceneCategory {
        self.category
    }

    #[must_use]
    pub const fn loop_selection(&self) -> LoopSelection {
        self.loop_selection
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticLightingAction {
    ApplyLook(LightingLook),
    HoldCurrentLook,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueOrigin {
    Automatic,
    Fallback,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueReason {
    PhraseCategoryMatched {
        phrase_kind: PhraseKind,
        category: SceneCategory,
    },
    MissingPhraseAnalysis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingCue {
    id: CueId,
    phrase_index: u16,
    start_beat: u32,
    end_beat: u32,
    action: SemanticLightingAction,
    origin: CueOrigin,
    reason: CueReason,
    locked: bool,
}

impl LightingCue {
    #[must_use]
    pub const fn new(
        id: CueId,
        phrase_index: u16,
        start_beat: u32,
        end_beat: u32,
        action: SemanticLightingAction,
        origin: CueOrigin,
        reason: CueReason,
    ) -> Self {
        Self {
            id,
            phrase_index,
            start_beat,
            end_beat,
            action,
            origin,
            reason,
            locked: false,
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
    pub const fn start_beat(&self) -> u32 {
        self.start_beat
    }

    #[must_use]
    pub const fn end_beat(&self) -> u32 {
        self.end_beat
    }

    #[must_use]
    pub const fn action(&self) -> &SemanticLightingAction {
        &self.action
    }

    #[must_use]
    pub const fn origin(&self) -> CueOrigin {
        self.origin
    }

    #[must_use]
    pub const fn reason(&self) -> CueReason {
        self.reason
    }

    #[must_use]
    pub const fn locked(&self) -> bool {
        self.locked
    }

    #[must_use]
    pub fn revised(&self, action: SemanticLightingAction, origin: CueOrigin, locked: bool) -> Self {
        Self {
            id: self.id,
            phrase_index: self.phrase_index,
            start_beat: self.start_beat,
            end_beat: self.end_beat,
            action,
            origin,
            reason: self.reason,
            locked,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingPlan {
    id: PlanId,
    deck_id: DeckId,
    track_id: TrackId,
    track_duration_beats: u32,
    track_load_id: TrackLoadId,
    revision: PlanRevision,
    configuration_revision: PlanConfigurationRevision,
    seed: u64,
    status: PlanStatus,
    cues: Vec<LightingCue>,
}

impl LightingPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        id: PlanId,
        deck_id: DeckId,
        track_id: TrackId,
        track_duration_beats: u32,
        track_load_id: TrackLoadId,
        revision: PlanRevision,
        configuration_revision: PlanConfigurationRevision,
        seed: u64,
        status: PlanStatus,
        cues: Vec<LightingCue>,
    ) -> Result<Self, PlanValidationError> {
        if revision.value() == 0 {
            return Err(PlanValidationError::ZeroRevision);
        }
        if configuration_revision.value() == 0 {
            return Err(PlanValidationError::ZeroConfigurationRevision);
        }
        if track_duration_beats == 0 {
            return Err(PlanValidationError::EmptyTrackDuration);
        }
        if cues.is_empty() {
            return Err(PlanValidationError::EmptyCues);
        }

        let mut cue_ids = BTreeSet::new();
        let mut previous_end = 0;
        let mut contains_fallback = false;
        for (expected_index, cue) in cues.iter().enumerate() {
            if !cue_ids.insert(cue.id()) {
                return Err(PlanValidationError::DuplicateCueId(cue.id()));
            }
            if usize::from(cue.phrase_index()) != expected_index {
                return Err(PlanValidationError::UnorderedPhraseIndex);
            }
            if cue.start_beat() != previous_end || cue.end_beat() <= cue.start_beat() {
                return Err(PlanValidationError::InvalidCueRange);
            }
            previous_end = cue.end_beat();
            contains_fallback |= cue.origin() == CueOrigin::Fallback;
        }
        match (status, contains_fallback) {
            (PlanStatus::Ready, true) => return Err(PlanValidationError::FallbackCueInReadyPlan),
            (PlanStatus::Fallback, false) => {
                return Err(PlanValidationError::FallbackPlanWithoutFallbackCue);
            }
            _ => {}
        }
        if previous_end != track_duration_beats {
            return Err(PlanValidationError::IncompleteCueCoverage);
        }

        Ok(Self {
            id,
            deck_id,
            track_id,
            track_duration_beats,
            track_load_id,
            revision,
            configuration_revision,
            seed,
            status,
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
    pub const fn track_id(&self) -> TrackId {
        self.track_id
    }

    #[must_use]
    pub const fn track_duration_beats(&self) -> u32 {
        self.track_duration_beats
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
    pub const fn configuration_revision(&self) -> PlanConfigurationRevision {
        self.configuration_revision
    }

    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    #[must_use]
    pub const fn status(&self) -> PlanStatus {
        self.status
    }

    #[must_use]
    pub fn cues(&self) -> &[LightingCue] {
        &self.cues
    }

    pub fn revised(&self, cues: Vec<LightingCue>) -> Result<Self, PlanValidationError> {
        let revision = self
            .revision
            .checked_next()
            .ok_or(PlanValidationError::RevisionOverflow)?;
        Self::try_new(
            self.id,
            self.deck_id,
            self.track_id,
            self.track_duration_beats,
            self.track_load_id,
            revision,
            self.configuration_revision,
            self.seed,
            self.status,
            cues,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanValidationError {
    ZeroRevision,
    ZeroConfigurationRevision,
    EmptyTrackDuration,
    EmptyCues,
    EmptyThemeName,
    EmptySceneName,
    DuplicateCueId(CueId),
    UnorderedPhraseIndex,
    InvalidCueRange,
    IncompleteCueCoverage,
    FallbackCueInReadyPlan,
    FallbackPlanWithoutFallbackCue,
    RevisionOverflow,
}

impl fmt::Display for PlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRevision => formatter.write_str("a plan revision must be greater than zero"),
            Self::ZeroConfigurationRevision => {
                formatter.write_str("a configuration revision must be greater than zero")
            }
            Self::EmptyTrackDuration => {
                formatter.write_str("a plan track duration must be greater than zero")
            }
            Self::EmptyCues => formatter.write_str("a lighting plan must contain at least one cue"),
            Self::EmptyThemeName => formatter.write_str("a theme name may not be empty"),
            Self::EmptySceneName => formatter.write_str("a scene name may not be empty"),
            Self::DuplicateCueId(id) => write!(formatter, "cue ID {} is duplicated", id.value()),
            Self::UnorderedPhraseIndex => {
                formatter.write_str("cue phrase indexes must be contiguous and ordered")
            }
            Self::InvalidCueRange => {
                formatter.write_str("cue beat ranges must be contiguous and non-empty")
            }
            Self::IncompleteCueCoverage => {
                formatter.write_str("plan cues must cover the complete track")
            }
            Self::FallbackCueInReadyPlan => {
                formatter.write_str("a ready plan may not contain a fallback cue")
            }
            Self::FallbackPlanWithoutFallbackCue => {
                formatter.write_str("a fallback plan must contain a fallback cue")
            }
            Self::RevisionOverflow => formatter.write_str("plan revision overflow"),
        }
    }
}

impl Error for PlanValidationError {}
