use serde_json::Value;

use crate::{
    RemoteCommand, RemoteCommandResult, RemoteFrame, RemoteFrameKind, RemoteLiveProjection,
};

#[test]
fn shared_snapshot_fixture_decodes_and_validates() {
    let frame = RemoteFrame::decode(include_bytes!(
        "../../../../contracts/remote/v1/fixtures/snapshot-live.json"
    ))
    .unwrap_or_else(|error| panic!("shared snapshot frame must decode: {error}"));
    assert_eq!(frame.frame_kind, RemoteFrameKind::Snapshot);
    let projection: RemoteLiveProjection = serde_json::from_value(frame.payload)
        .unwrap_or_else(|error| panic!("shared snapshot projection must decode: {error}"));
    projection
        .validate()
        .unwrap_or_else(|error| panic!("shared snapshot projection must validate: {error}"));
}

#[test]
fn shared_command_and_result_fixtures_match_the_allowlist() {
    let command_frame = RemoteFrame::decode(include_bytes!(
        "../../../../contracts/remote/v1/fixtures/command-autoloop.json"
    ))
    .unwrap_or_else(|error| panic!("shared command frame must decode: {error}"));
    let command: RemoteCommand = serde_json::from_value(command_frame.payload)
        .unwrap_or_else(|error| panic!("shared command must decode: {error}"));
    command
        .validate()
        .unwrap_or_else(|error| panic!("shared command must validate: {error}"));

    let result_frame = RemoteFrame::decode(include_bytes!(
        "../../../../contracts/remote/v1/fixtures/command-result-conflict.json"
    ))
    .unwrap_or_else(|error| panic!("shared result frame must decode: {error}"));
    let result: RemoteCommandResult = serde_json::from_value(result_frame.payload)
        .unwrap_or_else(|error| panic!("shared result must decode: {error}"));
    result
        .validate()
        .unwrap_or_else(|error| panic!("shared result must validate: {error}"));
}

#[test]
fn manifest_keeps_the_remote_scope_explicit() {
    let manifest: Value = serde_json::from_slice(include_bytes!(
        "../../../../contracts/remote/v1/manifest.json"
    ))
    .unwrap_or_else(|error| panic!("remote manifest must be JSON: {error}"));
    assert_eq!(manifest["protocolVersion"], 1);
    assert_eq!(manifest["maximumFrameBytes"], 512 * 1_024);
    let encoded = manifest.to_string();
    for excluded in ["library", "usb", "audioUri", "filesystemPath"] {
        assert!(encoded.contains(excluded));
    }
}
