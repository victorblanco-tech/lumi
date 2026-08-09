use lumi_domain::{
    CueReason, OutputEffectReason, OutputEffectResult, OutputEffectStatus, PhraseKind,
    SceneCategory, SemanticLightingAction,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordedOutput<'a> {
    command_id: u64,
    plan_id: String,
    plan_revision: u64,
    deck_id: u8,
    track_load_id: u64,
    phrase_index: u16,
    cue_id: u64,
    scheduled_at: u64,
    actual_at: u64,
    status: &'static str,
    result_reason: &'static str,
    cue_reason: RecordedCueReason,
    action: RecordedAction<'a>,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum RecordedCueReason {
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
enum RecordedAction<'a> {
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

pub fn canonical_output_transcript(
    results: &[OutputEffectResult],
) -> Result<Vec<u8>, serde_json::Error> {
    let recorded = results.iter().map(record_output).collect::<Vec<_>>();
    let mut bytes = serde_json::to_vec_pretty(&recorded)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn record_output(result: &OutputEffectResult) -> RecordedOutput<'_> {
    let request = result.request();
    RecordedOutput {
        command_id: request.command_id().value(),
        plan_id: request.plan_id().value().to_string(),
        plan_revision: request.plan_revision().value(),
        deck_id: request.deck_id().value(),
        track_load_id: request.track_load_id().value(),
        phrase_index: request.phrase_index(),
        cue_id: request.cue_id().value(),
        scheduled_at: request.scheduled_at().ticks(),
        actual_at: result.actual_at().ticks(),
        status: status_name(result.status()),
        result_reason: result_reason_name(result.reason()),
        cue_reason: record_reason(request.cue_reason()),
        action: record_action(request.action()),
    }
}

fn record_action(action: &SemanticLightingAction) -> RecordedAction<'_> {
    match action {
        SemanticLightingAction::ApplyLook(look) => RecordedAction::ApplyLook {
            theme_id: look.theme_id().value(),
            theme_name: look.theme_name(),
            scene_id: look.scene_id().value(),
            scene_name: look.scene_name(),
            category: category_name(look.category()),
            loop_bank: look.loop_selection().bank(),
            loop_slot: look.loop_selection().slot(),
        },
        SemanticLightingAction::HoldCurrentLook => RecordedAction::HoldCurrentLook,
    }
}

const fn record_reason(reason: CueReason) -> RecordedCueReason {
    match reason {
        CueReason::PhraseCategoryMatched {
            phrase_kind,
            category,
        } => RecordedCueReason::PhraseCategoryMatched {
            phrase_kind: phrase_kind_name(phrase_kind),
            category: category_name(category),
        },
        CueReason::MissingPhraseAnalysis => RecordedCueReason::MissingPhraseAnalysis,
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

const fn category_name(category: SceneCategory) -> &'static str {
    match category {
        SceneCategory::Ambient => "ambient",
        SceneCategory::Groove => "groove",
        SceneCategory::Build => "build",
        SceneCategory::Impact => "impact",
        SceneCategory::Break => "break",
    }
}

const fn status_name(status: OutputEffectStatus) -> &'static str {
    match status {
        OutputEffectStatus::Simulated => "simulated",
        OutputEffectStatus::Rejected => "rejected",
        OutputEffectStatus::Skipped => "skipped",
    }
}

const fn result_reason_name(reason: OutputEffectReason) -> &'static str {
    match reason {
        OutputEffectReason::PhraseBoundary => "phraseBoundary",
        OutputEffectReason::ProviderRejected => "providerRejected",
        OutputEffectReason::StaleExecutionContext => "staleExecutionContext",
    }
}
