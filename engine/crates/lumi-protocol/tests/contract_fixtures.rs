use std::fs;
use std::path::PathBuf;

use lumi_protocol::{
    DecodeError, MAX_MESSAGE_BYTES, MessageDecoder, MessageType, PROTOCOL_VERSION,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractManifest {
    protocol_version: u16,
    max_message_bytes: usize,
    canonical_fixtures: Vec<String>,
}

fn contract_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/protocol/v1")
}

fn load_manifest() -> ContractManifest {
    let path = contract_directory().join("manifest.json");
    let Ok(bytes) = fs::read(&path) else {
        panic!("could not read {}", path.display());
    };
    let Ok(manifest) = serde_json::from_slice(&bytes) else {
        panic!("could not decode {}", path.display());
    };
    manifest
}

#[test]
fn decodes_all_canonical_contract_fixtures() {
    let manifest = load_manifest();

    assert_eq!(manifest.protocol_version, PROTOCOL_VERSION);
    assert_eq!(manifest.max_message_bytes, MAX_MESSAGE_BYTES);

    for fixture_name in manifest.canonical_fixtures {
        let path = contract_directory().join("fixtures").join(&fixture_name);
        let Ok(bytes) = fs::read(&path) else {
            panic!("could not read {}", path.display());
        };
        let Ok(envelope) = MessageDecoder::decode(&bytes) else {
            panic!("could not decode {}", path.display());
        };
        assert_eq!(envelope.protocol_version, PROTOCOL_VERSION);
    }
}

#[test]
fn ignores_new_optional_fields_within_v1() {
    let path = contract_directory()
        .join("fixtures")
        .join("event-forward-compatible.json");
    let Ok(bytes) = fs::read(&path) else {
        panic!("could not read {}", path.display());
    };
    let Ok(envelope) = MessageDecoder::decode(&bytes) else {
        panic!("could not decode {}", path.display());
    };

    assert_eq!(envelope.message_type, MessageType::Event);
}

#[test]
fn rejects_unsupported_protocol_version() {
    let input = br#"{
        "protocolVersion":2,
        "messageType":"command",
        "messageId":"command-1",
        "sequence":1,
        "correlationId":"interaction-1",
        "sentAt":"2026-08-02T18:00:00Z",
        "payload":{}
    }"#;

    assert_eq!(
        MessageDecoder::decode(input),
        Err(DecodeError::UnsupportedProtocolVersion(2))
    );
}
