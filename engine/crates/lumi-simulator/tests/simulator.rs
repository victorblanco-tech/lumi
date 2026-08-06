use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckId, DeckObservation, DomainEvent, MonotonicTime, TrackIdentityFacts, TrackLoadId,
};
use lumi_simulator::{
    ManualClock, SimulationControl, SimulationSpeed, SimulatorDeckSourceProvider, SimulatorError,
    canonical_transcript,
};

#[test]
fn initial_events_publish_both_tracks_before_the_leader() {
    let clock = ManualClock::new(0);
    let mut provider = demo(clock);
    let events = drain(&mut provider);

    assert!(matches!(
        observation(&events[0]),
        DeckObservation::SourceStatusChanged { .. }
    ));
    assert!(matches!(
        observation(&events[1]),
        DeckObservation::TrackLoaded { deck_id, .. } if *deck_id == DeckId::new(1)
    ));
    assert!(matches!(
        observation(&events[2]),
        DeckObservation::TrackLoaded { deck_id, .. } if *deck_id == DeckId::new(2)
    ));
    assert!(matches!(
        observation(&events[3]),
        DeckObservation::LeaderChanged { deck_id, .. } if *deck_id == DeckId::new(1)
    ));
    assert!(matches!(
        observation(&events[4]),
        DeckObservation::PlaybackPosition { beat: 0, .. }
    ));
    assert!(matches!(
        observation(&events[5]),
        DeckObservation::PhraseChanged {
            phrase_index: 0,
            ..
        }
    ));
}

#[test]
fn loading_a_library_track_emits_one_normalized_identity_safe_deck_event() {
    let clock = ManualClock::new(0);
    let mut provider = demo(clock);
    let initial = drain(&mut provider);
    let metadata = initial
        .iter()
        .find_map(|event| match observation(event) {
            DeckObservation::TrackLoaded { metadata, .. } => Some(metadata.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("demo must contain a track"))
        .with_identity_facts(
            TrackIdentityFacts::try_new("demo", "library-v1", "source-track-1", "a1", 7)
                .unwrap_or_else(|error| panic!("identity must be valid: {error}")),
        );

    let load_id = provider
        .load_track(DeckId::new(2), metadata)
        .unwrap_or_else(|error| panic!("library track must load: {error}"));
    assert_eq!(load_id, TrackLoadId::new(2002));
    let events = drain(&mut provider);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        observation(&events[0]),
        DeckObservation::TrackLoaded {
            deck_id,
            track_load_id,
            metadata,
        } if *deck_id == DeckId::new(2)
            && *track_load_id == load_id
            && metadata.identity_facts().is_some_and(|facts| {
                facts.source_track_id() == "source-track-1"
                    && facts.lumi_timeline_revision() == 7
            })
    ));
}

#[test]
fn loading_the_leader_deck_resets_position_without_fabricating_a_leader_change() {
    let mut provider = demo(ManualClock::new(0));
    let initial = drain(&mut provider);
    let metadata = initial
        .iter()
        .find_map(|event| match observation(event) {
            DeckObservation::TrackLoaded { metadata, .. } => Some(metadata.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("demo must contain a track"));

    let load_id = provider
        .load_track(DeckId::new(1), metadata)
        .unwrap_or_else(|error| panic!("leader track must load: {error}"));
    let events = drain(&mut provider);
    assert_eq!(events.len(), 3);
    assert!(matches!(
        observation(&events[0]),
        DeckObservation::TrackLoaded { deck_id, .. } if *deck_id == DeckId::new(1)
    ));
    assert!(matches!(
        observation(&events[1]),
        DeckObservation::PlaybackPosition {
            deck_id,
            track_load_id,
            beat: 0,
        } if *deck_id == DeckId::new(1) && *track_load_id == load_id
    ));
    assert!(matches!(
        observation(&events[2]),
        DeckObservation::PhraseChanged {
            deck_id,
            track_load_id,
            phrase_index: 0,
        } if *deck_id == DeckId::new(1) && *track_load_id == load_id
    ));
    assert_eq!(provider.leader_deck_id(), DeckId::new(1));
}

#[test]
fn same_fixture_and_commands_produce_the_same_transcript() {
    let first_clock = ManualClock::new(0);
    let second_clock = ManualClock::new(0);
    let mut first = demo(first_clock.clone());
    let mut second = demo(second_clock.clone());

    let mut first_events = drain(&mut first);
    let mut second_events = drain(&mut second);
    for (first_control, second_control, ticks) in [
        (
            SimulationControl::SetSpeed(SimulationSpeed::Four),
            SimulationControl::SetSpeed(SimulationSpeed::Four),
            1_000,
        ),
        (
            SimulationControl::AdvanceLeader,
            SimulationControl::AdvanceLeader,
            500,
        ),
        (SimulationControl::Pause, SimulationControl::Pause, 5_000),
        (SimulationControl::Resume, SimulationControl::Resume, 1_000),
    ] {
        assert!(first.apply_control(first_control).is_ok());
        assert!(second.apply_control(second_control).is_ok());
        assert!(first_clock.advance(ticks).is_some());
        assert!(second_clock.advance(ticks).is_some());
        assert!(first.update_to_clock().is_ok());
        assert!(second.update_to_clock().is_ok());
        first_events.extend(drain(&mut first));
        second_events.extend(drain(&mut second));
    }

    assert_eq!(transcript(&first_events), transcript(&second_events));
}

#[test]
fn sixty_four_x_emits_every_beat_and_phrase_boundary() {
    let clock = ManualClock::new(0);
    let mut provider = demo(clock.clone());
    drain(&mut provider);
    assert!(
        provider
            .apply_control(SimulationControl::SetSpeed(SimulationSpeed::SixtyFour))
            .is_ok()
    );
    assert!(clock.advance(1_000).is_some());
    assert!(provider.update_to_clock().is_ok());
    let events = drain(&mut provider);

    let beats: Vec<u32> = events
        .iter()
        .filter_map(|event| match observation(event) {
            DeckObservation::PlaybackPosition { beat, .. } => Some(*beat),
            _ => None,
        })
        .collect();
    let phrases: Vec<u16> = events
        .iter()
        .filter_map(|event| match observation(event) {
            DeckObservation::PhraseChanged { phrase_index, .. } => Some(*phrase_index),
            _ => None,
        })
        .collect();

    assert_eq!(beats, (1..=128).collect::<Vec<_>>());
    assert_eq!(phrases, vec![1, 2, 3]);
}

#[test]
fn pause_resume_speed_and_leader_controls_are_deterministic() {
    let clock = ManualClock::new(0);
    let mut provider = demo(clock.clone());
    drain(&mut provider);

    assert!(provider.apply_control(SimulationControl::Pause).is_ok());
    assert!(clock.advance(10_000).is_some());
    assert!(provider.update_to_clock().is_ok());
    let paused_events = drain(&mut provider);
    assert_eq!(paused_events.len(), 1);
    assert!(matches!(
        observation(&paused_events[0]),
        DeckObservation::PlaybackStateChanged { playing: false, .. }
    ));

    assert!(provider.apply_control(SimulationControl::Resume).is_ok());
    assert!(
        provider
            .apply_control(SimulationControl::SetSpeed(SimulationSpeed::Sixteen))
            .is_ok()
    );
    assert!(clock.advance(500).is_some());
    assert!(provider.update_to_clock().is_ok());
    assert!(!drain(&mut provider).is_empty());

    assert!(
        provider
            .apply_control(SimulationControl::AdvanceLeader)
            .is_ok()
    );
    let leader_events = drain(&mut provider);
    assert_eq!(provider.leader_deck_id(), DeckId::new(2));
    assert!(leader_events.iter().any(|event| matches!(
        observation(event),
        DeckObservation::LeaderChanged { deck_id, .. } if *deck_id == DeckId::new(2)
    )));
}

#[test]
fn reset_restores_a_byte_equivalent_canonical_snapshot() {
    let clock = ManualClock::new(0);
    let mut provider = demo(clock.clone());
    let initial = encode_snapshot(&provider);
    drain(&mut provider);

    assert!(
        provider
            .apply_control(SimulationControl::SetSpeed(SimulationSpeed::SixtyFour))
            .is_ok()
    );
    assert!(clock.advance(500).is_some());
    assert!(provider.update_to_clock().is_ok());
    drain(&mut provider);
    assert!(
        provider
            .apply_control(SimulationControl::AdvanceLeader)
            .is_ok()
    );
    drain(&mut provider);
    assert!(provider.apply_control(SimulationControl::Reset).is_ok());

    assert_eq!(initial, encode_snapshot(&provider));
}

#[test]
fn clock_regression_is_a_typed_error() {
    let clock = ManualClock::new(10);
    let mut provider = demo(clock.clone());
    drain(&mut provider);
    clock.set(9);

    assert!(matches!(
        provider.update_to_clock(),
        Err(SimulatorError::ClockRegressed {
            previous,
            current,
        }) if previous == MonotonicTime::new(10) && current == MonotonicTime::new(9)
    ));
}

#[test]
fn initial_transcript_matches_the_reviewed_golden_file() {
    let mut provider = demo(ManualClock::new(0));
    let transcript = transcript(&drain(&mut provider));
    let expected = include_bytes!("../../../../fixtures/demo-session-v1/initial-transcript.ndjson");
    assert_eq!(transcript, expected);
}

fn demo(clock: ManualClock) -> SimulatorDeckSourceProvider<ManualClock> {
    match SimulatorDeckSourceProvider::demo(clock) {
        Ok(provider) => provider,
        Err(error) => panic!("demo fixture must be valid: {error}"),
    }
}

fn drain(provider: &mut SimulatorDeckSourceProvider<ManualClock>) -> Vec<DomainEvent> {
    match provider.drain_events() {
        Ok(events) => events,
        Err(error) => panic!("simulator drain must succeed: {error}"),
    }
}

fn observation(event: &DomainEvent) -> &DeckObservation {
    match event {
        DomainEvent::Observation(envelope) => &envelope.observation,
        _ => panic!("simulator must emit observation events"),
    }
}

fn transcript(events: &[DomainEvent]) -> Vec<u8> {
    match canonical_transcript(events) {
        Ok(bytes) => bytes,
        Err(error) => panic!("transcript must encode: {error}"),
    }
}

fn encode_snapshot(provider: &SimulatorDeckSourceProvider<ManualClock>) -> Vec<u8> {
    match serde_json::to_vec(&provider.canonical_snapshot()) {
        Ok(bytes) => bytes,
        Err(error) => panic!("snapshot must encode: {error}"),
    }
}
