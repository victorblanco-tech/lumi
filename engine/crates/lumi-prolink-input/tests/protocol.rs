use lumi_prolink_input::{
    BridgeDecodeError, BridgeDecoder, BridgeEvent, PROTOCOL_NAME, PROTOCOL_VERSION,
};

const HELLO: &str = include_str!("../../../../contracts/prolink-bridge/v1/fixtures/hello.json");
const DECK_STATUS: &str =
    include_str!("../../../../contracts/prolink-bridge/v1/fixtures/deck-status.json");
const PRECISE_POSITION: &str =
    include_str!("../../../../contracts/prolink-bridge/v1/fixtures/precise-position.json");

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
fn decodes_modern_player_precise_position() {
    let mut decoder = BridgeDecoder::new();
    decoder
        .decode_line(HELLO)
        .unwrap_or_else(|error| panic!("hello fixture must decode: {error}"));
    decoder
        .decode_line(DECK_STATUS)
        .unwrap_or_else(|error| panic!("deck fixture must decode: {error}"));
    let message = decoder
        .decode_line(PRECISE_POSITION)
        .unwrap_or_else(|error| panic!("precise position must decode: {error}"));
    let BridgeEvent::PrecisePosition(position) = message.event else {
        panic!("expected precise position");
    };
    assert_eq!(position.device_number, 2);
    assert_eq!(position.playback_position_millis, 42_750);
    assert_eq!(position.effective_bpm, 136.5);
    assert_eq!(position.beat_within_bar, 1);
    assert!(position.tempo_master);
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

#[test]
fn accepts_loaded_pre_roll_and_empty_deck_status() {
    let mut decoder = BridgeDecoder::new();
    decoder
        .decode_line(HELLO)
        .unwrap_or_else(|error| panic!("hello fixture must decode: {error}"));

    let pre_roll = DECK_STATUS
        .replace("\"beatNumber\":169", "\"beatNumber\":0")
        .replace("\"beatWithinBar\":1", "\"beatWithinBar\":0");
    decoder
        .decode_line(&pre_roll)
        .unwrap_or_else(|error| panic!("loaded pre-roll status must decode: {error}"));

    let empty = pre_roll
        .replace("\"sequence\":2", "\"sequence\":3")
        .replace("\"sourcePlayer\":2", "\"sourcePlayer\":0")
        .replace("\"rekordboxId\":1842", "\"rekordboxId\":0")
        .replace("\"trackBpm\":130.0", "\"trackBpm\":655.35")
        .replace("\"effectiveBpm\":136.5", "\"effectiveBpm\":0.0");
    decoder
        .decode_line(&empty)
        .unwrap_or_else(|error| panic!("empty deck status must decode: {error}"));

    // A physical CDJ-1500X retains its own player number while briefly
    // publishing NO_TRACK and rekordboxId 0. This is an unloaded deck, not an
    // invalid half-loaded identity.
    let physical_empty = empty
        .replace("\"sequence\":3", "\"sequence\":4")
        .replace("\"sourcePlayer\":0", "\"sourcePlayer\":2")
        .replace("\"sourceSlot\":\"USB_SLOT\"", "\"sourceSlot\":\"NO_TRACK\"")
        .replace("\"trackType\":\"REKORDBOX\"", "\"trackType\":\"NO_TRACK\"");
    decoder
        .decode_line(&physical_empty)
        .unwrap_or_else(|error| panic!("physical empty-deck status must decode: {error}"));
}

#[test]
fn accepts_physical_cdj_unloaded_sentinel_observed_on_the_wire() {
    let mut decoder = BridgeDecoder::new();
    decoder
        .decode_line(HELLO)
        .unwrap_or_else(|error| panic!("hello fixture must decode: {error}"));

    let unloaded = DECK_STATUS
        .replace("\"sourcePlayer\":2", "\"sourcePlayer\":0")
        .replace("\"sourceSlot\":\"USB_SLOT\"", "\"sourceSlot\":\"NO_TRACK\"")
        .replace("\"trackType\":\"REKORDBOX\"", "\"trackType\":\"NO_TRACK\"")
        .replace("\"rekordboxId\":1842", "\"rekordboxId\":0")
        .replace("\"trackBpm\":130.0", "\"trackBpm\":655.35")
        .replace("\"effectiveBpm\":136.5", "\"effectiveBpm\":655.35")
        .replace("\"beatNumber\":169", "\"beatNumber\":-1")
        .replace("\"beatWithinBar\":1", "\"beatWithinBar\":0");

    let message = decoder
        .decode_line(&unloaded)
        .unwrap_or_else(|error| panic!("real CDJ unloaded sentinel must decode: {error}"));
    let BridgeEvent::DeckStatus(status) = message.event else {
        panic!("expected deck status");
    };
    assert_eq!(status.rekordbox_id, 0);
    assert_eq!(status.beat_number, -1);
    assert_eq!(status.track_bpm, 655.35);
    assert_eq!(status.effective_bpm, 655.35);
}
