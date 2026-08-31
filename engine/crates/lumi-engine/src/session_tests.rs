use super::*;
use crate::commands::PlanCommandContext;
use lumi_domain::{ClientId, CommandSequence, OperationCommand, ThemeId, UserCommandEnvelope};
use lumi_library::AutoloopTheme;
use lumi_output_dry_run::canonical_output_transcript;
use lumi_simulator::{SimulationControl, SimulationSpeed};

#[test]
fn carabiner_control_port_stays_inside_helpers_valid_range() {
    assert_eq!(
        first_available_loopback_port_in(20_000..=32_767, |candidate| candidate == 32_767),
        Some(32_767)
    );
    assert_eq!(
        first_available_loopback_port_in(std::iter::empty(), |_| true),
        None
    );
}

#[test]
fn negative_output_timing_offset_advances_and_positive_delays() {
    assert_eq!(position_with_timing_offset(1_000, -35), 1_035);
    assert_eq!(position_with_timing_offset(1_000, 35), 965);
    assert_eq!(position_with_timing_offset(10, 35), 0);
    assert_eq!(position_with_timing_offset(u64::MAX - 5, -35), u64::MAX);

    let beat_at_140 = Duration::from_micros(60_000_000_000_u64 / 140_000);
    assert_eq!(
        negative_offset_trigger_delay(1, 140_000, -20),
        Some(beat_at_140 - Duration::from_millis(20))
    );
    assert_eq!(negative_offset_trigger_delay(1, 140_000, 20), None);
    assert_eq!(negative_offset_trigger_delay(0, 140_000, -20), None);

    let beat_at_300 = Duration::from_micros(60_000_000_000_u64 / 300_000);
    assert_eq!(negative_offset_trigger_delay(1, 300_000, -250), None);
    assert_eq!(
        negative_offset_trigger_delay(2, 300_000, -250),
        Some(beat_at_300.saturating_mul(2) - Duration::from_millis(250))
    );
}

#[test]
fn non_matching_color_only_theme_is_not_executable_for_the_track() {
    let policy = LightPlanningPolicy {
        default_theme_id: Some(3),
        theme_rules: vec![
            lumi_light_plans::ThemeRule {
                theme_id: 1,
                enabled: true,
                selection_weight: 1,
                color_behavior: ColorBehavior::Only,
                color_rgb: vec![0xff00a0],
            },
            lumi_light_plans::ThemeRule {
                theme_id: 3,
                enabled: true,
                selection_weight: 1,
                color_behavior: ColorBehavior::Neutral,
                color_rgb: Vec::new(),
            },
        ],
        ..LightPlanningPolicy::default()
    };
    let eligible = policy_eligible_executable_themes(
        vec![(ThemeId::new(1), "BLUE PINK".to_owned())],
        Some(0x0078ff),
        &policy,
    );
    assert!(eligible.is_empty());

    let matching = policy_eligible_executable_themes(
        vec![(ThemeId::new(1), "BLUE PINK".to_owned())],
        Some(0xff00a0),
        &policy,
    );
    assert_eq!(matching, vec![(ThemeId::new(1), "BLUE PINK".to_owned())]);

    let absent = policy_eligible_executable_themes(
        vec![
            (ThemeId::new(1), "BLUE PINK".to_owned()),
            (ThemeId::new(3), "BLUE RED GREEN".to_owned()),
        ],
        None,
        &policy,
    );
    assert_eq!(
        absent,
        vec![(ThemeId::new(3), "BLUE RED GREEN".to_owned())],
        "an absent Track Color must exclude Only without blocking a neutral Theme"
    );
}

#[test]
fn missing_executable_theme_is_a_safe_no_plan_result() {
    let planner = planner_for_executable_themes(14, Vec::new(), &LightPlanningPolicy::default())
        .unwrap_or_else(|error| panic!("an incomplete mapping must not fail the engine: {error}"));
    assert!(planner.is_none());
}

#[test]
fn first_live_output_with_a_later_mapping_gap_keeps_snapshot_and_engine_alive() {
    let mut runtime =
        initialized_runtime_for_mode(ManualClock::new(0), DeckSourceMode::LocalPlayback)
            .unwrap_or_else(|error| panic!("local product runtime must initialize: {error}"));

    // The demo track opens with Intro and ends with Drop. Keep Intro mapped
    // on Theme 1, but remove every Theme-1 Drop address. This reproduces a
    // real sparse SoundSwitch bank: first Play is executable while a later
    // Phrase Role deliberately has no AutoLoop mapping.
    for (expected_revision, button_number) in [(1, 4), (2, 12), (3, 24), (4, 30)] {
        apply_session_command(
            &mut runtime,
            SessionCommand::MutateAutoloopCatalog {
                expected_revision,
                mutation: crate::library::AutoloopCatalogMutation::ClearButton {
                    theme_id: ThemeId::new(1),
                    button_number,
                },
            },
        );
    }
    let catalog = runtime
        .library_worker
        .autoloop_catalog()
        .unwrap_or_else(|error| panic!("sparse catalog must reload: {error}"));
    runtime.planning_worker.synchronize_themes(&catalog);
    runtime
        .planning_worker
        .synchronize_light_policy(LightPlanningPolicy {
            default_theme_id: Some(1),
            theme_rules: vec![lumi_light_plans::ThemeRule {
                theme_id: 1,
                enabled: true,
                selection_weight: 1,
                color_behavior: ColorBehavior::Neutral,
                color_rgb: Vec::new(),
            }],
            rules: vec![lumi_light_plans::AutoloopRule {
                theme_id: 1,
                role_id: "intro-outro".to_owned(),
                variant_id: "mapping-1".to_owned(),
                enabled: true,
                selection_weight: 1,
                color_behavior: ColorBehavior::Neutral,
                color_rgb: Vec::new(),
            }],
            ..LightPlanningPolicy::default()
        });

    apply_current_session_command(&mut runtime, |expected_state_revision| {
        SessionCommand::LoadLibraryTrackOnLocalDeck {
            track_id: 1,
            deck_id: lumi_domain::DeckId::new(1),
            expected_timeline_revision: 1,
            expected_state_revision,
        }
    });
    let track_load_id = runtime
        .state
        .state()
        .deck(lumi_domain::DeckId::new(1))
        .map(lumi_domain::DeckState::track_load_id)
        .unwrap_or_else(|| panic!("local deck must be loaded"));
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Arm,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_session_command(
        &mut runtime,
        SessionCommand::UpdateLocalPlaybackTransport {
            deck_id: lumi_domain::DeckId::new(1),
            track_load_id,
            position_millis: 0,
            playing: true,
        },
    );
    assert_eq!(runtime.state.state().operation(), OperationState::Live);
    assert_eq!(runtime.output_worker.provider.records().count(), 1);

    let snapshot =
        snapshot_envelope(&runtime, 1, "first-play-sparse-theme").unwrap_or_else(|error| {
            panic!("diagnostic enrichment must never terminate Live output: {error}")
        });
    assert_eq!(snapshot.payload["operationState"], "live");
    assert_eq!(
        snapshot.payload["outputEffects"].as_array().map(Vec::len),
        Some(1)
    );
    assert!(
        snapshot.payload["outputEffects"][0]["libraryResolution"]["dryRunEntry"]["id"].is_string()
    );
}

#[test]
fn policy_with_only_disabled_executable_themes_is_a_safe_no_plan_result() {
    let policy = LightPlanningPolicy {
        theme_rules: vec![lumi_light_plans::ThemeRule {
            theme_id: 1,
            enabled: false,
            selection_weight: 1,
            color_behavior: ColorBehavior::Neutral,
            color_rgb: Vec::new(),
        }],
        ..LightPlanningPolicy::default()
    };
    let planner =
        planner_for_executable_themes(15, vec![(ThemeId::new(1), "BLUE PINK".to_owned())], &policy)
            .unwrap_or_else(|error| panic!("a disabled Theme must fail closed safely: {error}"));
    assert!(planner.is_none());
}

#[test]
fn static_look_transition_is_sparse_and_never_reasserts_the_same_toggle() {
    let first = StaticLookTarget {
        modifier_id: "first".to_owned(),
        display_name: "Moving Heads Off".to_owned(),
        static_look_number: 1,
    };
    let second = StaticLookTarget {
        modifier_id: "second".to_owned(),
        display_name: "Only Lasers".to_owned(),
        static_look_number: 2,
    };
    assert_eq!(
        static_look_transition(None, Some(&first)),
        Some(StaticLookTransition::Activate(first.clone()))
    );
    assert_eq!(static_look_transition(Some(&first), Some(&first)), None);
    let same_address_from_manual_control = StaticLookTarget {
        modifier_id: "manual".to_owned(),
        display_name: "Static Look 1".to_owned(),
        static_look_number: 1,
    };
    assert_eq!(
        static_look_transition(Some(&same_address_from_manual_control), Some(&first)),
        None
    );
    assert_eq!(
        static_look_transition(Some(&first), Some(&second)),
        Some(StaticLookTransition::Activate(second.clone()))
    );
    assert_eq!(
        static_look_transition(Some(&second), None),
        Some(StaticLookTransition::Release(second))
    );
    assert_eq!(static_look_transition(None, None), None);
}

#[test]
fn pro_dj_link_clock_is_independent_from_lighting_operation_state() {
    let observation = lumi_prolink_input::ProLinkTimingObservation {
        deck_id: lumi_domain::DeckId::new(2),
        observed_at_nanos: 12_345_000,
        absolute_beat: 42,
        effective_bpm_milli: 155_250,
        beat_within_bar: 3,
        playing: true,
        generation: 7,
        discontinuity: false,
    };

    let clock = prolink_link_clock(observation);
    assert_eq!(clock.bpm_milli, 155_250);
    assert_eq!(clock.beat_within_bar, 3);
    assert_eq!(clock.deck_number, Some(2));
    assert!(clock.playing);

    let transport_jump = lumi_prolink_input::ProLinkTimingObservation {
        generation: 99,
        discontinuity: true,
        ..observation
    };
    assert_eq!(
        prolink_link_clock(transport_jump),
        clock,
        "show transport generations and Hot Cue/seek discontinuities must be invisible to Link"
    );
}

#[test]
fn pro_dj_link_stale_window_tracks_eight_beats_with_safe_bounds() {
    assert_eq!(
        prolink_timing_stale_after(Some(140_000)),
        Duration::from_micros(3_428_571)
    );
    assert_eq!(
        prolink_timing_stale_after(Some(300_000)),
        MINIMUM_PROLINK_TIMING_STALE_AFTER
    );
    assert_eq!(
        prolink_timing_stale_after(Some(20_000)),
        MAXIMUM_PROLINK_TIMING_STALE_AFTER
    );
    assert_eq!(
        prolink_timing_stale_after(None),
        MINIMUM_PROLINK_TIMING_STALE_AFTER
    );
}

#[test]
fn integration_pump_metrics_detect_starvation_without_unbounded_samples() {
    let mut metrics = IntegrationPumpMetrics::new();
    let started = Instant::now();
    metrics.record(started);
    metrics.record(started + INTEGRATION_PUMP_INTERVAL);
    metrics.record(started + INTEGRATION_PUMP_INTERVAL.saturating_mul(4));

    assert_eq!(metrics.tick_count, 3);
    assert_eq!(metrics.starvation_count, 1);
    assert_eq!(
        metrics.max_lateness_micros,
        u64::try_from(INTEGRATION_PUMP_INTERVAL.saturating_mul(2).as_micros()).unwrap_or(u64::MAX)
    );
}

#[tokio::test]
async fn command_reader_retains_partial_input_when_timing_tick_cancels_the_read() {
    let (mut client, server) = tokio::io::duplex(128);
    let mut reader = BufReader::new(server);
    let mut buffer = Vec::new();
    client
        .write_all(b"{\"partial\":")
        .await
        .unwrap_or_else(|error| panic!("partial command should write: {error}"));

    let interrupted = tokio::time::timeout(
        Duration::from_millis(5),
        read_command_line(&mut reader, &mut buffer),
    )
    .await;
    assert!(interrupted.is_err());
    assert_eq!(buffer, b"{\"partial\":");

    client
        .write_all(b"true}\n")
        .await
        .unwrap_or_else(|error| panic!("remaining command should write: {error}"));
    let line = read_command_line(&mut reader, &mut buffer)
        .await
        .unwrap_or_else(|error| panic!("command should complete: {error}"));
    assert_eq!(line.as_deref(), Some(b"{\"partial\":true}".as_slice()));
    assert!(buffer.is_empty());
}

#[test]
fn pro_dj_link_preflight_failure_is_actionable_and_not_retryable() {
    let message = "Close rekordbox before starting Pro DJ Link.";
    let result = application_error_envelope(
        1,
        "select-live-decks",
        &CommandApplicationError::ProLinkUnavailable(message.to_owned()),
    );
    let Ok(envelope) = result else {
        panic!("preflight error must serialize");
    };

    assert_eq!(
        envelope.payload.get("code"),
        Some(&json!("proDjLinkUnavailable"))
    );
    assert_eq!(envelope.payload.get("message"), Some(&json!(message)));
    assert_eq!(envelope.payload.get("retryable"), Some(&json!(false)));
}

#[test]
fn next_plan_is_ready_before_the_initial_leader_event() {
    let mut runtime = match SerializedRuntime::try_new(EVENT_QUEUE_CAPACITY) {
        Ok(runtime) => runtime,
        Err(error) => panic!("test runtime must initialize: {error}"),
    };
    if let Err(error) = submit_and_process(
        &mut runtime,
        DomainEvent::RuntimeStarted {
            at: MonotonicTime::new(0),
        },
    ) {
        panic!("test runtime must start: {error}");
    }
    let mut source = match SimulatorDeckSourceProvider::demo(ManualClock::new(0)) {
        Ok(source) => source,
        Err(error) => panic!("test simulator must initialize: {error}"),
    };
    let events = match source.drain_events() {
        Ok(events) => events,
        Err(error) => panic!("test simulator events must drain: {error}"),
    };
    let catalog = AutoloopCatalog::try_new(
        1,
        0,
        (1_u16..=4)
            .map(|number| {
                AutoloopTheme::try_new(
                    lumi_domain::ThemeId::new(u64::from(number)),
                    format!("Bank {number}"),
                    number,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("test themes must be valid: {error}")),
        Vec::new(),
        Vec::new(),
    )
    .unwrap_or_else(|error| panic!("test catalog must be valid: {error}"));
    let mut worker = PlanningWorker::new(&catalog);
    let mut output_worker = OutputWorker::new();

    for event in events {
        if matches!(
            &event,
            DomainEvent::Observation(lumi_domain::ObservationEnvelope {
                observation: DeckObservation::LeaderChanged { .. },
                ..
            })
        ) {
            assert!(runtime.state().plan(lumi_domain::DeckId::new(2)).is_some());
        }
        if let Err(error) = worker.process_source_event(
            &mut runtime,
            &mut output_worker,
            event,
            Some(source.leader_deck_id()),
        ) {
            panic!("test source event must process: {error}");
        }
    }
}

#[test]
fn simulator_snapshot_exposes_normalized_rgb_waveforms_for_both_decks() {
    let runtime = initialized_runtime()
        .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
    let snapshot = snapshot_envelope(&runtime, 1, "waveform-contract")
        .unwrap_or_else(|error| panic!("snapshot must encode: {error}"));
    let decks = snapshot.payload["decks"]
        .as_array()
        .unwrap_or_else(|| panic!("snapshot must contain decks"));

    assert_eq!(decks.len(), 2);
    for deck in decks {
        let waveform = &deck["track"]["waveformPreview"];
        assert_eq!(waveform["source"], "simulator");
        assert_eq!(waveform["style"], "rgb");
        assert_eq!(waveform["points"].as_array().map(Vec::len), Some(192));
    }
}

#[test]
fn live_execution_matches_the_canonical_dry_run_transcript() {
    let clock = ManualClock::new(0);
    let mut runtime = match initialized_runtime_with_clock(clock.clone()) {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_operation(&mut runtime, 1, OperationCommand::Arm);
    apply_operation(&mut runtime, 2, OperationCommand::Start);
    apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
    apply_simulation_control(
        &mut runtime,
        SimulationControl::SetSpeed(SimulationSpeed::SixtyFour),
    );
    assert!(clock.advance(1_000).is_some());
    if let Err(error) = runtime.deck_source.update_to_clock() {
        panic!("test simulator must advance: {error}");
    }
    if let Err(error) = process_pending_source_events(&mut runtime) {
        panic!("test source events must process: {error}");
    }

    let results = runtime
        .state
        .state()
        .output_effects()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 5);
    assert!(
        results
            .iter()
            .all(|result| result.status() == OutputEffectStatus::Simulated)
    );
    let actual = match canonical_output_transcript(&results) {
        Ok(actual) => actual,
        Err(error) => panic!("test transcript must encode: {error}"),
    };
    assert_eq!(
        actual,
        include_bytes!("../../../../fixtures/demo-session-v1/output-effects.json")
    );
}

#[test]
fn library_track_runs_from_exact_timeline_through_next_activation_and_dry_run() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    runtime
        .library_worker
        .open_editor(1)
        .unwrap_or_else(|error| panic!("fixture track must open: {error}"));
    runtime
        .library_worker
        .set_phrase_loop_strategy(
            1,
            1,
            1,
            1,
            lumi_library::PhraseLoopStrategy::FixedVariant(
                lumi_library::VariantId::try_new("mapping-2")
                    .unwrap_or_else(|error| panic!("fixture variant must be valid: {error}")),
            ),
        )
        .unwrap_or_else(|error| panic!("fixture loop strategy must save: {error}"));
    let expected_state_revision = runtime.state.state().revision();
    apply_session_command(
        &mut runtime,
        SessionCommand::LoadLibraryTrackOnLocalDeck {
            track_id: 1,
            deck_id: lumi_domain::DeckId::new(2),
            expected_timeline_revision: 2,
            expected_state_revision,
        },
    );

    let preview = snapshot_envelope(&runtime, 1, "library-preview")
        .unwrap_or_else(|error| panic!("preview must encode: {error}"));
    assert_eq!(preview.payload["decks"][1]["track"]["id"], 1);
    assert_eq!(
        preview.payload["decks"][1]["track"]["beatGrid"]["beatsPerBar"],
        4
    );
    assert!(
        preview.payload["decks"][1]["track"]["beatGrid"]["timesMillis"]
            .as_array()
            .is_some_and(|markers| !markers.is_empty() && markers[0].is_number())
    );
    assert_eq!(
        preview.payload["decks"][1]["track"]["identityFacts"]["timelineRevision"],
        2
    );
    assert_eq!(
        preview.payload["nextPlan"]["libraryTrack"]["matchStatus"],
        "exact"
    );
    assert_eq!(
        preview.payload["nextPlan"]["libraryTrack"]["providerKind"],
        "demo"
    );
    assert!(preview.payload["nextPlan"]["themeDecision"]["reason"].is_string());
    let preview_cues = preview.payload["nextPlan"]["cues"]
        .as_array()
        .unwrap_or_else(|| panic!("next plan must expose cues"));
    assert!(!preview_cues.is_empty());
    assert!(preview_cues.iter().all(|cue| {
        cue["libraryResolution"]["roleId"].is_string()
            && cue["libraryResolution"]["strategy"].is_string()
            && cue["libraryResolution"]["variantId"].is_string()
            && cue["libraryResolution"]["dryRunEntry"]["id"].is_string()
            && cue["libraryResolution"]["choices"]
                .as_array()
                .is_some_and(|choices| !choices.is_empty())
            && cue["action"]["sceneId"] == cue["libraryResolution"]["autoloopNumber"]
    }));

    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Arm,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceToNextTrack { expected_revision }
    });
    let activated_snapshot = snapshot_envelope(&runtime, 2, "library-activated")
        .unwrap_or_else(|error| panic!("activated snapshot must encode: {error}"));
    let active = runtime
        .state
        .state()
        .active_plan()
        .unwrap_or_else(|| panic!("leader change must activate the prepared plan"));
    assert_eq!(active.track_id(), lumi_domain::TrackId::new(1));
    let cue_count = active.cues().len();
    let active_track_load_id = active.track_load_id();
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetSimulationSpeed {
            expected_revision,
            speed: SimulationSpeed::SixtyFour,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks: 1_000,
        }
    });

    let results = runtime.state.state().output_effects().collect::<Vec<_>>();
    assert_eq!(results.len(), cue_count + 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| result.request().track_load_id() == active_track_load_id)
            .map(|result| result.request().phrase_index())
            .collect::<Vec<_>>(),
        (0..u16::try_from(cue_count)
            .unwrap_or_else(|error| panic!("fixture cue count must fit u16: {error}")))
            .collect::<Vec<_>>()
    );
    let completed = snapshot_envelope(&runtime, 3, "library-completed")
        .unwrap_or_else(|error| panic!("completed snapshot must encode: {error}"));
    assert!(
        completed.payload["outputEffects"]
            .as_array()
            .is_some_and(|effects| {
                effects
                    .iter()
                    .filter(|effect| effect["trackLoadId"] == active_track_load_id.value())
                    .all(|effect| {
                        effect["status"] == "simulated"
                            && effect["libraryResolution"]["dryRunEntry"]["id"].is_string()
                    })
            })
    );
    let evidence = canonical_library_simulation_evidence(
        &preview.payload,
        &activated_snapshot.payload,
        &completed.payload,
    );
    assert_eq!(
        String::from_utf8_lossy(&evidence),
        String::from_utf8_lossy(include_bytes!(
            "../../../../fixtures/demo-library-v1/simulator-e2e.json"
        ))
    );
}

#[test]
fn armed_and_paused_states_never_call_the_output_provider() {
    let clock = ManualClock::new(0);
    let mut runtime = match initialized_runtime_with_clock(clock.clone()) {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_operation(&mut runtime, 1, OperationCommand::Arm);
    apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
    assert_eq!(runtime.output_worker.provider.records().count(), 0);

    apply_operation(&mut runtime, 2, OperationCommand::Start);
    let records_after_start = runtime.output_worker.provider.records().count();
    assert_eq!(records_after_start, 1);
    apply_operation(&mut runtime, 3, OperationCommand::Pause);
    apply_simulation_control(
        &mut runtime,
        SimulationControl::SetSpeed(SimulationSpeed::SixtyFour),
    );
    assert!(clock.advance(1_000).is_some());
    if let Err(error) = runtime.deck_source.update_to_clock() {
        panic!("test simulator must advance: {error}");
    }
    if let Err(error) = process_pending_source_events(&mut runtime) {
        panic!("test source events must process: {error}");
    }
    assert_eq!(
        runtime.output_worker.provider.records().count(),
        records_after_start
    );
    assert_eq!(runtime.state.state().operation(), OperationState::Paused);
}

#[test]
fn operation_control_survives_unrelated_deck_revision_churn() {
    let mut runtime = initialized_runtime()
        .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
    let ui_revision = runtime.state.state().revision();

    // A playing Pro DJ Link deck can advance the global state revision
    // after the UI rendered Off but before its Arm command arrives.
    apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
    assert_ne!(runtime.state.state().revision(), ui_revision);
    assert_eq!(runtime.state.state().operation(), OperationState::Off);

    apply_session_command(
        &mut runtime,
        SessionCommand::SetOperationState {
            expected_revision: ui_revision,
            command: OperationCommand::Arm,
        },
    );

    assert_eq!(runtime.state.state().operation(), OperationState::Armed);
}

#[test]
fn accepted_live_plan_revisions_update_future_output_atomically() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
    let active_before = match runtime.state.state().active_plan().cloned() {
        Some(plan) => plan,
        None => panic!("leader change must freeze an active plan"),
    };
    let stored_before = match runtime
        .state
        .state()
        .plan(lumi_domain::DeckId::new(2))
        .cloned()
    {
        Some(plan) => plan,
        None => panic!("stored plan must remain available"),
    };
    let changed = match runtime
        .planning_worker
        .planner
        .select_theme(&stored_before, lumi_domain::ThemeId::new(4))
    {
        Ok(plan) => plan,
        Err(error) => panic!("preview Theme must change: {error}"),
    };
    if let Err(error) = runtime
        .planning_worker
        .accept_revised_plan(&mut runtime.state, changed)
    {
        panic!("revised preview plan must be accepted: {error}");
    }

    let active_after = match runtime.state.state().active_plan() {
        Some(plan) => plan,
        None => panic!("active plan must remain frozen"),
    };
    let stored_after = match runtime.state.state().plan(lumi_domain::DeckId::new(2)) {
        Some(plan) => plan,
        None => panic!("stored plan must remain available"),
    };
    assert_ne!(active_after, &active_before);
    assert_eq!(active_after.revision(), PlanRevision::new(2));
    assert_eq!(stored_after.revision(), PlanRevision::new(2));
    assert_eq!(
        active_after.theme_decision().map(|value| value.theme_id()),
        stored_after.theme_decision().map(|value| value.theme_id())
    );
}

#[test]
fn started_live_phrases_are_locked_but_future_phrases_remain_editable() {
    let mut runtime = initialized_runtime()
        .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
    let active = runtime
        .state
        .state()
        .active_plan()
        .cloned()
        .unwrap_or_else(|| panic!("initial leader must have an active plan"));
    let current_theme = active
        .theme_decision()
        .map(|decision| decision.theme_id())
        .unwrap_or_else(|| panic!("ready plan must have a Theme"));
    let new_theme = [
        ThemeId::new(1),
        ThemeId::new(2),
        ThemeId::new(3),
        ThemeId::new(4),
    ]
    .into_iter()
    .find(|candidate| *candidate != current_theme)
    .unwrap_or_else(|| panic!("fixture must expose an alternate Theme"));
    let context = PlanCommandContext {
        plan_id: active.id(),
        track_load_id: active.track_load_id(),
        expected_revision: active.revision(),
    };

    let started = apply_command(
        &mut runtime,
        SessionCommand::SelectThemeFromPhrase {
            context,
            phrase_index: 0,
            theme_id: new_theme,
        },
    );
    assert!(matches!(
        started,
        Err(CommandApplicationError::StartedLivePhraseNotEditable)
    ));

    apply_session_command(
        &mut runtime,
        SessionCommand::SelectThemeFromPhrase {
            context,
            phrase_index: 1,
            theme_id: new_theme,
        },
    );
    let revised = runtime
        .state
        .state()
        .active_plan()
        .unwrap_or_else(|| panic!("accepted edit must remain active"));
    assert_eq!(revised.revision(), PlanRevision::new(2));
    assert_eq!(revised.cues()[0], active.cues()[0]);
    assert_ne!(revised.cues()[1], active.cues()[1]);
}

#[test]
fn future_live_theme_change_materializes_the_selected_bank_per_phrase() {
    let mut runtime =
        initialized_runtime_for_mode(ManualClock::new(0), DeckSourceMode::LocalPlayback)
            .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
    apply_current_session_command(&mut runtime, |expected_state_revision| {
        SessionCommand::LoadLibraryTrackOnLocalDeck {
            track_id: 1,
            deck_id: lumi_domain::DeckId::new(1),
            expected_timeline_revision: 1,
            expected_state_revision,
        }
    });
    let active = runtime
        .state
        .state()
        .active_plan()
        .cloned()
        .unwrap_or_else(|| panic!("initial leader must have an active plan"));
    assert!(active.cues().len() > 1, "fixture needs a future phrase");
    let current_theme = active
        .theme_decision()
        .map(|decision| decision.theme_id())
        .unwrap_or_else(|| panic!("ready plan must have a Theme"));
    let selected_theme = if current_theme == ThemeId::new(4) {
        ThemeId::new(3)
    } else {
        ThemeId::new(4)
    };
    runtime
        .planning_worker
        .library_contexts
        .get_mut(&active.track_load_id())
        .unwrap_or_else(|| panic!("active Library track must expose its context"))
        .remap_phrase_for_test(selected_theme, 1, 32)
        .unwrap_or_else(|error| panic!("test mapping must be replaceable: {error}"));

    apply_session_command(
        &mut runtime,
        SessionCommand::SelectThemeFromPhrase {
            context: PlanCommandContext {
                plan_id: active.id(),
                track_load_id: active.track_load_id(),
                expected_revision: active.revision(),
            },
            phrase_index: 1,
            theme_id: selected_theme,
        },
    );

    let revised = runtime
        .state
        .state()
        .active_plan()
        .unwrap_or_else(|| panic!("accepted edit must remain active"));
    assert_eq!(revised.cues()[0], active.cues()[0]);
    let SemanticLightingAction::ApplyLook(look) = revised.cues()[1].action() else {
        panic!("mapped future phrase must apply a concrete look");
    };
    assert_eq!(look.theme_id(), selected_theme);
    assert_eq!(look.scene_id(), SceneId::new(32));

    let snapshot = snapshot_envelope(&runtime, 7, "future-theme-regression")
        .unwrap_or_else(|error| panic!("revised snapshot must serialize: {error}"));
    let future = &snapshot.payload["livePlan"]["cues"][1];
    assert_eq!(future["action"]["themeId"], json!(selected_theme.value()));
    assert_eq!(future["action"]["sceneId"], json!(32));
    assert_eq!(
        future["libraryResolution"]["bankNumber"],
        json!(selected_theme.value())
    );
    assert_eq!(future["libraryResolution"]["autoloopNumber"], json!(32));
}

#[test]
fn live_apply_look_maps_to_a_sound_switch_bank_and_autoloop_button() {
    let runtime = initialized_runtime()
        .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
    let cue = runtime
        .state
        .state()
        .active_plan()
        .and_then(|plan| plan.cues().first())
        .unwrap_or_else(|| panic!("initial Live plan must expose a cue"));
    let SemanticLightingAction::ApplyLook(look) = cue.action() else {
        panic!("ready Live cue must apply a look");
    };

    let target = automatic_midi_target(cue.action())
        .unwrap_or_else(|error| panic!("fixture target must fit MIDI: {error}"));
    assert_eq!(
        target,
        Some((
            u8::try_from(look.theme_id().value())
                .unwrap_or_else(|error| panic!("fixture Theme must fit: {error}")),
            u8::try_from(look.scene_id().value())
                .unwrap_or_else(|error| panic!("fixture button must fit: {error}")),
        ))
    );
}

#[test]
fn stale_execution_is_skipped_before_the_provider_call() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_operation(&mut runtime, 1, OperationCommand::Arm);
    apply_operation(&mut runtime, 2, OperationCommand::Start);
    apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
    let Some(request) = runtime
        .state
        .state()
        .output_effects()
        .next()
        .map(|result| result.request().clone())
    else {
        panic!("live leader change must create the first output request");
    };
    let records_before_stale = runtime.output_worker.provider.records().count();
    assert_eq!(records_before_stale, 2);

    apply_operation(&mut runtime, 3, OperationCommand::Pause);
    if let Err(error) = runtime.output_worker.process_effects(
        &mut runtime.state,
        vec![lumi_domain::Effect::ExecuteCue(request)],
    ) {
        panic!("stale output must be recorded safely: {error}");
    }

    assert_eq!(
        runtime.output_worker.provider.records().count(),
        records_before_stale
    );
    let Some(last) = runtime.state.state().output_effects().last() else {
        panic!("skipped output must be retained");
    };
    assert_eq!(last.status(), OutputEffectStatus::Skipped);
    assert_eq!(last.reason(), OutputEffectReason::StaleExecutionContext);
}

#[test]
fn local_playback_reasserts_a_restarted_phrase_and_activates_a_paused_seek_on_resume() {
    let mut runtime =
        initialized_runtime_for_mode(ManualClock::new(0), DeckSourceMode::LocalPlayback)
            .unwrap_or_else(|error| panic!("local product runtime must initialize: {error}"));
    apply_current_session_command(&mut runtime, |expected_state_revision| {
        SessionCommand::LoadLibraryTrackOnLocalDeck {
            track_id: 1,
            deck_id: lumi_domain::DeckId::new(1),
            expected_timeline_revision: 1,
            expected_state_revision,
        }
    });
    let track_load_id = runtime
        .state
        .state()
        .deck(lumi_domain::DeckId::new(1))
        .map(lumi_domain::DeckState::track_load_id)
        .unwrap_or_else(|| panic!("local deck must be loaded"));
    let second_phrase_beat = runtime
        .state
        .state()
        .deck(lumi_domain::DeckId::new(1))
        .and_then(|deck| deck.metadata().phrases().get(1))
        .map(|phrase| phrase.start_beat())
        .unwrap_or_else(|| panic!("fixture track must have a second phrase"));
    let second_phrase_millis = runtime
        .planning_worker
        .library_context(track_load_id)
        .and_then(|context| {
            (0..120_000_u64)
                .step_by(100)
                .find(|position| context.beat_at_millis(*position) >= second_phrase_beat)
        })
        .unwrap_or_else(|| panic!("fixture beat grid must reach its second phrase"));

    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Arm,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_session_command(
        &mut runtime,
        SessionCommand::UpdateLocalPlaybackTransport {
            deck_id: lumi_domain::DeckId::new(1),
            track_load_id,
            position_millis: 0,
            playing: true,
        },
    );
    assert_eq!(runtime.output_worker.provider.records().count(), 1);

    apply_session_command(
        &mut runtime,
        SessionCommand::SetOutputTimingOffset { millis: 20 },
    );
    assert_eq!(runtime.output_worker.timing_offset_millis(), 0);
    assert_eq!(
        runtime.output_worker.pending_timing_offset_millis(),
        Some(20)
    );

    for playing in [false, true] {
        apply_session_command(
            &mut runtime,
            SessionCommand::UpdateLocalPlaybackTransport {
                deck_id: lumi_domain::DeckId::new(1),
                track_load_id,
                position_millis: 0,
                playing,
            },
        );
    }
    assert_eq!(runtime.output_worker.provider.records().count(), 2);
    assert_eq!(runtime.output_worker.timing_offset_millis(), 0);
    assert_eq!(
        runtime.output_worker.pending_timing_offset_millis(),
        Some(20)
    );

    apply_session_command(
        &mut runtime,
        SessionCommand::UpdateLocalPlaybackTransport {
            deck_id: lumi_domain::DeckId::new(1),
            track_load_id,
            position_millis: second_phrase_millis,
            playing: false,
        },
    );
    assert_eq!(runtime.output_worker.provider.records().count(), 2);
    assert_eq!(runtime.output_worker.timing_offset_millis(), 0);
    assert_eq!(
        runtime.output_worker.pending_timing_offset_millis(),
        Some(20)
    );
    apply_session_command(
        &mut runtime,
        SessionCommand::UpdateLocalPlaybackTransport {
            deck_id: lumi_domain::DeckId::new(1),
            track_load_id,
            position_millis: second_phrase_millis,
            playing: true,
        },
    );
    assert_eq!(runtime.output_worker.provider.records().count(), 3);
    assert_eq!(runtime.output_worker.timing_offset_millis(), 20);
    assert_eq!(runtime.output_worker.pending_timing_offset_millis(), None);
}

#[test]
fn app_commands_complete_the_canonical_demo_and_reset_without_restart() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetSimulationSpeed {
            expected_revision,
            speed: SimulationSpeed::SixtyFour,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Arm,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceToNextTrack { expected_revision }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks: 1_000,
        }
    });

    assert_eq!(runtime.state.state().operation(), OperationState::Live);
    assert_eq!(runtime.output_worker.provider.records().count(), 5);
    assert!(
        runtime
            .state
            .state()
            .timeline()
            .any(|entry| entry.source() == TimelineSource::Operation)
    );
    assert!(
        runtime
            .state
            .state()
            .timeline()
            .any(|entry| entry.result() == TimelineResult::Simulated)
    );

    let reset_revision = runtime.state.state().revision();
    apply_session_command(
        &mut runtime,
        SessionCommand::ResetDemoSession {
            expected_revision: reset_revision,
        },
    );
    let canonical = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("canonical engine must initialize: {error}"),
    };
    assert_eq!(runtime.state.state(), canonical.state.state());
    assert_eq!(
        runtime.deck_source.canonical_snapshot(),
        canonical.deck_source.canonical_snapshot()
    );
    assert_eq!(runtime.output_worker.provider.records().count(), 0);
}

#[test]
fn simulation_speed_preserves_semantic_output_order() {
    let sixteen = semantic_output_order(SimulationSpeed::Sixteen, 4_000);
    let sixty_four = semantic_output_order(SimulationSpeed::SixtyFour, 1_000);
    assert_eq!(sixteen, sixty_four);
    assert_eq!(sixteen.len(), 5);
}

#[test]
fn runtime_timeline_is_bounded_to_the_latest_256_entries() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    for sequence in 1..=300 {
        if let Err(error) = submit_and_process(
            &mut runtime.state,
            DomainEvent::EffectResult(EffectResultEnvelope {
                effect_id: EffectId::new(sequence),
                worker_id: WorkerId::new(99),
                sequence: EffectSequence::new(sequence),
                completed_at: MonotonicTime::new(sequence),
                result: EffectResult::OutputGateClosed,
            }),
        ) {
            panic!("timeline event must process: {error}");
        }
    }
    let timeline = runtime.state.state().timeline().collect::<Vec<_>>();
    assert_eq!(timeline.len(), 256);
    assert_eq!(
        timeline.last().map(|entry| entry.occurred_at().ticks()),
        Some(300)
    );
    assert!(
        timeline
            .windows(2)
            .all(|entries| entries[0].sequence() < entries[1].sequence())
    );
}

#[test]
fn full_snapshot_projection_has_a_measured_bounded_baseline() {
    let runtime = initialized_runtime()
        .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
    let mut samples_micros = Vec::with_capacity(250);
    let mut live_samples_micros = Vec::with_capacity(250);
    let mut maximum_payload_bytes = 0_usize;
    let mut maximum_live_payload_bytes = 0_usize;

    for sequence in 1..=260_u64 {
        let started = Instant::now();
        let snapshot = snapshot_envelope(&runtime, sequence, "snapshot-baseline")
            .unwrap_or_else(|error| panic!("snapshot projection must succeed: {error}"));
        let encoded = serde_json::to_vec(&snapshot)
            .unwrap_or_else(|error| panic!("snapshot encoding must succeed: {error}"));
        if sequence > 10 {
            samples_micros.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
            maximum_payload_bytes = maximum_payload_bytes.max(encoded.len());
        }

        let live_started = Instant::now();
        let live_snapshot =
            snapshot_envelope_without_library(&runtime, sequence, "live-snapshot-baseline")
                .unwrap_or_else(|error| panic!("live snapshot projection must succeed: {error}"));
        assert!(!live_snapshot.payload.contains_key("library"));
        let live_encoded = serde_json::to_vec(&live_snapshot)
            .unwrap_or_else(|error| panic!("live snapshot encoding must succeed: {error}"));
        if sequence > 10 {
            live_samples_micros
                .push(u64::try_from(live_started.elapsed().as_micros()).unwrap_or(u64::MAX));
            maximum_live_payload_bytes = maximum_live_payload_bytes.max(live_encoded.len());
        }
    }

    samples_micros.sort_unstable();
    let percentile = |percent: usize| {
        let index = samples_micros
            .len()
            .saturating_mul(percent)
            .div_ceil(100)
            .saturating_sub(1);
        samples_micros[index]
    };
    let p50 = percentile(50);
    let p95 = percentile(95);
    let p99 = percentile(99);
    let maximum = *samples_micros.last().unwrap_or(&0);
    live_samples_micros.sort_unstable();
    let live_p95 = live_samples_micros[live_samples_micros
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1)];
    eprintln!(
        "Engine snapshot baseline: samples={} full_p50={}us full_p95={}us full_p99={}us full_max={}us full_payload={}bytes live_p95={}us live_payload={}bytes",
        samples_micros.len(),
        p50,
        p95,
        p99,
        maximum,
        maximum_payload_bytes,
        live_p95,
        maximum_live_payload_bytes,
    );

    assert!(p95 <= 25_000, "full snapshot p95 exceeded 25 ms");
    assert!(
        maximum_payload_bytes <= 2_000_000,
        "full snapshot exceeded the 2 MB protocol safety budget"
    );
    assert!(
        maximum_live_payload_bytes < maximum_payload_bytes,
        "the Live projection must be smaller than the full library snapshot"
    );
}

#[test]
fn canonical_app_to_output_scenario_matches_release_evidence() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    let Some(initial_plan) = runtime
        .state
        .state()
        .plan(lumi_domain::DeckId::new(2))
        .cloned()
    else {
        panic!("demo must generate the next plan");
    };
    let revised = match runtime.planning_worker.planner.select_scene(
        &initial_plan,
        1,
        lumi_domain::SceneId::new(9),
    ) {
        Ok(plan) => plan,
        Err(error) => panic!("canonical scene edit must succeed: {error}"),
    };
    if let Err(error) = runtime
        .planning_worker
        .accept_revised_plan(&mut runtime.state, revised)
    {
        panic!("canonical scene edit must enter the runtime: {error}");
    }
    let Some(revised_plan) = runtime
        .state
        .state()
        .plan(lumi_domain::DeckId::new(2))
        .cloned()
    else {
        panic!("revised plan must remain available");
    };
    let locked = match runtime
        .planning_worker
        .planner
        .set_cue_lock(&revised_plan, 1, true)
    {
        Ok(plan) => plan,
        Err(error) => panic!("canonical cue lock must succeed: {error}"),
    };
    if let Err(error) = runtime
        .planning_worker
        .accept_revised_plan(&mut runtime.state, locked)
    {
        panic!("canonical cue lock must enter the runtime: {error}");
    }

    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetSimulationSpeed {
            expected_revision,
            speed: SimulationSpeed::SixtyFour,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Arm,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceToNextTrack { expected_revision }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks: 250,
        }
    });
    let output_count_before_pause = runtime.output_worker.provider.records().count();
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Pause,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetSimulationPlayback {
            expected_revision,
            playing: false,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks: 500,
        }
    });
    assert_eq!(
        runtime.output_worker.provider.records().count(),
        output_count_before_pause
    );
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetSimulationPlayback {
            expected_revision,
            playing: true,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks: 750,
        }
    });

    let actual = canonical_e2e_evidence(&runtime, output_count_before_pause);
    let expected = include_bytes!("../../../../fixtures/demo-session-v1/canonical-e2e.json");
    assert_eq!(
        String::from_utf8_lossy(&actual),
        String::from_utf8_lossy(expected)
    );
}

#[test]
fn queue_overload_forces_pause_and_cannot_emit_new_output() {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_operation(&mut runtime, 1, OperationCommand::Arm);
    apply_operation(&mut runtime, 2, OperationCommand::Start);
    apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
    let records_before_fault = runtime.output_worker.provider.records().count();
    if let Err(error) = process_domain_event(
        &mut runtime.state,
        &mut runtime.output_worker,
        DomainEvent::QueueOverloaded(lumi_domain::QueueOverloadEvent {
            occurred_at: MonotonicTime::new(1),
            rejected_kind: lumi_domain::DomainEventKind::Observation,
            rejected_critical: false,
            occurrences: 1,
        }),
    ) {
        panic!("queue overload must reduce safely: {error}");
    }
    assert_eq!(runtime.state.state().operation(), OperationState::Paused);
    assert_eq!(runtime.state.state().health(), RuntimeHealth::Degraded);
    assert_eq!(
        runtime.output_worker.provider.records().count(),
        records_before_fault
    );

    apply_simulation_control(
        &mut runtime,
        SimulationControl::SetSpeed(SimulationSpeed::SixtyFour),
    );
    assert!(runtime.clock.advance(1_000).is_some());
    if let Err(error) = runtime.deck_source.update_to_clock() {
        panic!("fault playback must remain processable: {error}");
    }
    if let Err(error) = process_pending_source_events(&mut runtime) {
        panic!("fault playback events must drain: {error}");
    }
    assert_eq!(
        runtime.output_worker.provider.records().count(),
        records_before_fault
    );
}

fn canonical_e2e_evidence(runtime: &EngineRuntime, paused_output_count: usize) -> Vec<u8> {
    let state = runtime.state.state();
    let Some(plan) = state.plan(lumi_domain::DeckId::new(2)) else {
        panic!("canonical evidence requires the deck 2 plan");
    };
    let Some(locked_cue) = plan.cues().get(1) else {
        panic!("canonical evidence requires the edited cue");
    };
    let timeline = state.timeline().collect::<Vec<_>>();
    let value = json!({
        "scenarioVersion": 1,
        "engineVersion": env!("CARGO_PKG_VERSION"),
        "operationState": operation_state_name(state.operation()),
        "simulation": {
            "speed": runtime.deck_source.speed().multiplier(),
            "paused": runtime.deck_source.is_paused(),
        },
        "leaderDeckId": state.leader_deck().map(lumi_domain::DeckId::value),
        "leaderBeat": state
            .deck(lumi_domain::DeckId::new(2))
            .map(lumi_domain::DeckState::beat),
        "plan": {
            "planId": plan.id().value().to_string(),
            "revision": plan.revision().value(),
            "status": plan_status_name(plan.status()),
            "lockedCue": {
                "phraseIndex": locked_cue.phrase_index(),
                "locked": locked_cue.locked(),
                "origin": cue_origin_name(locked_cue.origin()),
                "action": action_json(locked_cue.action()),
            },
        },
        "pausedOutputCount": paused_output_count,
        "outputRecordCount": runtime.output_worker.provider.records().count(),
        "outputEffects": state.output_effects().map(|result| {
            let request = result.request();
            json!({
                "commandId": request.command_id().value(),
                "phraseIndex": request.phrase_index(),
                "planRevision": request.plan_revision().value(),
                "scheduledAt": request.scheduled_at().ticks(),
                "status": output_effect_status_name(result.status()),
                "action": action_json(request.action()),
            })
        }).collect::<Vec<_>>(),
        "timeline": {
            "entryCount": timeline.len(),
            "lastSequence": timeline.last().map(|entry| entry.sequence()),
            "simulatedOutputEntries": timeline
                .iter()
                .filter(|entry| entry.result() == TimelineResult::Simulated)
                .count(),
        },
    });
    let mut encoded = match serde_json::to_vec_pretty(&value) {
        Ok(encoded) => encoded,
        Err(error) => panic!("canonical evidence must encode: {error}"),
    };
    encoded.push(b'\n');
    encoded
}

fn canonical_library_simulation_evidence(
    preview: &Map<String, Value>,
    activated: &Map<String, Value>,
    completed: &Map<String, Value>,
) -> Vec<u8> {
    let next = &preview["nextPlan"];
    let cues = next["cues"]
        .as_array()
        .unwrap_or_else(|| panic!("golden evidence requires preview cues"))
        .iter()
        .map(|cue| {
            json!({
                "phraseIndex": cue["phraseIndex"],
                "startBeat": cue["startBeat"],
                "endBeat": cue["endBeat"],
                "roleId": cue["libraryResolution"]["roleId"],
                "strategy": cue["libraryResolution"]["strategy"],
                "variantId": cue["libraryResolution"]["variantId"],
                "entryId": cue["libraryResolution"]["dryRunEntry"]["id"],
            })
        })
        .collect::<Vec<_>>();
    let outputs = completed["outputEffects"]
        .as_array()
        .unwrap_or_else(|| panic!("golden evidence requires output effects"))
        .iter()
        .filter(|effect| effect["libraryResolution"]["dryRunEntry"]["id"].is_string())
        .map(|effect| {
            json!({
                "phraseIndex": effect["phraseIndex"],
                "status": effect["status"],
                "entryId": effect["libraryResolution"]["dryRunEntry"]["id"],
            })
        })
        .collect::<Vec<_>>();
    let evidence = json!({
        "scenarioVersion": 1,
        "next": {
            "deckId": next["deckId"],
            "trackId": next["trackId"],
            "trackLoadId": next["trackLoadId"],
            "libraryTrack": next["libraryTrack"],
            "themeDecision": next["themeDecision"],
            "cues": cues,
        },
        "activation": {
            "leaderDeckId": activated["leaderDeckId"],
            "activePlan": activated["activePlan"],
        },
        "outputs": outputs,
    });
    let mut bytes = serde_json::to_vec_pretty(&evidence)
        .unwrap_or_else(|error| panic!("golden evidence must encode: {error}"));
    bytes.push(b'\n');
    bytes
}

fn semantic_output_order(speed: SimulationSpeed, elapsed_ticks: u64) -> Vec<(u16, u64)> {
    let mut runtime = match initialized_runtime() {
        Ok(runtime) => runtime,
        Err(error) => panic!("test engine must initialize: {error}"),
    };
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetSimulationSpeed {
            expected_revision,
            speed,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Arm,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::SetOperationState {
            expected_revision,
            command: OperationCommand::Start,
        }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceToNextTrack { expected_revision }
    });
    apply_current_session_command(&mut runtime, |expected_revision| {
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks,
        }
    });
    runtime
        .output_worker
        .provider
        .records()
        .map(|result| {
            (
                result.request().phrase_index(),
                result.request().cue_id().value(),
            )
        })
        .collect()
}

fn apply_session_command(runtime: &mut EngineRuntime, command: SessionCommand) {
    if let Err(error) = apply_command(runtime, command) {
        panic!("test session command must apply: {error}");
    }
}

fn apply_current_session_command(
    runtime: &mut EngineRuntime,
    command: impl FnOnce(lumi_domain::StateRevision) -> SessionCommand,
) {
    let expected_revision = runtime.state.state().revision();
    apply_session_command(runtime, command(expected_revision));
}

fn apply_operation(runtime: &mut EngineRuntime, sequence: u64, command: OperationCommand) {
    let expected_state_revision = runtime.state.state().revision();
    let event = DomainEvent::UserCommand(UserCommandEnvelope {
        client_id: ClientId::new(1),
        sequence: CommandSequence::new(sequence),
        expected_state_revision,
        issued_at: MonotonicTime::new(sequence),
        command,
    });
    if let Err(error) = process_domain_event(&mut runtime.state, &mut runtime.output_worker, event)
    {
        panic!("test operation must apply: {error}");
    }
}

fn apply_simulation_control(runtime: &mut EngineRuntime, control: SimulationControl) {
    if let Err(error) = runtime.deck_source.apply_control(control) {
        panic!("test simulation control must apply: {error}");
    }
    if let Err(error) = process_pending_source_events(runtime) {
        panic!("test source events must process: {error}");
    }
}
