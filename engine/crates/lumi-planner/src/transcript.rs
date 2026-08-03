use std::error::Error;
use std::fmt;

use lumi_domain::{
    CueOrigin, CueReason, LightingPlan, PhraseKind, PlanStatus, SceneCategory,
    SemanticLightingAction,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalPlan<'a> {
    plan_id: u64,
    deck_id: u8,
    track_id: u64,
    track_duration_beats: u32,
    track_load_id: u64,
    revision: u64,
    configuration_revision: u64,
    seed: u64,
    status: &'static str,
    cues: Vec<CanonicalCue<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalCue<'a> {
    cue_id: u64,
    phrase_index: u16,
    start_beat: u32,
    end_beat: u32,
    origin: &'static str,
    locked: bool,
    reason: CanonicalReason,
    action: CanonicalAction<'a>,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum CanonicalReason {
    PhraseCategoryMatched {
        phrase_kind: &'static str,
        category: &'static str,
    },
    MissingPhraseAnalysis,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum CanonicalAction<'a> {
    ApplyLook {
        theme_id: u64,
        theme_name: &'a str,
        scene_id: u64,
        scene_name: &'a str,
        category: &'static str,
        loop_bank: u8,
        loop_slot: u8,
    },
    HoldCurrentLook,
}

pub fn canonical_plan(plan: &LightingPlan) -> Result<Vec<u8>, CanonicalPlanError> {
    let value = CanonicalPlan {
        plan_id: plan.id().value(),
        deck_id: plan.deck_id().value(),
        track_id: plan.track_id().value(),
        track_duration_beats: plan.track_duration_beats(),
        track_load_id: plan.track_load_id().value(),
        revision: plan.revision().value(),
        configuration_revision: plan.configuration_revision().value(),
        seed: plan.seed(),
        status: plan_status_name(plan.status()),
        cues: plan
            .cues()
            .iter()
            .map(|cue| CanonicalCue {
                cue_id: cue.id().value(),
                phrase_index: cue.phrase_index(),
                start_beat: cue.start_beat(),
                end_beat: cue.end_beat(),
                origin: cue_origin_name(cue.origin()),
                locked: cue.locked(),
                reason: canonical_reason(cue.reason()),
                action: canonical_action(cue.action()),
            })
            .collect(),
    };
    let mut bytes = serde_json::to_vec_pretty(&value).map_err(CanonicalPlanError)?;
    bytes.push(b'\n');
    Ok(bytes)
}

const fn canonical_reason(reason: CueReason) -> CanonicalReason {
    match reason {
        CueReason::PhraseCategoryMatched {
            phrase_kind,
            category,
        } => CanonicalReason::PhraseCategoryMatched {
            phrase_kind: phrase_kind_name(phrase_kind),
            category: category_name(category),
        },
        CueReason::MissingPhraseAnalysis => CanonicalReason::MissingPhraseAnalysis,
    }
}

fn canonical_action(action: &SemanticLightingAction) -> CanonicalAction<'_> {
    match action {
        SemanticLightingAction::ApplyLook(look) => CanonicalAction::ApplyLook {
            theme_id: look.theme_id().value(),
            theme_name: look.theme_name(),
            scene_id: look.scene_id().value(),
            scene_name: look.scene_name(),
            category: category_name(look.category()),
            loop_bank: look.loop_selection().bank(),
            loop_slot: look.loop_selection().slot(),
        },
        SemanticLightingAction::HoldCurrentLook => CanonicalAction::HoldCurrentLook,
    }
}

const fn plan_status_name(status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::Ready => "ready",
        PlanStatus::Fallback => "fallback",
    }
}

const fn cue_origin_name(origin: CueOrigin) -> &'static str {
    match origin {
        CueOrigin::Automatic => "automatic",
        CueOrigin::Fallback => "fallback",
        CueOrigin::User => "user",
    }
}

const fn category_name(category: SceneCategory) -> &'static str {
    match category {
        SceneCategory::Ambient => "ambient",
        SceneCategory::Groove => "groove",
        SceneCategory::Build => "build",
        SceneCategory::Impact => "impact",
        SceneCategory::Break => "break",
    }
}

const fn phrase_kind_name(kind: PhraseKind) -> &'static str {
    match kind {
        PhraseKind::Intro => "intro",
        PhraseKind::Verse => "verse",
        PhraseKind::Build => "build",
        PhraseKind::Drop => "drop",
        PhraseKind::Breakdown => "breakdown",
        PhraseKind::Outro => "outro",
    }
}

#[derive(Debug)]
pub struct CanonicalPlanError(serde_json::Error);

impl fmt::Display for CanonicalPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "canonical plan encoding failed: {}", self.0)
    }
}

impl Error for CanonicalPlanError {}
