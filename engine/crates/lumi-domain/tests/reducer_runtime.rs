use lumi_domain::{
    ClientId, CommandSequence, CueId, CueOrigin, CueReason, DecisionReason, DeckId,
    DeckObservation, DomainEvent, DomainEventKind, Effect, EffectId, EffectResult,
    EffectResultEnvelope, EffectSequence, IngressError, IngressOutcome, KeyMode, LightingCue,
    LightingLook, LightingPlan, LoopSelection, MonotonicTime, MusicalKey, ObservationEnvelope,
    OperationCommand, OperationState, PhraseKind, PitchClass, PlanConfigurationRevision, PlanId,
    PlanRevision, PlanStatus, ReducerError, RuntimeHealth, SceneCategory, SemanticLightingAction,
    SerializedRuntime, SourceId, SourceSequence, StateRevision, ThemeId, TrackId, TrackLoadId,
    TrackMetadata, TrackPhrase, UserCommandEnvelope, WorkerId,
};

#[test]
fn operation_transition_table_is_explicit() {
    let mut runtime = started_runtime(16);

    for (sequence, command, expected_state) in [
        (1, OperationCommand::Arm, OperationState::Armed),
        (2, OperationCommand::Start, OperationState::Live),
        (3, OperationCommand::Pause, OperationState::Paused),
        (4, OperationCommand::Start, OperationState::Live),
        (5, OperationCommand::Off, OperationState::Off),
    ] {
        let revision = runtime.state().revision();
        let result = submit_and_process(&mut runtime, command_event(sequence, revision, command));
        assert_eq!(result.decision, DecisionReason::OperationTransitionAccepted);
        assert_eq!(runtime.state().operation(), expected_state);
    }

    let revision = runtime.state().revision();
    assert!(
        runtime
            .submit(command_event(6, revision, OperationCommand::Pause))
            .is_ok()
    );
    assert_eq!(
        runtime.process_next(),
        Err(ReducerError::InvalidOperationTransition {
            from: OperationState::Off,
            command: OperationCommand::Pause,
        })
    );
    assert_eq!(runtime.state().operation(), OperationState::Off);
}

#[test]
fn identical_event_sequences_produce_identical_state_and_effects() {
    let events = vec![
        track_loaded(1, 10, 100, 1),
        position(2, 10, 32, 2),
        position(3, 10, 64, 3),
    ];
    let mut first = started_runtime(16);
    let mut second = started_runtime(16);

    let first_results = process_all(&mut first, events.clone());
    let second_results = process_all(&mut second, events);

    assert_eq!(first.state(), second.state());
    assert_eq!(first_results, second_results);
}

#[test]
fn stale_observations_and_old_track_loads_cannot_roll_deck_state_back() {
    let mut runtime = started_runtime(16);
    submit_and_process(&mut runtime, track_loaded(1, 10, 100, 1));
    submit_and_process(&mut runtime, position(2, 10, 64, 2));
    submit_and_process(&mut runtime, track_loaded(3, 11, 200, 3));

    let mismatch = submit_and_process(&mut runtime, position(4, 10, 96, 4));
    let stale = submit_and_process(&mut runtime, position(2, 11, 128, 5));

    assert_eq!(mismatch.decision, DecisionReason::TrackLoadMismatch);
    assert_eq!(stale.decision, DecisionReason::StaleObservationIgnored);
    let deck = runtime.state().deck(DeckId::new(1));
    assert_eq!(
        deck.map(|value| value.track_load_id()),
        Some(TrackLoadId::new(11))
    );
    assert_eq!(deck.map(|value| value.track_id()), Some(TrackId::new(200)));
    assert_eq!(deck.map(|value| value.beat()), Some(0));
}

#[test]
fn explicit_seek_can_move_the_active_track_backward() {
    let mut runtime = started_runtime(16);
    submit_and_process(&mut runtime, track_loaded(1, 10, 100, 1));
    submit_and_process(&mut runtime, position(2, 10, 96, 2));

    let result = submit_and_process(&mut runtime, seek_position(3, 10, 24, 3));

    assert_eq!(result.decision, DecisionReason::PositionSeeked);
    assert_eq!(
        runtime.state().deck(DeckId::new(1)).map(|deck| deck.beat()),
        Some(24)
    );
}

#[test]
fn every_non_increasing_source_sequence_is_ignored() {
    for stale_sequence in 0..=64 {
        let mut runtime = started_runtime(8);
        submit_and_process(&mut runtime, track_loaded(65, 10, 100, 65));
        let result = submit_and_process(&mut runtime, position(stale_sequence, 10, 999, 66));

        assert_eq!(result.decision, DecisionReason::StaleObservationIgnored);
        assert_eq!(
            runtime.state().deck(DeckId::new(1)).map(|deck| deck.beat()),
            Some(0)
        );
    }
}

#[test]
fn duplicate_user_command_is_idempotent() {
    let mut runtime = started_runtime(8);
    let command = command_event(1, StateRevision::new(1), OperationCommand::Arm);

    let accepted = submit_and_process(&mut runtime, command.clone());
    let accepted_revision = runtime.state().revision();
    let duplicate = submit_and_process(&mut runtime, command);

    assert_eq!(
        accepted.decision,
        DecisionReason::OperationTransitionAccepted
    );
    assert_eq!(duplicate.decision, DecisionReason::DuplicateCommandIgnored);
    assert_eq!(runtime.state().operation(), OperationState::Armed);
    assert_eq!(runtime.state().revision(), accepted_revision);
    assert!(duplicate.effects.is_empty());
}

#[test]
fn plan_revision_and_track_load_identity_are_enforced() {
    let mut runtime = started_runtime(16);
    submit_and_process(&mut runtime, track_loaded(1, 10, 100, 1));

    let accepted = submit_and_process(
        &mut runtime,
        plan_result(1, plan(PlanRevision::initial(), TrackLoadId::new(10))),
    );
    let stale = submit_and_process(
        &mut runtime,
        plan_result(2, plan(PlanRevision::initial(), TrackLoadId::new(10))),
    );
    let mismatch = submit_and_process(
        &mut runtime,
        plan_result(3, plan(PlanRevision::new(2), TrackLoadId::new(99))),
    );

    assert_eq!(accepted.decision, DecisionReason::PlanAccepted);
    assert_eq!(stale.decision, DecisionReason::StalePlanIgnored);
    assert_eq!(mismatch.decision, DecisionReason::PlanTrackLoadMismatch);

    assert!(
        runtime
            .submit(plan_result(
                4,
                plan(PlanRevision::new(3), TrackLoadId::new(10))
            ))
            .is_ok()
    );
    assert_eq!(
        runtime.process_next(),
        Err(ReducerError::PlanRevisionGap {
            expected: PlanRevision::new(2),
            actual: PlanRevision::new(3),
        })
    );
}

#[test]
fn live_phrase_execution_requires_playback_and_deduplicates_the_same_cue() {
    let mut runtime = started_runtime(32);
    submit_and_process(&mut runtime, track_loaded(1, 10, 100, 1));
    submit_and_process(
        &mut runtime,
        plan_result(1, plan(PlanRevision::initial(), TrackLoadId::new(10))),
    );
    submit_and_process(&mut runtime, leader_changed(2, 10, 2));
    let revision = runtime.state().revision();
    submit_and_process(
        &mut runtime,
        command_event(1, revision, OperationCommand::Arm),
    );
    let revision = runtime.state().revision();
    submit_and_process(
        &mut runtime,
        command_event(2, revision, OperationCommand::Start),
    );

    let stopped = submit_and_process(&mut runtime, phrase_changed(3, 10, 0, 3));
    assert_eq!(stopped.decision, DecisionReason::PhraseChanged);
    assert!(stopped.effects.is_empty());

    submit_and_process(&mut runtime, playback_state(4, 10, true, 4));
    let first = submit_and_process(&mut runtime, phrase_changed(5, 10, 0, 5));
    assert_eq!(first.decision, DecisionReason::PhraseExecutionScheduled);
    assert!(matches!(first.effects.as_slice(), [Effect::ExecuteCue(_)]));

    let duplicate = submit_and_process(&mut runtime, phrase_changed(6, 10, 0, 6));
    assert_eq!(duplicate.decision, DecisionReason::PhraseExecutionSkipped);
    assert!(duplicate.effects.is_empty());
}

#[test]
fn overload_records_a_safe_diagnostic_and_preserves_critical_ingress() {
    let mut runtime = started_runtime(2);
    assert_eq!(
        runtime.submit(track_loaded(1, 10, 100, 1)),
        Ok(IngressOutcome::Accepted)
    );
    assert_eq!(
        runtime.submit(position(2, 10, 16, 2)),
        Ok(IngressOutcome::Accepted)
    );

    let critical = command_event(1, runtime.state().revision(), OperationCommand::Arm);
    assert_eq!(
        runtime.submit(critical),
        Ok(IngressOutcome::AcceptedAfterEvictingNonCritical {
            evicted_kind: DomainEventKind::Observation,
        })
    );

    let overload = process_next(&mut runtime);
    assert_eq!(overload.event_kind, DomainEventKind::QueueOverloaded);
    assert_eq!(overload.decision, DecisionReason::QueueSaturated);
    assert_eq!(
        overload.effects,
        vec![Effect::EnsureOutputClosed {
            reason: DecisionReason::QueueSaturated,
        }]
    );
    assert_eq!(runtime.state().health(), RuntimeHealth::Degraded);
    assert_eq!(runtime.state().diagnostics().count(), 1);
}

#[test]
fn saturation_of_an_all_critical_queue_is_an_explicit_typed_error() {
    let mut runtime = started_runtime(1);
    let revision = runtime.state().revision();
    assert!(
        runtime
            .submit(command_event(1, revision, OperationCommand::Arm))
            .is_ok()
    );

    let error = runtime.submit(DomainEvent::EffectResult(EffectResultEnvelope {
        effect_id: EffectId::new(9),
        worker_id: WorkerId::new(1),
        sequence: EffectSequence::new(1),
        completed_at: MonotonicTime::new(9),
        result: EffectResult::OutputGateClosed,
    }));
    assert_eq!(
        error,
        Err(IngressError::Saturated {
            rejected_kind: DomainEventKind::EffectResult,
            critical: true,
        })
    );
    assert!(runtime.queue_depth() <= runtime.queue_capacity() + 1);
}

fn started_runtime(capacity: usize) -> SerializedRuntime {
    let Ok(mut runtime) = SerializedRuntime::try_new(capacity) else {
        panic!("test queue capacity must be valid");
    };
    assert!(
        runtime
            .submit(DomainEvent::RuntimeStarted {
                at: MonotonicTime::new(0),
            })
            .is_ok()
    );
    let result = process_next(&mut runtime);
    assert_eq!(result.decision, DecisionReason::RuntimeInitialized);
    runtime
}

fn process_all(
    runtime: &mut SerializedRuntime,
    events: Vec<DomainEvent>,
) -> Vec<lumi_domain::ProcessResult> {
    events
        .into_iter()
        .map(|event| submit_and_process(runtime, event))
        .collect()
}

fn submit_and_process(
    runtime: &mut SerializedRuntime,
    event: DomainEvent,
) -> lumi_domain::ProcessResult {
    assert!(runtime.submit(event).is_ok());
    process_next(runtime)
}

fn process_next(runtime: &mut SerializedRuntime) -> lumi_domain::ProcessResult {
    match runtime.process_next() {
        Ok(Some(result)) => result,
        Ok(None) => panic!("test expected a queued event"),
        Err(error) => panic!("test reducer failed: {error}"),
    }
}

fn track_loaded(sequence: u64, load: u64, track: u64, at: u64) -> DomainEvent {
    DomainEvent::Observation(ObservationEnvelope {
        source_id: SourceId::new(1),
        sequence: SourceSequence::new(sequence),
        observed_at: MonotonicTime::new(at),
        observation: DeckObservation::TrackLoaded {
            deck_id: DeckId::new(1),
            metadata: track_metadata(track),
            track_load_id: TrackLoadId::new(load),
        },
    })
}

fn track_metadata(track: u64) -> TrackMetadata {
    let result = TrackMetadata::try_new(
        TrackId::new(track),
        format!("Track {track}"),
        "Lumi Test".to_owned(),
        128_000,
        MusicalKey::new(PitchClass::A, KeyMode::Minor),
        128,
        vec![TrackPhrase::new(0, 0, 128, PhraseKind::Intro)],
    );
    match result {
        Ok(metadata) => metadata,
        Err(error) => panic!("test metadata must be valid: {error}"),
    }
}

fn position(sequence: u64, load: u64, beat: u32, at: u64) -> DomainEvent {
    DomainEvent::Observation(ObservationEnvelope {
        source_id: SourceId::new(1),
        sequence: SourceSequence::new(sequence),
        observed_at: MonotonicTime::new(at),
        observation: DeckObservation::PlaybackPosition {
            deck_id: DeckId::new(1),
            track_load_id: TrackLoadId::new(load),
            beat,
        },
    })
}

fn seek_position(sequence: u64, load: u64, beat: u32, at: u64) -> DomainEvent {
    DomainEvent::Observation(ObservationEnvelope {
        source_id: SourceId::new(1),
        sequence: SourceSequence::new(sequence),
        observed_at: MonotonicTime::new(at),
        observation: DeckObservation::PlaybackPositionSeeked {
            deck_id: DeckId::new(1),
            track_load_id: TrackLoadId::new(load),
            beat,
        },
    })
}

fn playback_state(sequence: u64, load: u64, playing: bool, at: u64) -> DomainEvent {
    DomainEvent::Observation(ObservationEnvelope {
        source_id: SourceId::new(1),
        sequence: SourceSequence::new(sequence),
        observed_at: MonotonicTime::new(at),
        observation: DeckObservation::PlaybackStateChanged {
            deck_id: DeckId::new(1),
            track_load_id: TrackLoadId::new(load),
            playing,
        },
    })
}

fn phrase_changed(sequence: u64, load: u64, phrase_index: u16, at: u64) -> DomainEvent {
    DomainEvent::Observation(ObservationEnvelope {
        source_id: SourceId::new(1),
        sequence: SourceSequence::new(sequence),
        observed_at: MonotonicTime::new(at),
        observation: DeckObservation::PhraseChanged {
            deck_id: DeckId::new(1),
            track_load_id: TrackLoadId::new(load),
            phrase_index,
        },
    })
}

fn leader_changed(sequence: u64, load: u64, at: u64) -> DomainEvent {
    DomainEvent::Observation(ObservationEnvelope {
        source_id: SourceId::new(1),
        sequence: SourceSequence::new(sequence),
        observed_at: MonotonicTime::new(at),
        observation: DeckObservation::LeaderChanged {
            deck_id: DeckId::new(1),
            track_load_id: TrackLoadId::new(load),
        },
    })
}

fn command_event(sequence: u64, revision: StateRevision, command: OperationCommand) -> DomainEvent {
    DomainEvent::UserCommand(UserCommandEnvelope {
        client_id: ClientId::new(1),
        sequence: CommandSequence::new(sequence),
        expected_state_revision: revision,
        issued_at: MonotonicTime::new(sequence),
        command,
    })
}

fn plan_result(sequence: u64, plan: LightingPlan) -> DomainEvent {
    DomainEvent::EffectResult(EffectResultEnvelope {
        effect_id: EffectId::new(sequence),
        worker_id: WorkerId::new(1),
        sequence: EffectSequence::new(sequence),
        completed_at: MonotonicTime::new(sequence + 10),
        result: EffectResult::PlanGenerated(plan),
    })
}

fn plan(revision: PlanRevision, track_load_id: TrackLoadId) -> LightingPlan {
    let look = match LightingLook::try_new(
        ThemeId::new(1),
        "Test Theme".to_owned(),
        lumi_domain::SceneId::new(1),
        "Test Scene".to_owned(),
        SceneCategory::Ambient,
        LoopSelection::new(1, 1),
    ) {
        Ok(look) => look,
        Err(error) => panic!("test look must be valid: {error}"),
    };
    let result = LightingPlan::try_new(
        PlanId::new(1),
        DeckId::new(1),
        TrackId::new(100),
        128,
        track_load_id,
        revision,
        PlanConfigurationRevision::new(1),
        1,
        PlanStatus::Ready,
        vec![LightingCue::new(
            CueId::new(1),
            0,
            0,
            128,
            SemanticLightingAction::ApplyLook(look),
            CueOrigin::Automatic,
            CueReason::PhraseCategoryMatched {
                phrase_kind: PhraseKind::Intro,
                category: SceneCategory::Ambient,
            },
        )],
    );
    match result {
        Ok(plan) => plan,
        Err(error) => panic!("test plan must be valid: {error}"),
    }
}
