use lumi_domain::{
    CueId, CueOrigin, CueReason, DeckId, LightingCue, LightingPlan, PhraseKind,
    PlanConfigurationRevision, PlanId, PlanRevision, PlanStatus, PlanValidationError,
    SceneCategory, SemanticLightingAction, TrackId, TrackLoadId,
};

#[test]
fn invalid_plan_input_returns_typed_errors() {
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackId::new(1),
            32,
            TrackLoadId::new(1),
            PlanRevision::new(0),
            PlanConfigurationRevision::new(1),
            1,
            PlanStatus::Ready,
            vec![cue(1, 0)],
        ),
        Err(PlanValidationError::ZeroRevision)
    );
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackId::new(1),
            32,
            TrackLoadId::new(1),
            PlanRevision::initial(),
            PlanConfigurationRevision::new(1),
            1,
            PlanStatus::Ready,
            Vec::new(),
        ),
        Err(PlanValidationError::EmptyCues)
    );
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackId::new(1),
            64,
            TrackLoadId::new(1),
            PlanRevision::initial(),
            PlanConfigurationRevision::new(1),
            1,
            PlanStatus::Ready,
            vec![cue(1, 0), cue(1, 1)],
        ),
        Err(PlanValidationError::DuplicateCueId(CueId::new(1)))
    );
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackId::new(1),
            64,
            TrackLoadId::new(1),
            PlanRevision::initial(),
            PlanConfigurationRevision::new(1),
            1,
            PlanStatus::Ready,
            vec![cue(1, 2), cue(2, 1)],
        ),
        Err(PlanValidationError::UnorderedPhraseIndex)
    );
}

fn cue(id: u64, phrase_index: u16) -> LightingCue {
    LightingCue::new(
        CueId::new(id),
        phrase_index,
        u32::from(phrase_index) * 32,
        (u32::from(phrase_index) + 1) * 32,
        SemanticLightingAction::HoldCurrentLook,
        CueOrigin::Automatic,
        CueReason::PhraseCategoryMatched {
            phrase_kind: PhraseKind::Intro,
            category: SceneCategory::Ambient,
        },
    )
}
