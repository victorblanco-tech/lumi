use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{DeckObservation, DomainEvent, MonotonicTime};
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
    assert_eq!(beat.effective_bpm_milli, 157_250);
    assert_eq!(beat.beat_within_bar, 2);
    assert!(beat.playing);
}
