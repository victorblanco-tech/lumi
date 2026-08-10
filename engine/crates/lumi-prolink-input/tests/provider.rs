use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{DeckObservation, DomainEvent, MonotonicTime};
use lumi_prolink_input::{BridgeDecoder, ProLinkDeckSourceProvider};

const HELLO: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":1,"observedAtNanos":10,"type":"hello","payload":{"bridgeVersion":"0.4.0-dev","beatLinkVersion":"8.0.0","readOnly":true}}"#;
const READY: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":2,"observedAtNanos":20,"type":"sourceStatus","payload":{"status":"ready","detail":"network ready"}}"#;
const STATUS: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":3,"observedAtNanos":30,"type":"deckStatus","payload":{"deviceNumber":1,"deviceName":"LUMI-SIM","playing":true,"paused":false,"cued":false,"tempoMaster":true,"onAir":true,"sourcePlayer":1,"sourceSlot":"USB_SLOT","trackType":"REKORDBOX","rekordboxId":1256,"trackBpm":155.0,"effectiveBpm":157.25,"beatNumber":17,"beatWithinBar":1,"rawPitch":1082458112}}"#;

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
