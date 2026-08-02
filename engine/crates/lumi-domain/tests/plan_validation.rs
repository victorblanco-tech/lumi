use lumi_domain::{
    CueId, DeckId, LightingCue, LightingPlan, PlanId, PlanRevision, PlanValidationError,
    SemanticLightingAction, ThemeId, TrackLoadId,
};

#[test]
fn invalid_plan_input_returns_typed_errors() {
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackLoadId::new(1),
            PlanRevision::new(0),
            vec![cue(1, 0)],
        ),
        Err(PlanValidationError::ZeroRevision)
    );
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackLoadId::new(1),
            PlanRevision::initial(),
            Vec::new(),
        ),
        Err(PlanValidationError::EmptyCues)
    );
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackLoadId::new(1),
            PlanRevision::initial(),
            vec![cue(1, 0), cue(1, 1)],
        ),
        Err(PlanValidationError::DuplicateCueId(CueId::new(1)))
    );
    assert_eq!(
        LightingPlan::try_new(
            PlanId::new(1),
            DeckId::new(1),
            TrackLoadId::new(1),
            PlanRevision::initial(),
            vec![cue(1, 2), cue(2, 1)],
        ),
        Err(PlanValidationError::UnorderedPhraseIndex)
    );
}

fn cue(id: u64, phrase_index: u16) -> LightingCue {
    LightingCue::new(
        CueId::new(id),
        phrase_index,
        SemanticLightingAction::SelectTheme(ThemeId::new(1)),
    )
}
