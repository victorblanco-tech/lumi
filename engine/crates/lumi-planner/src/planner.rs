use std::error::Error;
use std::fmt;

use lumi_domain::{
    CueId, CueOrigin, CueReason, DeckId, LightingCue, LightingLook, LightingPlan, LoopSelection,
    PhraseKind, PlanConfigurationRevision, PlanId, PlanRevision, PlanStatus, PlanValidationError,
    SceneCategory, SceneId, SemanticLightingAction, ThemeId, TrackId, TrackLoadId, TrackMetadata,
    TrackPhrase,
};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannerTrack {
    id: TrackId,
    duration_beats: u32,
    phrases: Option<Vec<TrackPhrase>>,
}

impl PlannerTrack {
    #[must_use]
    pub fn analyzed(metadata: &TrackMetadata) -> Self {
        Self {
            id: metadata.id(),
            duration_beats: metadata.duration_beats(),
            phrases: Some(metadata.phrases().to_vec()),
        }
    }

    #[must_use]
    pub const fn without_analysis(id: TrackId, duration_beats: u32) -> Self {
        Self {
            id,
            duration_beats,
            phrases: None,
        }
    }

    #[must_use]
    pub fn with_analysis(id: TrackId, duration_beats: u32, phrases: Vec<TrackPhrase>) -> Self {
        Self {
            id,
            duration_beats,
            phrases: Some(phrases),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningInput {
    pub deck_id: DeckId,
    pub track_load_id: TrackLoadId,
    pub track: PlannerTrack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ThemeDefinition {
    id: ThemeId,
    name: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SceneDefinition {
    id: SceneId,
    name: &'static str,
    category: SceneCategory,
    loop_selection: LoopSelection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningConfiguration {
    revision: PlanConfigurationRevision,
    themes: Vec<ThemeDefinition>,
    scenes: Vec<SceneDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeOption {
    pub id: ThemeId,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneOption {
    pub id: SceneId,
    pub name: String,
    pub category: SceneCategory,
    pub loop_selection: LoopSelection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningOptions {
    pub themes: Vec<ThemeOption>,
    pub scenes: Vec<SceneOption>,
}

impl PlanningConfiguration {
    #[must_use]
    pub fn epic_one() -> Self {
        Self {
            revision: PlanConfigurationRevision::new(1),
            themes: vec![
                ThemeDefinition {
                    id: ThemeId::new(1),
                    name: "Midnight Drive",
                },
                ThemeDefinition {
                    id: ThemeId::new(2),
                    name: "Electric Bloom",
                },
            ],
            scenes: vec![
                scene(1, "Soft Motion", SceneCategory::Ambient, 1, 1),
                scene(2, "Star Wash", SceneCategory::Ambient, 1, 2),
                scene(3, "Neon Motion", SceneCategory::Groove, 2, 1),
                scene(4, "Prism Sweep", SceneCategory::Groove, 2, 2),
                scene(5, "Rising Pulse", SceneCategory::Build, 3, 1),
                scene(6, "Velocity Build", SceneCategory::Build, 3, 2),
                scene(7, "Full Energy", SceneCategory::Impact, 4, 1),
                scene(8, "Color Impact", SceneCategory::Impact, 4, 2),
                scene(9, "Deep Space", SceneCategory::Break, 5, 1),
                scene(10, "Slow Wave", SceneCategory::Break, 5, 2),
            ],
        }
    }

    #[must_use]
    pub const fn revision(&self) -> PlanConfigurationRevision {
        self.revision
    }
}

const fn scene(
    id: u64,
    name: &'static str,
    category: SceneCategory,
    bank: u8,
    slot: u8,
) -> SceneDefinition {
    SceneDefinition {
        id: SceneId::new(id),
        name,
        category,
        loop_selection: LoopSelection::new(bank, slot),
    }
}

pub trait ChoiceSource {
    fn choose(&self, seed: u64, decision: u64, candidate_count: usize) -> Option<usize>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StableChoiceSource;

impl ChoiceSource for StableChoiceSource {
    fn choose(&self, seed: u64, decision: u64, candidate_count: usize) -> Option<usize> {
        if candidate_count == 0 {
            return None;
        }
        let mixed = mix(seed ^ decision.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        Some((mixed % candidate_count as u64) as usize)
    }
}

pub struct DeterministicPlanner<C: ChoiceSource> {
    configuration: PlanningConfiguration,
    choice_source: C,
}

impl DeterministicPlanner<StableChoiceSource> {
    #[must_use]
    pub fn epic_one() -> Self {
        Self {
            configuration: PlanningConfiguration::epic_one(),
            choice_source: StableChoiceSource,
        }
    }
}

impl<C: ChoiceSource> DeterministicPlanner<C> {
    #[must_use]
    pub const fn new(configuration: PlanningConfiguration, choice_source: C) -> Self {
        Self {
            configuration,
            choice_source,
        }
    }

    pub fn generate(&self, input: &PlanningInput) -> Result<LightingPlan, PlannerError> {
        if input.track.duration_beats == 0 {
            return Err(PlannerError::EmptyTrackDuration);
        }
        let seed = stable_seed(input.track.id, self.configuration.revision);
        let plan_id = PlanId::new(nonzero(mix(seed ^ input.track_load_id.value())));
        let cues = match input.track.phrases.as_deref() {
            Some(phrases) if analysis_is_complete(phrases, input.track.duration_beats) => {
                self.plan_analyzed_phrases(seed, plan_id, phrases)?
            }
            _ => vec![LightingCue::new(
                CueId::new(nonzero(mix(plan_id.value()))),
                0,
                0,
                input.track.duration_beats,
                SemanticLightingAction::HoldCurrentLook,
                CueOrigin::Fallback,
                CueReason::MissingPhraseAnalysis,
            )],
        };
        let status = if cues.iter().any(|cue| cue.origin() == CueOrigin::Fallback) {
            PlanStatus::Fallback
        } else {
            PlanStatus::Ready
        };

        LightingPlan::try_new(
            plan_id,
            input.deck_id,
            input.track.id,
            input.track.duration_beats,
            input.track_load_id,
            PlanRevision::initial(),
            self.configuration.revision,
            seed,
            status,
            cues,
        )
        .map_err(PlannerError::InvalidPlan)
    }

    #[must_use]
    pub fn options(&self) -> PlanningOptions {
        PlanningOptions {
            themes: self
                .configuration
                .themes
                .iter()
                .map(|theme| ThemeOption {
                    id: theme.id,
                    name: theme.name.to_owned(),
                })
                .collect(),
            scenes: self
                .configuration
                .scenes
                .iter()
                .map(|scene| SceneOption {
                    id: scene.id,
                    name: scene.name.to_owned(),
                    category: scene.category,
                    loop_selection: scene.loop_selection,
                })
                .collect(),
        }
    }

    pub fn select_theme(
        &self,
        current: &LightingPlan,
        theme_id: ThemeId,
    ) -> Result<LightingPlan, PlanMutationError> {
        ensure_ready(current)?;
        let theme = self
            .configuration
            .themes
            .iter()
            .find(|candidate| candidate.id == theme_id)
            .ok_or(PlanMutationError::UnknownTheme(theme_id))?;
        let mut changed = false;
        let cues = current
            .cues()
            .iter()
            .map(|cue| {
                if cue.locked() {
                    return Ok(cue.clone());
                }
                match cue.action() {
                    SemanticLightingAction::ApplyLook(look) => {
                        changed |= look.theme_id() != theme.id;
                        let revised = LightingLook::try_new(
                            theme.id,
                            theme.name.to_owned(),
                            look.scene_id(),
                            look.scene_name().to_owned(),
                            look.category(),
                            look.loop_selection(),
                        )?;
                        Ok(cue.revised(
                            SemanticLightingAction::ApplyLook(revised),
                            CueOrigin::User,
                            false,
                        ))
                    }
                    SemanticLightingAction::HoldCurrentLook => {
                        Err(PlanMutationError::FallbackPlanNotEditable)
                    }
                }
            })
            .collect::<Result<Vec<_>, PlanMutationError>>()?;
        if !changed {
            return Err(PlanMutationError::NoChange);
        }
        current
            .revised(cues)
            .map_err(PlanMutationError::InvalidPlan)
    }

    pub fn select_scene(
        &self,
        current: &LightingPlan,
        phrase_index: u16,
        scene_id: SceneId,
    ) -> Result<LightingPlan, PlanMutationError> {
        ensure_ready(current)?;
        let selected_scene = self
            .configuration
            .scenes
            .iter()
            .find(|candidate| candidate.id == scene_id)
            .ok_or(PlanMutationError::UnknownScene(scene_id))?;
        let target = current
            .cues()
            .get(usize::from(phrase_index))
            .ok_or(PlanMutationError::UnknownPhrase(phrase_index))?;
        let expected_category = cue_category(target)?;
        if selected_scene.category != expected_category {
            return Err(PlanMutationError::SceneCategoryMismatch {
                expected: expected_category,
                actual: selected_scene.category,
            });
        }
        let SemanticLightingAction::ApplyLook(current_look) = target.action() else {
            return Err(PlanMutationError::FallbackPlanNotEditable);
        };
        if current_look.scene_id() == selected_scene.id {
            return Err(PlanMutationError::NoChange);
        }
        let look = LightingLook::try_new(
            current_look.theme_id(),
            current_look.theme_name().to_owned(),
            selected_scene.id,
            selected_scene.name.to_owned(),
            selected_scene.category,
            selected_scene.loop_selection,
        )?;
        let mut cues = current.cues().to_vec();
        cues[usize::from(phrase_index)] = target.revised(
            SemanticLightingAction::ApplyLook(look),
            CueOrigin::User,
            target.locked(),
        );
        current
            .revised(cues)
            .map_err(PlanMutationError::InvalidPlan)
    }

    pub fn set_cue_lock(
        &self,
        current: &LightingPlan,
        phrase_index: u16,
        locked: bool,
    ) -> Result<LightingPlan, PlanMutationError> {
        ensure_ready(current)?;
        let target = current
            .cues()
            .get(usize::from(phrase_index))
            .ok_or(PlanMutationError::UnknownPhrase(phrase_index))?;
        if target.locked() == locked {
            return Err(PlanMutationError::NoChange);
        }
        let mut cues = current.cues().to_vec();
        cues[usize::from(phrase_index)] =
            target.revised(target.action().clone(), CueOrigin::User, locked);
        current
            .revised(cues)
            .map_err(PlanMutationError::InvalidPlan)
    }

    pub fn regenerate(
        &self,
        current: &LightingPlan,
        input: &PlanningInput,
    ) -> Result<LightingPlan, PlanMutationError> {
        ensure_ready(current)?;
        if current.deck_id() != input.deck_id
            || current.track_load_id() != input.track_load_id
            || current.track_id() != input.track.id
        {
            return Err(PlanMutationError::TrackLoadMismatch);
        }
        let generated = self.generate(input)?;
        let cues = generated
            .cues()
            .iter()
            .map(|generated_cue| {
                let Some(locked) = current
                    .cues()
                    .get(usize::from(generated_cue.phrase_index()))
                    .filter(|cue| cue.locked())
                else {
                    return generated_cue.clone();
                };
                if locked.start_beat() == generated_cue.start_beat()
                    && locked.end_beat() == generated_cue.end_beat()
                    && cue_category(locked).ok() == cue_category(generated_cue).ok()
                {
                    locked.clone()
                } else {
                    generated_cue.clone()
                }
            })
            .collect::<Vec<_>>();
        current
            .revised(cues)
            .map_err(PlanMutationError::InvalidPlan)
    }

    fn plan_analyzed_phrases(
        &self,
        seed: u64,
        plan_id: PlanId,
        phrases: &[TrackPhrase],
    ) -> Result<Vec<LightingCue>, PlannerError> {
        let theme_index = self
            .choice_source
            .choose(seed, 0, self.configuration.themes.len())
            .ok_or(PlannerError::EmptyThemeCatalog)?;
        let theme = &self.configuration.themes[theme_index];
        phrases
            .iter()
            .map(|phrase| {
                let category = category_for(phrase.kind());
                let candidates = self
                    .configuration
                    .scenes
                    .iter()
                    .filter(|scene| scene.category == category)
                    .collect::<Vec<_>>();
                let scene_index = self
                    .choice_source
                    .choose(seed, u64::from(phrase.index()) + 1, candidates.len())
                    .ok_or(PlannerError::MissingSceneCategory(category))?;
                let scene = candidates[scene_index];
                let look = LightingLook::try_new(
                    theme.id,
                    theme.name.to_owned(),
                    scene.id,
                    scene.name.to_owned(),
                    scene.category,
                    scene.loop_selection,
                )
                .map_err(PlannerError::InvalidPlan)?;
                Ok(LightingCue::new(
                    CueId::new(nonzero(mix(
                        plan_id.value() ^ (u64::from(phrase.index()) + 1)
                    ))),
                    phrase.index(),
                    phrase.start_beat(),
                    phrase.end_beat(),
                    SemanticLightingAction::ApplyLook(look),
                    CueOrigin::Automatic,
                    CueReason::PhraseCategoryMatched {
                        phrase_kind: phrase.kind(),
                        category,
                    },
                ))
            })
            .collect()
    }
}

fn ensure_ready(plan: &LightingPlan) -> Result<(), PlanMutationError> {
    if plan.status() == PlanStatus::Ready {
        Ok(())
    } else {
        Err(PlanMutationError::FallbackPlanNotEditable)
    }
}

fn cue_category(cue: &LightingCue) -> Result<SceneCategory, PlanMutationError> {
    match cue.reason() {
        CueReason::PhraseCategoryMatched { category, .. } => Ok(category),
        CueReason::MissingPhraseAnalysis => Err(PlanMutationError::FallbackPlanNotEditable),
    }
}

fn analysis_is_complete(phrases: &[TrackPhrase], duration_beats: u32) -> bool {
    if phrases.is_empty() {
        return false;
    }
    let mut previous_end = 0;
    for (expected_index, phrase) in phrases.iter().enumerate() {
        if usize::from(phrase.index()) != expected_index
            || phrase.start_beat() != previous_end
            || phrase.end_beat() <= phrase.start_beat()
            || phrase.end_beat() > duration_beats
        {
            return false;
        }
        previous_end = phrase.end_beat();
    }
    previous_end == duration_beats
}

const fn category_for(kind: PhraseKind) -> SceneCategory {
    match kind {
        PhraseKind::Intro | PhraseKind::Outro => SceneCategory::Ambient,
        PhraseKind::Verse => SceneCategory::Groove,
        PhraseKind::Build => SceneCategory::Build,
        PhraseKind::Drop => SceneCategory::Impact,
        PhraseKind::Breakdown => SceneCategory::Break,
    }
}

fn stable_seed(track_id: TrackId, revision: PlanConfigurationRevision) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in track_id
        .value()
        .to_le_bytes()
        .into_iter()
        .chain(revision.value().to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

const fn mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

const fn nonzero(value: u64) -> u64 {
    if value == 0 { 1 } else { value }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlannerError {
    EmptyTrackDuration,
    EmptyThemeCatalog,
    MissingSceneCategory(SceneCategory),
    InvalidPlan(PlanValidationError),
}

impl fmt::Display for PlannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTrackDuration => formatter.write_str("track duration must be non-zero"),
            Self::EmptyThemeCatalog => formatter.write_str("planner theme catalog is empty"),
            Self::MissingSceneCategory(category) => {
                write!(formatter, "planner has no scene for category {category:?}")
            }
            Self::InvalidPlan(error) => write!(formatter, "generated plan is invalid: {error}"),
        }
    }
}

impl Error for PlannerError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanMutationError {
    UnknownTheme(ThemeId),
    UnknownScene(SceneId),
    UnknownPhrase(u16),
    SceneCategoryMismatch {
        expected: SceneCategory,
        actual: SceneCategory,
    },
    FallbackPlanNotEditable,
    TrackLoadMismatch,
    NoChange,
    InvalidPlan(PlanValidationError),
    Planner(PlannerError),
}

impl From<PlanValidationError> for PlanMutationError {
    fn from(error: PlanValidationError) -> Self {
        Self::InvalidPlan(error)
    }
}

impl From<PlannerError> for PlanMutationError {
    fn from(error: PlannerError) -> Self {
        Self::Planner(error)
    }
}

impl fmt::Display for PlanMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTheme(id) => write!(formatter, "theme {} does not exist", id.value()),
            Self::UnknownScene(id) => write!(formatter, "scene {} does not exist", id.value()),
            Self::UnknownPhrase(index) => write!(formatter, "phrase {index} does not exist"),
            Self::SceneCategoryMismatch { expected, actual } => write!(
                formatter,
                "scene category {actual:?} is incompatible with required category {expected:?}"
            ),
            Self::FallbackPlanNotEditable => {
                formatter.write_str("a fallback plan cannot be edited")
            }
            Self::TrackLoadMismatch => {
                formatter.write_str("the plan no longer belongs to the loaded track")
            }
            Self::NoChange => formatter.write_str("the requested edit does not change the plan"),
            Self::InvalidPlan(error) => write!(formatter, "the revised plan is invalid: {error}"),
            Self::Planner(error) => write!(formatter, "regeneration failed: {error}"),
        }
    }
}

impl Error for PlanMutationError {}
