use std::time::{Duration, Instant};

use lumi_domain::{
    CueOrigin, CueReason, DeckId, PhraseKind, PlanConfigurationRevision, PlanRevision, PlanStatus,
    SceneId, SemanticLightingAction, ThemeId, ThemeSelectionReason, TrackColor, TrackId,
    TrackLoadId, TrackPhrase,
};
use lumi_planner::{
    ChoiceSource, DeterministicPlanner, PlannerTrack, PlanningConfiguration, PlanningInput,
    ThemeColorRule, ThemeColorRuleMode, ThemeOption, ThemeRuleColorBehavior, ThemeSelectionContext,
    ThemeSelectionRule, WeightedThemeCandidate, canonical_plan,
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

    assert_eq!(look.theme_name(), "Electric Bloom");
    assert_eq!(look.scene_name(), "Soft Motion");
}

#[test]
fn configured_bank_names_are_used_by_options_decisions_and_cues() {
    let themes = (1_u64..=4)
        .map(|id| ThemeOption {
            id: ThemeId::new(id),
            name: format!("User Bank {id}"),
        })
        .collect();
    let planner = DeterministicPlanner::new(
        PlanningConfiguration::epic_one().with_themes(PlanConfigurationRevision::new(184), themes),
        FirstChoice,
    );

    assert_eq!(planner.options().themes[0].name, "User Bank 1");
    let plan = generate(&planner, &demo_input());
    assert_eq!(
        plan.theme_decision().map(|decision| decision.theme_name()),
        Some("User Bank 1")
    );
    let SemanticLightingAction::ApplyLook(look) = plan.cues()[0].action() else {
        panic!("an analyzed phrase must apply a look");
    };
    assert_eq!(look.theme_name(), "User Bank 1");
}

#[test]
fn unavailable_theme_rules_are_removed_from_an_executable_subset() {
    let planner = DeterministicPlanner::new(
        PlanningConfiguration::epic_one().with_themes(
            PlanConfigurationRevision::new(185),
            vec![ThemeOption {
                id: ThemeId::new(1),
                name: "Only Mapped Bank".to_owned(),
            }],
        ),
        FirstChoice,
    );
    let plan = generate(&planner, &colored_input(TrackColor::new(187, 72, 126)));

    assert_eq!(
        plan.theme_decision().map(|decision| decision.theme_id()),
        Some(ThemeId::new(1))
    );
    assert_eq!(
        plan.theme_decision().map(|decision| decision.theme_name()),
        Some("Only Mapped Bank")
    );
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
fn theme_override_matches_the_reviewed_golden_plan_diff() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());
    let overridden = mutate(planner.select_theme(&original, ThemeId::new(4)));
    let actual = encode(&overridden);
    let expected =
        include_bytes!("../../../../fixtures/demo-session-v1/next-plan-theme-override.json");
    assert_eq!(actual, expected);
}

#[test]
fn theme_override_from_a_future_phrase_preserves_the_earlier_live_cues() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());
    let revised = mutate(planner.select_theme_from_phrase(&original, 2, ThemeId::new(4)));

    assert_eq!(revised.revision(), PlanRevision::new(2));
    assert_eq!(&revised.cues()[..2], &original.cues()[..2]);
    for cue in &revised.cues()[2..] {
        let SemanticLightingAction::ApplyLook(look) = cue.action() else {
            panic!("future cue must remain a concrete look");
        };
        assert_eq!(look.theme_id(), ThemeId::new(4));
        assert_eq!(cue.origin(), CueOrigin::User);
    }
    assert_eq!(
        revised.theme_decision().map(|decision| decision.theme_id()),
        original
            .theme_decision()
            .map(|decision| decision.theme_id())
    );
}

#[test]
fn theme_override_from_the_first_phrase_updates_the_plan_decision() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());
    let revised = mutate(planner.select_theme_from_phrase(&original, 0, ThemeId::new(4)));

    assert!(revised.cues().iter().all(|cue| {
        matches!(
            cue.action(),
            SemanticLightingAction::ApplyLook(look) if look.theme_id() == ThemeId::new(4)
        )
    }));
    assert_eq!(
        revised.theme_decision().map(|decision| decision.theme_id()),
        Some(ThemeId::new(4))
    );
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

#[test]
fn theme_precedence_is_global_then_user_then_force_prefer_rotation_and_default() {
    let color = TrackColor::new(72, 112, 205);
    let rules = vec![ThemeColorRule {
        color,
        mode: ThemeColorRuleMode::Force,
        candidates: vec![WeightedThemeCandidate {
            theme_id: ThemeId::new(4),
            weight: 1,
        }],
    }];
    let locked = DeterministicPlanner::new(
        PlanningConfiguration::epic_one()
            .with_color_rules(rules.clone())
            .with_global_theme_lock(Some(ThemeId::new(3))),
        StableFirstChoice,
    );
    let input = colored_input(color);
    let locked_plan = generate(&locked, &input);
    let Some(locked_decision) = locked_plan.theme_decision() else {
        panic!("ready plan must contain a Theme decision");
    };
    assert_eq!(locked_decision.theme_id(), ThemeId::new(3));
    assert_eq!(locked_decision.reason(), ThemeSelectionReason::GlobalLock);

    let forced = DeterministicPlanner::new(
        PlanningConfiguration::epic_one().with_color_rules(rules),
        StableFirstChoice,
    );
    let forced_plan = generate(&forced, &input);
    assert_eq!(
        forced_plan.theme_decision().map(|value| value.reason()),
        Some(ThemeSelectionReason::ColorForce)
    );
    let user_plan = mutate(forced.select_theme(&forced_plan, ThemeId::new(2)));
    let Some(user_decision) = user_plan.theme_decision() else {
        panic!("user override must contain a Theme decision");
    };
    assert_eq!(user_decision.theme_id(), ThemeId::new(2));
    assert_eq!(
        user_decision.reason(),
        ThemeSelectionReason::PlanInstanceUserChoice
    );

    let default_plan = generate(&DeterministicPlanner::epic_one(), &demo_input());
    assert_eq!(
        default_plan.theme_decision().map(|value| value.reason()),
        Some(ThemeSelectionReason::DefaultTheme)
    );
    let rotated = generate_with_context(
        &DeterministicPlanner::epic_one(),
        &demo_input(),
        &ThemeSelectionContext::new(vec![ThemeId::new(1)]),
    );
    assert_eq!(
        rotated.theme_decision().map(|value| value.reason()),
        Some(ThemeSelectionReason::Rotation)
    );
    assert_ne!(
        rotated.theme_decision().map(|value| value.theme_id()),
        Some(ThemeId::new(1))
    );
}

#[test]
fn weighted_preference_and_no_repeat_are_deterministic() {
    let color = TrackColor::new(35, 168, 190);
    let planner = DeterministicPlanner::new(
        PlanningConfiguration::epic_one().with_color_rules(vec![ThemeColorRule {
            color,
            mode: ThemeColorRuleMode::Prefer,
            candidates: vec![
                WeightedThemeCandidate {
                    theme_id: ThemeId::new(2),
                    weight: 3,
                },
                WeightedThemeCandidate {
                    theme_id: ThemeId::new(3),
                    weight: 1,
                },
            ],
        }]),
        StableFirstChoice,
    );
    let input = colored_input(color);
    let context = ThemeSelectionContext::new(vec![ThemeId::new(2)]);
    let first = generate_with_context(&planner, &input, &context);
    let second = generate_with_context(&planner, &input, &context);
    assert_eq!(first, second);
    let Some(decision) = first.theme_decision() else {
        panic!("preferred plan must contain a Theme decision");
    };
    assert_eq!(decision.theme_id(), ThemeId::new(3));
    assert_eq!(decision.reason(), ThemeSelectionReason::ColorPrefer);
    assert_eq!(decision.matched_color(), Some(color.rgb_u32()));
}

#[test]
fn theme_strategy_keeps_color_only_themes_out_of_unmatched_tracks() {
    let pink = TrackColor::new(255, 0, 160);
    let rules = vec![
        ThemeSelectionRule {
            theme_id: ThemeId::new(1),
            enabled: true,
            weight: 2,
            color_behavior: ThemeRuleColorBehavior::Neutral,
            colors: vec![],
        },
        ThemeSelectionRule {
            theme_id: ThemeId::new(2),
            enabled: true,
            weight: 2,
            color_behavior: ThemeRuleColorBehavior::Only,
            colors: vec![pink],
        },
    ];
    let planner = DeterministicPlanner::new(
        PlanningConfiguration::epic_one().with_theme_selection_rules(rules),
        StableFirstChoice,
    );

    let matching = generate(&planner, &colored_input(pink));
    assert_eq!(
        matching.theme_decision().map(|value| value.theme_id()),
        Some(ThemeId::new(2))
    );
    assert_eq!(
        matching.theme_decision().map(|value| value.reason()),
        Some(ThemeSelectionReason::ColorForce)
    );

    let unmatched = generate(&planner, &colored_input(TrackColor::new(0, 120, 255)));
    assert_eq!(
        unmatched.theme_decision().map(|value| value.theme_id()),
        Some(ThemeId::new(1))
    );
}

#[test]
fn theme_strategy_applies_the_complete_configured_cooldown_window() {
    let rules = (1_u64..=4)
        .map(|id| ThemeSelectionRule {
            theme_id: ThemeId::new(id),
            enabled: true,
            weight: 2,
            color_behavior: ThemeRuleColorBehavior::Neutral,
            colors: vec![],
        })
        .collect();
    let planner = DeterministicPlanner::new(
        PlanningConfiguration::epic_one().with_theme_selection_rules(rules),
        StableFirstChoice,
    );
    let context =
        ThemeSelectionContext::new(vec![ThemeId::new(1), ThemeId::new(2), ThemeId::new(3)]);

    let plan = generate_with_context(&planner, &demo_input(), &context);
    assert_eq!(
        plan.theme_decision().map(|value| value.theme_id()),
        Some(ThemeId::new(4))
    );
    assert!(plan.cues().iter().all(|cue| match cue.action() {
        SemanticLightingAction::ApplyLook(look) => look.theme_id() == ThemeId::new(4),
        SemanticLightingAction::HoldCurrentLook => false,
    }));
}

#[test]
fn plan_instance_theme_switch_rethemes_locked_and_unlocked_cues_in_one_revision() {
    let planner = DeterministicPlanner::epic_one();
    let original = generate(&planner, &demo_input());
    let locked = mutate(planner.set_cue_lock(&original, 1, true));
    let switched = mutate(planner.select_theme(&locked, ThemeId::new(4)));

    assert_eq!(switched.revision(), PlanRevision::new(3));
    assert!(switched.cues()[1].locked());
    assert!(switched.cues().iter().all(|cue| match cue.action() {
        SemanticLightingAction::ApplyLook(look) => look.theme_id() == ThemeId::new(4),
        SemanticLightingAction::HoldCurrentLook => false,
    }));
    assert_eq!(
        switched.theme_decision().map(|value| value.reason()),
        Some(ThemeSelectionReason::PlanInstanceUserChoice)
    );
}

#[derive(Clone, Copy)]
struct FirstChoice;

impl ChoiceSource for FirstChoice {
    fn choose(&self, _seed: u64, _decision: u64, candidate_count: usize) -> Option<usize> {
        (candidate_count > 0).then_some(0)
    }
}

#[derive(Clone, Copy)]
struct StableFirstChoice;

impl ChoiceSource for StableFirstChoice {
    fn choose(&self, _seed: u64, _decision: u64, candidate_count: usize) -> Option<usize> {
        (candidate_count > 0).then_some(0)
    }
}

fn colored_input(color: TrackColor) -> PlanningInput {
    let base = demo_input();
    PlanningInput {
        deck_id: base.deck_id,
        track_load_id: base.track_load_id,
        track: PlannerTrack::with_analysis_and_color(
            TrackId::new(202),
            128,
            color,
            vec![
                TrackPhrase::new(0, 0, 32, PhraseKind::Intro),
                TrackPhrase::new(1, 32, 64, PhraseKind::Breakdown),
                TrackPhrase::new(2, 64, 96, PhraseKind::Build),
                TrackPhrase::new(3, 96, 128, PhraseKind::Drop),
            ],
        ),
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

fn generate_with_context<C: ChoiceSource>(
    planner: &DeterministicPlanner<C>,
    input: &PlanningInput,
    context: &ThemeSelectionContext,
) -> lumi_domain::LightingPlan {
    match planner.generate_with_context(input, context) {
        Ok(plan) => plan,
        Err(error) => panic!("test plan must generate with context: {error}"),
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
