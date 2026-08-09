use lumi_prolink_input::{
    BridgeDecodeError, BridgeDecoder, BridgeEvent, PROTOCOL_NAME, PROTOCOL_VERSION,
};

const HELLO: &str = include_str!("../../../../contracts/prolink-bridge/v1/fixtures/hello.json");
const DECK_STATUS: &str =
    include_str!("../../../../contracts/prolink-bridge/v1/fixtures/deck-status.json");

#[test]
fn decodes_versioned_hello_and_rich_deck_status() {
    let mut decoder = BridgeDecoder::new();
    let hello = decoder
        .decode_line(HELLO)
        .unwrap_or_else(|error| panic!("hello fixture must decode: {error}"));
    assert!(matches!(hello.event, BridgeEvent::Hello(_)));

    let status = decoder
        .decode_line(DECK_STATUS)
        .unwrap_or_else(|error| panic!("deck fixture must decode: {error}"));
    let BridgeEvent::DeckStatus(status) = status.event else {
        panic!("expected deck status");
    };
    assert_eq!(status.device_number, 2);
    assert_eq!(status.rekordbox_id, 1842);
    assert_eq!(status.effective_bpm, 136.5);
    assert_eq!(status.beat_number, 169);
    assert!(status.tempo_master);
    assert_eq!(decoder.last_sequence(), Some(2));
}

#[test]
fn fails_closed_before_hello_and_on_sequence_gaps() {
    let mut decoder = BridgeDecoder::new();
    assert!(matches!(
        decoder.decode_line(DECK_STATUS),
        Err(BridgeDecodeError::NonMonotoneSequence { .. }) | Err(BridgeDecodeError::HelloRequired)
    ));

    decoder
        .decode_line(HELLO)
        .unwrap_or_else(|error| panic!("hello fixture must decode: {error}"));
    let skipped = DECK_STATUS.replace("\"sequence\":2", "\"sequence\":3");
    assert!(matches!(
        decoder.decode_line(&skipped),
        Err(BridgeDecodeError::NonMonotoneSequence {
            expected: 2,
            actual: 3
        })
    ));
}

#[test]
fn exposes_owned_protocol_identity() {
    assert_eq!(PROTOCOL_NAME, "lumi-prolink-bridge");
    assert_eq!(PROTOCOL_VERSION, 1);
}
