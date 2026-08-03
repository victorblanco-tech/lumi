use std::time::{Duration, Instant};

use lumi_domain::{
    CueOrigin, CueReason, DeckId, PhraseKind, PlanRevision, PlanStatus, SceneId,
    SemanticLightingAction, ThemeId, TrackId, TrackLoadId, TrackPhrase,
};
use lumi_planner::{
    ChoiceSource, DeterministicPlanner, PlannerTrack, PlanningConfiguration, PlanningInput,
    canonical_plan,
};

#[test]
fn same_input_and_configuration_produce_the_same_canonical_plan() {
    let planner = DeterministicPlanner::epic_one();
    let first = generate(&planner, &demo_input());
    let second = generate(&planner, &demo_input());

    assert_eq!(first, second);
    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn every_analyzed_phrase_has_one_explainable_automatic_cue() {
    let plan = generate(&DeterministicPlanner::epic_one(), &demo_input());

    assert_eq!(plan.status(), PlanStatus::Ready);
    assert_eq!(plan.cues().len(), 4);
    for (expected_index, cue) in (0_u16..4).zip(plan.cues()) {
        assert_eq!(cue.phrase_index(), expected_index);
        assert_eq!(cue.origin(), CueOrigin::Automatic);
        assert!(matches!(
            cue.reason(),
            CueReason::PhraseCategoryMatched { .. }
        ));
        assert!(matches!(cue.action(), SemanticLightingAction::ApplyLook(_)));
    }
}

#[test]
fn missing_or_partial_analysis_produces_a_safe_visible_fallback() {
    let planner = DeterministicPlanner::epic_one();
    for track in [
        PlannerTrack::without_analysis(TrackId::new(202), 128),
        PlannerTrack::with_analysis(
            TrackId::new(202),
            128,
            vec![TrackPhrase::new(0, 0, 32, PhraseKind::Intro)],
        ),
    ] {
        let plan = generate(
            &planner,
            &PlanningInput {
                deck_id: DeckId::new(2),
                track_load_id: TrackLoadId::new(2001),
                track,
            },
        );
        assert_eq!(plan.status(), PlanStatus::Fallback);
        assert_eq!(plan.cues().len(), 1);
        assert_eq!(plan.cues()[0].origin(), CueOrigin::Fallback);
        assert_eq!(plan.cues()[0].reason(), CueReason::MissingPhraseAnalysis);
        assert!(matches!(
            plan.cues()[0].action(),
            SemanticLightingAction::HoldCurrentLook
        ));
    }
}

#[test]
fn choice_source_is_injected_and_does_not_use_wall_clock_time() {
    let planner = DeterministicPlanner::new(PlanningConfiguration::epic_one(), FirstChoice);
    let plan = generate(&planner, &demo_input());
    let SemanticLightingAction::ApplyLook(look) = plan.cues()[0].action() else {
        panic!("an analyzed phrase must apply a look");
    };

    assert_eq!(look.theme_name(), "Midnight Drive");
    assert_eq!(look.scene_name(), "Soft Motion");
}

#[test]
fn two_hundred_phrase_plan_completes_within_epic_one_budget() {
    let phrases = (0_u16..200)
        .map(|index| {
            let start = u32::from(index) * 32;
            TrackPhrase::new(index, start, start + 32, phrase_kind(index))
        })
        .collect();
    let input = PlanningInput {
        deck_id: DeckId::new(2),
        track_load_id: TrackLoadId::new(9_001),
        track: PlannerTrack::with_analysis(TrackId::new(900), 6_400, phrases),
    };
    let planner = DeterministicPlanner::epic_one();
    let started = Instant::now();
    let plan = generate(&planner, &input);
    let elapsed = started.elapsed();

    assert_eq!(plan.cues().len(), 200);
    assert!(
        elapsed < Duration::from_millis(50),
        "200-phrase planning exceeded 50 ms: {elapsed:?}"
    );
}

#[test]
fn demo_next_track_matches_the_reviewed_golden_plan() {
    let plan = generate(&DeterministicPlanner::epic_one(), &demo_input());
    let actual = encode(&plan);
    let expected = include_bytes!("../../../../fixtures/demo-session-v1/next-plan.json");
    assert_eq!(actual, expected);
}

#[test]
fn each_accepted_edit_creates_exactly_one_revision() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());
    let themed = mutate(planner.select_theme(&original, ThemeId::new(1)));
    let scene = mutate(planner.select_scene(&themed, 1, SceneId::new(9)));
    let locked = mutate(planner.set_cue_lock(&scene, 1, true));

    assert_eq!(themed.revision(), PlanRevision::new(2));
    assert_eq!(scene.revision(), PlanRevision::new(3));
    assert_eq!(locked.revision(), PlanRevision::new(4));
    assert_eq!(locked.cues()[1].origin(), CueOrigin::User);
    assert!(locked.cues()[1].locked());
}

#[test]
fn invalid_scene_category_is_actionable_and_does_not_create_a_plan() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());

    let result = planner.select_scene(&original, 1, SceneId::new(7));

    assert!(matches!(
        result,
        Err(lumi_planner::PlanMutationError::SceneCategoryMismatch { .. })
    ));
    assert_eq!(original.revision(), PlanRevision::initial());
}

#[test]
fn regeneration_rebases_valid_locks_and_replaces_unlocked_edits() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());
    let changed = mutate(planner.select_scene(&original, 1, SceneId::new(9)));
    let locked = mutate(planner.set_cue_lock(&changed, 1, true));
    let themed = mutate(planner.select_theme(&locked, ThemeId::new(1)));
    let regenerated = mutate(planner.regenerate(&themed, &demo_input()));

    let SemanticLightingAction::ApplyLook(locked_look) = regenerated.cues()[1].action() else {
        panic!("locked cue must remain a concrete look");
    };
    let SemanticLightingAction::ApplyLook(unlocked_look) = regenerated.cues()[0].action() else {
        panic!("unlocked cue must remain a concrete look");
    };
    assert_eq!(regenerated.revision(), PlanRevision::new(5));
    assert_eq!(locked_look.scene_name(), "Deep Space");
    assert_eq!(locked_look.theme_name(), "Electric Bloom");
    assert!(regenerated.cues()[1].locked());
    assert_eq!(unlocked_look.theme_name(), "Electric Bloom");
    assert!(!regenerated.cues()[0].locked());
}

#[derive(Clone, Copy)]
struct FirstChoice;

impl ChoiceSource for FirstChoice {
    fn choose(&self, _seed: u64, _decision: u64, candidate_count: usize) -> Option<usize> {
        (candidate_count > 0).then_some(0)
    }
}

fn demo_input() -> PlanningInput {
    PlanningInput {
        deck_id: DeckId::new(2),
        track_load_id: TrackLoadId::new(2001),
        track: PlannerTrack::with_analysis(
            TrackId::new(202),
            128,
            vec![
                TrackPhrase::new(0, 0, 32, PhraseKind::Intro),
                TrackPhrase::new(1, 32, 64, PhraseKind::Breakdown),
                TrackPhrase::new(2, 64, 96, PhraseKind::Build),
                TrackPhrase::new(3, 96, 128, PhraseKind::Drop),
            ],
        ),
    }
}

fn phrase_kind(index: u16) -> PhraseKind {
    match index % 6 {
        0 => PhraseKind::Intro,
        1 => PhraseKind::Verse,
        2 => PhraseKind::Build,
        3 => PhraseKind::Drop,
        4 => PhraseKind::Breakdown,
        _ => PhraseKind::Outro,
    }
}

fn generate<C: ChoiceSource>(
    planner: &DeterministicPlanner<C>,
    input: &PlanningInput,
) -> lumi_domain::LightingPlan {
    match planner.generate(input) {
        Ok(plan) => plan,
        Err(error) => panic!("test plan must generate: {error}"),
    }
}

fn encode(plan: &lumi_domain::LightingPlan) -> Vec<u8> {
    match canonical_plan(plan) {
        Ok(bytes) => bytes,
        Err(error) => panic!("test plan must encode: {error}"),
    }
}

fn mutate(
    result: Result<lumi_domain::LightingPlan, lumi_planner::PlanMutationError>,
) -> lumi_domain::LightingPlan {
    match result {
        Ok(plan) => plan,
        Err(error) => panic!("plan mutation must succeed: {error}"),
    }
}
