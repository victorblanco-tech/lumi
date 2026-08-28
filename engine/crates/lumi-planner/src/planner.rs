use std::error::Error;
use std::fmt;

use lumi_domain::{
    CueId, CueOrigin, CueReason, DeckId, LightingCue, LightingLook, LightingPlan, LoopSelection,
    PhraseKind, PlanConfigurationRevision, PlanId, PlanRevision, PlanStatus, PlanValidationError,
    SceneCategory, SceneId, SemanticLightingAction, ThemeDecision, ThemeId, ThemeSelectionReason,
    TrackColor, TrackId, TrackLoadId, TrackMetadata, TrackPhrase,
};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannerTrack {
    id: TrackId,
    duration_beats: u32,
    color: Option<TrackColor>,
    phrases: Option<Vec<TrackPhrase>>,
}

impl PlannerTrack {
    #[must_use]
    pub fn analyzed(metadata: &TrackMetadata) -> Self {
        Self {
            id: metadata.id(),
            duration_beats: metadata.duration_beats(),
            color: metadata.color(),
            phrases: Some(metadata.phrases().to_vec()),
        }
    }

    #[must_use]
    pub const fn without_analysis(id: TrackId, duration_beats: u32) -> Self {
        Self {
            id,
            duration_beats,
            color: None,
            phrases: None,
        }
    }

    #[must_use]
    pub fn with_analysis(id: TrackId, duration_beats: u32, phrases: Vec<TrackPhrase>) -> Self {
        Self {
            id,
            duration_beats,
            color: None,
            phrases: Some(phrases),
        }
    }

    #[must_use]
    pub fn with_analysis_and_color(
        id: TrackId,
        duration_beats: u32,
        color: TrackColor,
        phrases: Vec<TrackPhrase>,
    ) -> Self {
        Self {
            id,
            duration_beats,
            color: Some(color),
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
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeColorRuleMode {
    Force,
    Prefer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeightedThemeCandidate {
    pub theme_id: ThemeId,
    pub weight: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeColorRule {
    pub color: TrackColor,
    pub mode: ThemeColorRuleMode,
    pub candidates: Vec<WeightedThemeCandidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeRuleColorBehavior {
    Neutral,
    Prefer,
    Only,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeSelectionRule {
    pub theme_id: ThemeId,
    pub enabled: bool,
    pub weight: u16,
    pub color_behavior: ThemeRuleColorBehavior,
    pub colors: Vec<TrackColor>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeSelectionContext {
    recent_theme_ids: Vec<ThemeId>,
}

impl ThemeSelectionContext {
    #[must_use]
    pub fn new(recent_theme_ids: Vec<ThemeId>) -> Self {
        Self {
            recent_theme_ids: recent_theme_ids
                .into_iter()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
        }
    }

    #[must_use]
    pub fn recent_theme_ids(&self) -> &[ThemeId] {
        &self.recent_theme_ids
    }
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
    global_theme_lock: Option<ThemeId>,
    color_rules: Vec<ThemeColorRule>,
    theme_selection_rules: Vec<ThemeSelectionRule>,
    default_theme_id: ThemeId,
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
                    name: "Electric Bloom".to_owned(),
                },
                ThemeDefinition {
                    id: ThemeId::new(2),
                    name: "Deep Ocean".to_owned(),
                },
                ThemeDefinition {
                    id: ThemeId::new(3),
                    name: "Solar Flare".to_owned(),
                },
                ThemeDefinition {
                    id: ThemeId::new(4),
                    name: "Ultraviolet".to_owned(),
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
            global_theme_lock: None,
            color_rules: vec![
                ThemeColorRule {
                    color: TrackColor::new(187, 72, 126),
                    mode: ThemeColorRuleMode::Force,
                    candidates: vec![WeightedThemeCandidate {
                        theme_id: ThemeId::new(4),
                        weight: 1,
                    }],
                },
                ThemeColorRule {
                    color: TrackColor::new(72, 112, 205),
                    mode: ThemeColorRuleMode::Prefer,
                    candidates: vec![WeightedThemeCandidate {
                        theme_id: ThemeId::new(2),
                        weight: 1,
                    }],
                },
            ],
            theme_selection_rules: Vec::new(),
            default_theme_id: ThemeId::new(1),
        }
    }

    #[must_use]
    pub const fn revision(&self) -> PlanConfigurationRevision {
        self.revision
    }

    #[must_use]
    pub fn with_global_theme_lock(mut self, theme_id: Option<ThemeId>) -> Self {
        self.global_theme_lock = theme_id;
        self
    }

    #[must_use]
    pub fn with_color_rules(mut self, rules: Vec<ThemeColorRule>) -> Self {
        self.color_rules = rules;
        self
    }

    #[must_use]
    pub fn with_theme_selection_rules(mut self, rules: Vec<ThemeSelectionRule>) -> Self {
        self.theme_selection_rules = rules;
        self
    }

    #[must_use]
    pub fn with_default_theme(mut self, theme_id: ThemeId) -> Self {
        self.default_theme_id = theme_id;
        self
    }

    /// Replaces the user-visible Theme/Bank catalog while preserving the
    /// provider-neutral planning rules and stable Theme identities.
    #[must_use]
    pub fn with_themes(
        mut self,
        revision: PlanConfigurationRevision,
        themes: Vec<ThemeOption>,
    ) -> Self {
        self.revision = revision;
        let theme_ids = themes.iter().map(|theme| theme.id).collect::<Vec<_>>();
        self.themes = themes
            .into_iter()
            .map(|theme| ThemeDefinition {
                id: theme.id,
                name: theme.name,
            })
            .collect();
        for rule in &mut self.color_rules {
            rule.candidates
                .retain(|candidate| theme_ids.contains(&candidate.theme_id));
        }
        self.color_rules
            .retain(|rule| rule.candidates.iter().any(|candidate| candidate.weight > 0));
        self.theme_selection_rules
            .retain(|rule| theme_ids.contains(&rule.theme_id));
        if self
            .global_theme_lock
            .is_some_and(|theme_id| !theme_ids.contains(&theme_id))
        {
            self.global_theme_lock = None;
        }
        if !theme_ids.contains(&self.default_theme_id)
            && let Some(first) = theme_ids.first()
        {
            self.default_theme_id = *first;
        }
        self
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
        self.generate_with_context(input, &ThemeSelectionContext::default())
    }

    pub fn generate_with_context(
        &self,
        input: &PlanningInput,
        context: &ThemeSelectionContext,
    ) -> Result<LightingPlan, PlannerError> {
        if input.track.duration_beats == 0 {
            return Err(PlannerError::EmptyTrackDuration);
        }
        let seed = stable_seed(input.track.id, self.configuration.revision);
        let plan_id = PlanId::new(nonzero(mix(seed ^ input.track_load_id.value())));
        let (theme_decision, cues) = match input.track.phrases.as_deref() {
            Some(phrases) if analysis_is_complete(phrases, input.track.duration_beats) => {
                let decision = self.select_initial_theme(&input.track, seed, context)?;
                let theme = self
                    .theme(decision.theme_id())
                    .ok_or(PlannerError::UnknownConfiguredTheme(decision.theme_id()))?;
                (
                    Some(decision),
                    self.plan_analyzed_phrases(seed, plan_id, phrases, theme)?,
                )
            }
            _ => (
                None,
                vec![LightingCue::new(
                    CueId::new(nonzero(mix(plan_id.value()))),
                    0,
                    0,
                    input.track.duration_beats,
                    SemanticLightingAction::HoldCurrentLook,
                    CueOrigin::Fallback,
                    CueReason::MissingPhraseAnalysis,
                )],
            ),
        };
        let status = if cues.iter().any(|cue| cue.origin() == CueOrigin::Fallback) {
            PlanStatus::Fallback
        } else {
            PlanStatus::Ready
        };

        LightingPlan::try_new_with_theme_decision(
            plan_id,
            input.deck_id,
            input.track.id,
            input.track.duration_beats,
            input.track_load_id,
            PlanRevision::initial(),
            self.configuration.revision,
            seed,
            status,
            theme_decision,
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
        let changed = current.theme_decision().is_none_or(|decision| {
            decision.theme_id() != theme.id
                || decision.reason() != ThemeSelectionReason::PlanInstanceUserChoice
        });
        let cues = current
            .cues()
            .iter()
            .map(|cue| match cue.action() {
                SemanticLightingAction::ApplyLook(look) => {
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
                        cue.locked(),
                    ))
                }
                SemanticLightingAction::HoldCurrentLook => {
                    Err(PlanMutationError::FallbackPlanNotEditable)
                }
            })
            .collect::<Result<Vec<_>, PlanMutationError>>()?;
        if !changed {
            return Err(PlanMutationError::NoChange);
        }
        let decision = ThemeDecision::try_new(
            theme.id,
            theme.name.to_owned(),
            ThemeSelectionReason::PlanInstanceUserChoice,
            None,
        )?;
        current
            .revised_with_theme_decision(cues, Some(decision))
            .map_err(PlanMutationError::InvalidPlan)
    }

    pub fn select_theme_from_phrase(
        &self,
        current: &LightingPlan,
        phrase_index: u16,
        theme_id: ThemeId,
    ) -> Result<LightingPlan, PlanMutationError> {
        if phrase_index == 0 {
            return self.select_theme(current, theme_id);
        }
        ensure_ready(current)?;
        let theme = self
            .configuration
            .themes
            .iter()
            .find(|candidate| candidate.id == theme_id)
            .ok_or(PlanMutationError::UnknownTheme(theme_id))?;
        if current.cues().get(usize::from(phrase_index)).is_none() {
            return Err(PlanMutationError::UnknownPhrase(phrase_index));
        }
        let mut changed = false;
        let cues = current
            .cues()
            .iter()
            .map(|cue| {
                if cue.phrase_index() < phrase_index {
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
                            cue.locked(),
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
        let generated = if current.theme_decision().is_some_and(|decision| {
            decision.reason() == ThemeSelectionReason::PlanInstanceUserChoice
        }) {
            let decision = current
                .theme_decision()
                .ok_or(PlanMutationError::FallbackPlanNotEditable)?;
            self.retheme_generated(generated, decision.theme_id())?
        } else {
            generated
        };
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
            .revised_with_theme_decision(cues, generated.theme_decision().cloned())
            .map_err(PlanMutationError::InvalidPlan)
    }

    fn plan_analyzed_phrases(
        &self,
        seed: u64,
        plan_id: PlanId,
        phrases: &[TrackPhrase],
        theme: &ThemeDefinition,
    ) -> Result<Vec<LightingCue>, PlannerError> {
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

    fn select_initial_theme(
        &self,
        track: &PlannerTrack,
        seed: u64,
        context: &ThemeSelectionContext,
    ) -> Result<ThemeDecision, PlannerError> {
        if let Some(theme_id) = self.configuration.global_theme_lock {
            return self.decision(theme_id, ThemeSelectionReason::GlobalLock, None);
        }
        if !self.configuration.theme_selection_rules.is_empty() {
            return self.select_policy_theme(track, seed, context);
        }
        if let Some(color) = track.color {
            for mode in [ThemeColorRuleMode::Force, ThemeColorRuleMode::Prefer] {
                if let Some(rule) = self
                    .configuration
                    .color_rules
                    .iter()
                    .find(|rule| rule.color == color && rule.mode == mode)
                {
                    let candidates = if mode == ThemeColorRuleMode::Prefer {
                        without_recent(&rule.candidates, context.recent_theme_ids())
                    } else {
                        rule.candidates.clone()
                    };
                    let candidates = if candidates.is_empty() {
                        rule.candidates.clone()
                    } else {
                        candidates
                    };
                    let theme_id =
                        self.weighted_choice(seed, u64::from(color.rgb_u32()), &candidates)?;
                    let reason = if mode == ThemeColorRuleMode::Force {
                        ThemeSelectionReason::ColorForce
                    } else {
                        ThemeSelectionReason::ColorPrefer
                    };
                    return self.decision(theme_id, reason, Some(color.rgb_u32()));
                }
            }
        }
        if !context.recent_theme_ids().is_empty() {
            let recent = context.recent_theme_ids();
            let candidates = self
                .configuration
                .themes
                .iter()
                .filter(|theme| !recent.contains(&theme.id))
                .collect::<Vec<_>>();
            if let Some(index) = self
                .choice_source
                .choose(seed, 0x0054_4845_4d45, candidates.len())
            {
                return self.decision(candidates[index].id, ThemeSelectionReason::Rotation, None);
            }
        }
        self.decision(
            self.configuration.default_theme_id,
            ThemeSelectionReason::DefaultTheme,
            None,
        )
    }

    fn select_policy_theme(
        &self,
        track: &PlannerTrack,
        seed: u64,
        context: &ThemeSelectionContext,
    ) -> Result<ThemeDecision, PlannerError> {
        let matches_color = |rule: &ThemeSelectionRule| {
            track
                .color
                .is_some_and(|color| rule.colors.contains(&color))
        };
        let matching_only = self
            .configuration
            .theme_selection_rules
            .iter()
            .filter(|rule| {
                rule.enabled
                    && rule.color_behavior == ThemeRuleColorBehavior::Only
                    && matches_color(rule)
            })
            .collect::<Vec<_>>();
        let uses_color_only = !matching_only.is_empty();
        let eligible = if uses_color_only {
            matching_only
        } else {
            self.configuration
                .theme_selection_rules
                .iter()
                .filter(|rule| rule.enabled && rule.color_behavior != ThemeRuleColorBehavior::Only)
                .collect::<Vec<_>>()
        };
        let weighted = eligible
            .iter()
            .map(|rule| WeightedThemeCandidate {
                theme_id: rule.theme_id,
                weight: if rule.color_behavior == ThemeRuleColorBehavior::Prefer
                    && matches_color(rule)
                {
                    rule.weight.saturating_mul(2)
                } else {
                    rule.weight
                }
                .max(1),
            })
            .collect::<Vec<_>>();
        if weighted.is_empty() {
            return Err(PlannerError::NoEligibleTheme);
        }
        let matching_prefer = eligible.iter().any(|rule| {
            rule.color_behavior == ThemeRuleColorBehavior::Prefer && matches_color(rule)
        });
        if !uses_color_only
            && !matching_prefer
            && context.recent_theme_ids().is_empty()
            && weighted
                .iter()
                .any(|candidate| candidate.theme_id == self.configuration.default_theme_id)
        {
            return self.decision(
                self.configuration.default_theme_id,
                ThemeSelectionReason::DefaultTheme,
                None,
            );
        }
        let without_recent = without_recent(&weighted, context.recent_theme_ids());
        let candidates = if without_recent.is_empty() {
            &weighted
        } else {
            &without_recent
        };
        let reason = if uses_color_only {
            ThemeSelectionReason::ColorForce
        } else if matching_prefer {
            ThemeSelectionReason::ColorPrefer
        } else {
            ThemeSelectionReason::Rotation
        };
        let decision = track.color.map_or(0x0054_4845_4d45, |color| {
            u64::from(color.rgb_u32()) ^ 0x0054_4845_4d45
        });
        let selected = self.weighted_choice(seed, decision, candidates)?;
        self.decision(
            selected,
            reason,
            (uses_color_only || matching_prefer)
                .then(|| track.color.map(TrackColor::rgb_u32))
                .flatten(),
        )
    }

    fn weighted_choice(
        &self,
        seed: u64,
        decision: u64,
        candidates: &[WeightedThemeCandidate],
    ) -> Result<ThemeId, PlannerError> {
        let total = candidates
            .iter()
            .try_fold(0_usize, |total, candidate| {
                total.checked_add(usize::from(candidate.weight))
            })
            .ok_or(PlannerError::InvalidThemeRule)?;
        let selected = self
            .choice_source
            .choose(seed, decision, total)
            .ok_or(PlannerError::InvalidThemeRule)?;
        let mut cursor = selected;
        for candidate in candidates {
            let weight = usize::from(candidate.weight);
            if weight == 0 {
                continue;
            }
            if cursor < weight {
                return Ok(candidate.theme_id);
            }
            cursor -= weight;
        }
        Err(PlannerError::InvalidThemeRule)
    }

    fn decision(
        &self,
        theme_id: ThemeId,
        reason: ThemeSelectionReason,
        matched_color: Option<u32>,
    ) -> Result<ThemeDecision, PlannerError> {
        let theme = self
            .theme(theme_id)
            .ok_or(PlannerError::UnknownConfiguredTheme(theme_id))?;
        ThemeDecision::try_new(theme.id, theme.name.to_owned(), reason, matched_color)
            .map_err(PlannerError::InvalidPlan)
    }

    fn theme(&self, theme_id: ThemeId) -> Option<&ThemeDefinition> {
        self.configuration
            .themes
            .iter()
            .find(|theme| theme.id == theme_id)
    }

    fn retheme_generated(
        &self,
        generated: LightingPlan,
        theme_id: ThemeId,
    ) -> Result<LightingPlan, PlanMutationError> {
        let theme = self
            .theme(theme_id)
            .ok_or(PlanMutationError::UnknownTheme(theme_id))?;
        let cues = generated
            .cues()
            .iter()
            .map(|cue| match cue.action() {
                SemanticLightingAction::ApplyLook(look) => Ok(cue.revised(
                    SemanticLightingAction::ApplyLook(LightingLook::try_new(
                        theme.id,
                        theme.name.to_owned(),
                        look.scene_id(),
                        look.scene_name().to_owned(),
                        look.category(),
                        look.loop_selection(),
                    )?),
                    cue.origin(),
                    cue.locked(),
                )),
                SemanticLightingAction::HoldCurrentLook => {
                    Err(PlanMutationError::FallbackPlanNotEditable)
                }
            })
            .collect::<Result<Vec<_>, PlanMutationError>>()?;
        let decision = ThemeDecision::try_new(
            theme.id,
            theme.name.to_owned(),
            ThemeSelectionReason::PlanInstanceUserChoice,
            None,
        )?;
        LightingPlan::try_new_with_theme_decision(
            generated.id(),
            generated.deck_id(),
            generated.track_id(),
            generated.track_duration_beats(),
            generated.track_load_id(),
            generated.revision(),
            generated.configuration_revision(),
            generated.seed(),
            generated.status(),
            Some(decision),
            cues,
        )
        .map_err(PlanMutationError::InvalidPlan)
    }
}

fn without_recent(
    candidates: &[WeightedThemeCandidate],
    recent: &[ThemeId],
) -> Vec<WeightedThemeCandidate> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| !recent.contains(&candidate.theme_id))
        .collect()
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
        CueReason::MissingAutoloopMapping => Err(PlanMutationError::FallbackPlanNotEditable),
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
    InvalidThemeRule,
    NoEligibleTheme,
    UnknownConfiguredTheme(ThemeId),
    MissingSceneCategory(SceneCategory),
    InvalidPlan(PlanValidationError),
}

impl fmt::Display for PlannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTrackDuration => formatter.write_str("track duration must be non-zero"),
            Self::EmptyThemeCatalog => formatter.write_str("planner theme catalog is empty"),
            Self::InvalidThemeRule => {
                formatter.write_str("a Theme rule has no positive weighted candidate")
            }
            Self::NoEligibleTheme => formatter
                .write_str("no enabled Theme is eligible for this track's Track Color rules"),
            Self::UnknownConfiguredTheme(theme_id) => write!(
                formatter,
                "Theme {} is referenced by configuration but not defined",
                theme_id.value()
            ),
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
