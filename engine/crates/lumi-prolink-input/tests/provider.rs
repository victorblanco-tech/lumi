use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckObservation, DeckSourceStatus, DomainEvent, KeyMode, MonotonicTime, MusicalKey, PhraseKind,
    PitchClass, TrackId, TrackMetadata, TrackPhrase,
};
use lumi_prolink_input::{BridgeDecoder, ProLinkDeckSourceProvider};

const HELLO: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":1,"observedAtNanos":10,"type":"hello","payload":{"bridgeVersion":"0.4.0-dev-20","beatLinkVersion":"8.0.0","readOnly":true}}"#;
const READY: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":2,"observedAtNanos":20,"type":"sourceStatus","payload":{"status":"ready","detail":"network ready"}}"#;
const STATUS: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":3,"observedAtNanos":30,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":17,"beatWithinBar":1,"rawPitch":1082458112}}"#;
const BEAT: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":40000000,"type":"beat","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","effectiveBpm":157.25,"beatWithinBar":2,"tempoMaster":true}}"#;
const REPLACEMENT_AT_PRE_ROLL: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":40,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":false,"paused":true,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1247,"trackBpm":150.0,"effectiveBpm":150.0,"beatNumber":0,"beatWithinBar":0,"rawPitch":1048576}}"#;

#[test]
fn translates_direct_status_into_provider_neutral_observations() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }

    let observations = provider
        .drain_events()
        .unwrap_or_else(|error| panic!("events should drain: {error}"))
        .into_iter()
        .filter_map(|event| match event {
            DomainEvent::Observation(envelope) => Some(envelope.observation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::TrackLoaded { deck_id, .. } if deck_id.value() == 1
    )));
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::PlaybackPosition { beat: 16, .. }
    )));
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::PlaybackTempoChanged {
            bpm_milli: 157_250,
            ..
        }
    )));
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::LeaderChanged { deck_id, .. } if deck_id.value() == 1
    )));
    let identity = provider
        .track_identity(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("track identity should be retained"));
    assert_eq!(identity.rekordbox_id, 1256);
}

#[test]
fn track_change_at_pre_roll_unloads_and_replaces_without_stopping_the_source() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [
        (HELLO, 1),
        (READY, 2),
        (STATUS, 3),
        (REPLACEMENT_AT_PRE_ROLL, 4),
    ] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }

    let observations = provider
        .drain_events()
        .unwrap_or_else(|error| panic!("events should drain: {error}"))
        .into_iter()
        .filter_map(|event| match event {
            DomainEvent::Observation(envelope) => Some(envelope.observation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::TrackUnloaded { track_load_id, .. }
            if track_load_id.value() == 1
    )));
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::TrackLoaded { track_load_id, .. }
            if track_load_id.value() == 2
    )));
    assert!(observations.iter().any(|observation| matches!(
        observation,
        DeckObservation::PlaybackPosition { track_load_id, beat: 0, .. }
            if track_load_id.value() == 2
    )));
    let replacement = provider
        .track_identity(lumi_domain::TrackLoadId::new(2))
        .unwrap_or_else(|| panic!("replacement identity should be retained"));
    assert_eq!(replacement.rekordbox_id, 1247);
}

#[test]
fn preserves_low_latency_master_beat_timestamp_for_timing_output() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3), (BEAT, 4)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }

    let timing = provider.drain_timing_observations();
    let beat = timing
        .last()
        .unwrap_or_else(|| panic!("master beat should become a timing observation"));
    assert_eq!(beat.deck_id.value(), 1);
    assert_eq!(beat.observed_at_nanos, 40_000_000);
    assert_eq!(beat.absolute_beat, 16);
    assert_eq!(beat.effective_bpm_milli, 157_250);
    assert_eq!(beat.beat_within_bar, 2);
    assert!(beat.playing);
}

#[test]
fn status_is_the_only_effective_tempo_authority() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let _ = provider.drain_timing_observations();

    let tempo = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":50000000,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":154.767,"beatNumber":17,"beatWithinBar":1,"rawPitch":1047000}}"#,
        )
        .unwrap_or_else(|error| panic!("tempo status should decode: {error}"));
    provider
        .ingest(tempo, MonotonicTime::new(4))
        .unwrap_or_else(|error| panic!("tempo status should translate: {error}"));
    assert_eq!(
        provider.drain_timing_observations()[0].effective_bpm_milli,
        154_767
    );

    let precise = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":5,"observedAtNanos":60000000,"type":"precisePosition","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","playbackPositionMillis":10000,"effectiveBpm":155.0,"beatWithinBar":1,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("precise position should decode: {error}"));
    provider
        .ingest(precise, MonotonicTime::new(5))
        .unwrap_or_else(|error| panic!("precise position should queue: {error}"));
    let precise = provider
        .drain_precise_position_observations()
        .pop()
        .unwrap_or_else(|| panic!("precise position should be retained"));
    assert_eq!(precise.effective_bpm_milli, 154_767);
    provider
        .apply_authoritative_position(precise, 26, false, MonotonicTime::new(6))
        .unwrap_or_else(|error| panic!("precise position should apply: {error}"));
    let _ = provider.drain_timing_observations();

    let beat = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":6,"observedAtNanos":70000000,"type":"beat","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","effectiveBpm":155.0,"beatWithinBar":2,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("beat should decode: {error}"));
    provider
        .ingest(beat, MonotonicTime::new(7))
        .unwrap_or_else(|error| panic!("beat should translate: {error}"));
    let timing = provider.drain_timing_observations();
    assert_eq!(timing.len(), 1);
    assert_eq!(timing[0].effective_bpm_milli, 154_767);
    assert_eq!(
        provider
            .transport(lumi_domain::TrackLoadId::new(1))
            .unwrap_or_else(|| panic!("transport should remain available"))
            .effective_bpm_milli,
        154_767
    );
}

#[test]
fn bar_relative_beats_never_activate_phrases_and_status_seeks_remain_explicit() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let metadata = TrackMetadata::try_new(
        TrackId::new(99),
        "Hydrated".to_owned(),
        "Lumi".to_owned(),
        155_000,
        MusicalKey::new(PitchClass::C, KeyMode::Minor),
        64,
        vec![
            TrackPhrase::new(0, 0, 32, PhraseKind::Breakdown),
            TrackPhrase::new(1, 32, 64, PhraseKind::Drop),
        ],
    )
    .unwrap_or_else(|error| panic!("metadata should be valid: {error}"));
    assert!(provider.hydrate_track_metadata(lumi_domain::TrackLoadId::new(1), metadata));
    let _ = provider.drain_events();
    let initial_revision = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("loaded deck should expose transport"))
        .discontinuity_revision;

    let boundary = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":41000000,"type":"beat","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","effectiveBpm":157.25,"beatWithinBar":1,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("beat should decode: {error}"));
    provider
        .ingest(boundary, MonotonicTime::new(5))
        .unwrap_or_else(|error| panic!("beat should translate: {error}"));
    let observations = provider
        .drain_events()
        .unwrap_or_else(|error| panic!("events should drain: {error}"));
    assert!(observations.iter().all(|event| !matches!(
        event,
        DomainEvent::Observation(envelope)
            if matches!(envelope.observation, DeckObservation::PhraseChanged { .. })
    )));

    let seek = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":5,"observedAtNanos":42000000,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":33,"beatWithinBar":1,"rawPitch":1082458112}}"#,
        )
        .unwrap_or_else(|error| panic!("seek should decode: {error}"));
    provider
        .ingest(seek, MonotonicTime::new(6))
        .unwrap_or_else(|error| panic!("seek should translate: {error}"));
    let observations = provider
        .drain_events()
        .unwrap_or_else(|error| panic!("events should drain: {error}"));
    assert!(observations.iter().any(|event| matches!(
        event,
        DomainEvent::Observation(envelope)
            if matches!(envelope.observation, DeckObservation::PlaybackPositionSeeked { beat: 32, .. })
    )));
    assert!(observations.iter().all(|event| !matches!(
        event,
        DomainEvent::Observation(envelope)
            if matches!(envelope.observation, DeckObservation::PhraseChanged { .. })
    )));
    let forward_seek_revision = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("seeked deck should expose transport"))
        .discontinuity_revision;
    assert!(forward_seek_revision > initial_revision);

    let backward_seek = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":6,"observedAtNanos":43000000,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":1,"beatWithinBar":1,"rawPitch":1082458112}}"#,
        )
        .unwrap_or_else(|error| panic!("backward seek should decode: {error}"));
    provider
        .ingest(backward_seek, MonotonicTime::new(7))
        .unwrap_or_else(|error| panic!("backward seek should translate: {error}"));
    let backward_seek_revision = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("backward-seeked deck should expose transport"))
        .discontinuity_revision;
    assert!(backward_seek_revision > forward_seek_revision);
}

#[test]
fn precise_position_overrides_a_stale_beat_after_hotcue_before_output_planning() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let metadata = TrackMetadata::try_new(
        TrackId::new(99),
        "Hotcue regression".to_owned(),
        "Lumi".to_owned(),
        155_000,
        MusicalKey::new(PitchClass::C, KeyMode::Minor),
        64,
        vec![
            TrackPhrase::new(0, 0, 32, PhraseKind::Intro),
            TrackPhrase::new(1, 32, 64, PhraseKind::Breakdown),
        ],
    )
    .unwrap_or_else(|error| panic!("metadata should be valid: {error}"));
    assert!(provider.hydrate_track_metadata(lumi_domain::TrackLoadId::new(1), metadata));
    let _ = provider.drain_events();

    let bridge_position = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":100000000,"type":"precisePosition","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","playbackPositionMillis":10000,"effectiveBpm":157.25,"beatWithinBar":1,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("precise position should decode: {error}"));
    provider
        .ingest(bridge_position, MonotonicTime::new(4))
        .unwrap_or_else(|error| panic!("precise position should queue: {error}"));
    let bridge_position = provider
        .drain_precise_position_observations()
        .pop()
        .unwrap_or_else(|| panic!("precise position should be retained"));
    provider
        .apply_authoritative_position(bridge_position, 32, false, MonotonicTime::new(5))
        .unwrap_or_else(|error| panic!("position should apply: {error}"));
    let _ = provider.drain_events();

    // A bar-relative beat can race ahead of the position packet after the DJ
    // presses Hotcue A. It must not advance or select a phrase by itself.
    let raced_beat = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":5,"observedAtNanos":110000000,"type":"beat","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","effectiveBpm":157.25,"beatWithinBar":1,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("raced beat should decode: {error}"));
    provider
        .ingest(raced_beat, MonotonicTime::new(6))
        .unwrap_or_else(|error| panic!("raced beat should translate: {error}"));
    assert!(
        provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events should drain: {error}"))
            .is_empty(),
        "a beat without absolute position may not authorize Bridge"
    );

    let mut applied = None;
    for (sequence, observed_at_nanos, position_millis, at) in
        [(6, 120_000_000, 0, 7), (10, 170_000_000, 50, 8)]
    {
        let intro_position = decoder
            .decode_line(&format!(
                r#"{{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":{sequence},"observedAtNanos":{observed_at_nanos},"type":"precisePosition","payload":{{"deviceNumber":1,"deviceName":"CDJ-1500X","playbackPositionMillis":{position_millis},"effectiveBpm":157.25,"beatWithinBar":1,"tempoMaster":true}}}}"#,
            ))
            .unwrap_or_else(|error| panic!("hotcue position should decode: {error}"));
        provider
            .ingest(intro_position, MonotonicTime::new(at))
            .unwrap_or_else(|error| panic!("hotcue position should queue: {error}"));
        let intro_position = provider
            .drain_precise_position_observations()
            .pop()
            .unwrap_or_else(|| panic!("hotcue position should be retained"));
        applied = provider
            .apply_authoritative_position(intro_position, 0, true, MonotonicTime::new(at + 10))
            .unwrap_or_else(|error| panic!("hotcue position should apply: {error}"));
        if sequence == 6 {
            // Precise packets alone are deliberately insufficient. The
            // independent absolute CdjStatus timeline must also show the
            // actual jump; a normal status frame that merely matches a noisy
            // position is not transport corroboration.
            for (status_sequence, status_nanos) in
                [(7, 130_000_000), (8, 145_000_000), (9, 160_000_000)]
            {
                let landing_status = decoder
                    .decode_line(&format!(
                        r#"{{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":{status_sequence},"observedAtNanos":{status_nanos},"type":"deckStatus","payload":{{"deviceNumber":1,"deviceName":"CDJ-1500X","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":1,"beatWithinBar":1,"rawPitch":1082458112}}}}"#,
                    ))
                    .unwrap_or_else(|error| panic!("hotcue status should decode: {error}"));
                provider
                    .ingest(landing_status, MonotonicTime::new(18))
                    .unwrap_or_else(|error| panic!("hotcue status should translate: {error}"));
                let _ = provider.drain_events();
            }
        }
    }
    let applied = applied.unwrap_or_else(|| panic!("stable hotcue timeline should be confirmed"));
    assert!(applied.discontinuity);
    let observations = provider
        .drain_events()
        .unwrap_or_else(|error| panic!("events should drain: {error}"));
    assert!(observations.iter().any(|event| matches!(
        event,
        DomainEvent::Observation(envelope)
            if matches!(envelope.observation, DeckObservation::PlaybackPositionSeeked { beat: 0, .. })
    )));
    assert!(observations.iter().any(|event| matches!(
        event,
        DomainEvent::Observation(envelope)
            if matches!(envelope.observation, DeckObservation::PhraseChanged { phrase_index: 0, .. })
    )));
    let timing = provider.drain_timing_observations();
    assert_eq!(
        timing.len(),
        1,
        "obsolete pre-hotcue timing must be discarded"
    );
    assert_eq!(timing[0].absolute_beat, 0);
    assert_eq!(timing[0].beat_within_bar, 1);
    assert_eq!(timing[0].effective_bpm_milli, 157_250);
    assert!(timing[0].discontinuity, "Link must re-anchor exactly once");
}

#[test]
fn reordered_precise_position_never_rewinds_the_accepted_timeline() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let metadata = TrackMetadata::try_new(
        TrackId::new(99),
        "Precise reorder regression".to_owned(),
        "Lumi".to_owned(),
        155_000,
        MusicalKey::new(PitchClass::C, KeyMode::Minor),
        256,
        vec![TrackPhrase::new(0, 0, 256, PhraseKind::Intro)],
    )
    .unwrap_or_else(|error| panic!("metadata should be valid: {error}"));
    assert!(provider.hydrate_track_metadata(lumi_domain::TrackLoadId::new(1), metadata));
    let _ = provider.drain_events();

    let first = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":100000000,"type":"precisePosition","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","playbackPositionMillis":48447,"effectiveBpm":155.0,"beatWithinBar":2,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("first position should decode: {error}"));
    provider
        .ingest(first, MonotonicTime::new(4))
        .unwrap_or_else(|error| panic!("first position should queue: {error}"));
    let first = provider
        .drain_precise_position_observations()
        .pop()
        .unwrap_or_else(|| panic!("first position should be retained"));
    provider
        .apply_authoritative_position(first, 125, false, MonotonicTime::new(5))
        .unwrap_or_else(|error| panic!("first position should apply: {error}"));
    let _ = provider.drain_events();
    let before = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("transport should exist"));

    let reordered = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":5,"observedAtNanos":130000000,"type":"precisePosition","payload":{"deviceNumber":1,"deviceName":"CDJ-1500X","playbackPositionMillis":47673,"effectiveBpm":155.0,"beatWithinBar":4,"tempoMaster":true}}"#,
        )
        .unwrap_or_else(|error| panic!("reordered position should decode: {error}"));
    provider
        .ingest(reordered, MonotonicTime::new(6))
        .unwrap_or_else(|error| panic!("reordered position should queue: {error}"));
    let reordered = provider
        .drain_precise_position_observations()
        .pop()
        .unwrap_or_else(|| panic!("reordered position should be retained"));
    let applied = provider
        .apply_authoritative_position(reordered, 123, false, MonotonicTime::new(7))
        .unwrap_or_else(|error| panic!("reordered position should be handled: {error}"));
    assert!(
        applied.is_none(),
        "one stale packet may never become authority"
    );
    let after = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("transport should remain available"));
    assert_eq!(after.beat, before.beat);
    assert_eq!(after.discontinuity_revision, before.discontinuity_revision);
    assert!(
        provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events should drain: {error}"))
            .is_empty(),
        "reordered input may not emit position or phrase changes"
    );
}

#[test]
fn playing_status_frames_never_impersonate_precise_beat_boundaries() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    assert!(
        provider.drain_timing_observations().is_empty(),
        "non-beat-aligned status must not steer Link phase while playing"
    );

    let beat = decoder
        .decode_line(BEAT)
        .unwrap_or_else(|error| panic!("beat fixture should decode: {error}"));
    provider
        .ingest(beat, MonotonicTime::new(4))
        .unwrap_or_else(|error| panic!("beat should translate: {error}"));
    assert_eq!(provider.drain_timing_observations().len(), 1);
}

#[test]
fn playing_tempo_change_updates_link_without_authorizing_a_phrase() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let _ = provider.drain_events();
    let tempo = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":4,"observedAtNanos":50000000,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":160.0,"beatNumber":17,"beatWithinBar":1,"rawPitch":1082458112}}"#,
        )
        .unwrap_or_else(|error| panic!("tempo status should decode: {error}"));
    provider
        .ingest(tempo, MonotonicTime::new(4))
        .unwrap_or_else(|error| panic!("tempo status should translate: {error}"));

    let timing = provider.drain_timing_observations();
    assert_eq!(timing.len(), 1);
    assert_eq!(timing[0].effective_bpm_milli, 160_000);
    assert!(!timing[0].discontinuity);
    assert!(
        provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events should drain: {error}"))
            .iter()
            .all(|event| !matches!(
                event,
                DomainEvent::Observation(envelope)
                    if matches!(envelope.observation, DeckObservation::PhraseChanged { .. })
            ))
    );
}

#[test]
fn delayed_playing_status_progress_does_not_create_a_transport_discontinuity() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3), (BEAT, 4)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let _ = provider.drain_events();
    let _ = provider.drain_timing_observations();
    let before = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("loaded deck should expose transport"));

    // Six beats of ordinary progress after roughly 2.3 seconds at 157.25 BPM.
    // The old fixed `> 2 beats` heuristic classified this as a seek whenever
    // precise Beat packets were delayed or coalesced before the next status.
    let delayed_status = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":5,"observedAtNanos":2330000000,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":24,"beatWithinBar":4,"rawPitch":1082458112}}"#,
        )
        .unwrap_or_else(|error| panic!("delayed status should decode: {error}"));
    provider
        .ingest(delayed_status, MonotonicTime::new(5))
        .unwrap_or_else(|error| panic!("delayed status should translate: {error}"));

    let observations = provider
        .drain_events()
        .unwrap_or_else(|error| panic!("events should drain: {error}"));
    assert!(
        observations.iter().all(|event| !matches!(
            event,
            DomainEvent::Observation(envelope)
                if matches!(envelope.observation, DeckObservation::PlaybackPositionSeeked { .. })
        )),
        "ordinary elapsed-time progress must never look like a seek"
    );
    assert!(
        provider.drain_timing_observations().is_empty(),
        "an asynchronous playing status must not emit a new Link anchor"
    );
    let after = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("loaded deck should retain transport"));
    assert_eq!(after.discontinuity_revision, before.discontinuity_revision);
    assert_eq!(after.beat, 23);
}

#[test]
fn a_late_playing_status_frame_cannot_rewind_the_canonical_deck_beat() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    for (line, time) in [(HELLO, 1), (READY, 2), (STATUS, 3), (BEAT, 4)] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(time))
            .unwrap_or_else(|error| panic!("message should translate: {error}"));
    }
    let _ = provider.drain_events();
    let before = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("loaded deck should expose transport"));
    assert_eq!(before.beat, 16);

    let late_status = decoder
        .decode_line(
            r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":5,"observedAtNanos":50000000,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":17,"beatWithinBar":1,"rawPitch":1082458112}}"#,
        )
        .unwrap_or_else(|error| panic!("late status should decode: {error}"));
    provider
        .ingest(late_status, MonotonicTime::new(5))
        .unwrap_or_else(|error| panic!("late status should translate: {error}"));

    let after = provider
        .transport(lumi_domain::TrackLoadId::new(1))
        .unwrap_or_else(|| panic!("loaded deck should retain transport"));
    assert_eq!(after.beat, 16);
    assert_eq!(after.discontinuity_revision, before.discontinuity_revision);
    assert!(
        provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events should drain: {error}"))
            .is_empty()
    );
}

#[test]
fn bridge_failure_is_visible_and_a_ready_event_clears_it_after_recovery() {
    let mut decoder = BridgeDecoder::new();
    let mut provider = ProLinkDeckSourceProvider::new(MonotonicTime::new(0))
        .unwrap_or_else(|error| panic!("provider should initialize: {error}"));
    provider
        .mark_degraded("bridge exited", MonotonicTime::new(1))
        .unwrap_or_else(|error| panic!("failure should be recorded: {error}"));
    assert_eq!(
        provider.diagnostics().source_status,
        DeckSourceStatus::Degraded
    );
    assert_eq!(
        provider.diagnostics().last_error.as_deref(),
        Some("bridge exited")
    );

    for line in [HELLO, READY] {
        let message = decoder
            .decode_line(line)
            .unwrap_or_else(|error| panic!("recovery fixture should decode: {error}"));
        provider
            .ingest(message, MonotonicTime::new(2))
            .unwrap_or_else(|error| panic!("ready event should recover: {error}"));
    }
    assert_eq!(
        provider.diagnostics().source_status,
        DeckSourceStatus::Ready
    );
    assert_eq!(provider.diagnostics().last_error, None);
}
