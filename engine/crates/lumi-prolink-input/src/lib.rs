//! Versioned process boundary for the supervised Lumi Pro DJ Link bridge.
//!
//! Beat Link and Java types stop in the helper process. This crate accepts
//! only complete protocol envelopes and exposes immutable Lumi-owned facts to
//! the future deck-source adapter.

#![forbid(unsafe_code)]

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

mod provider;
mod supervisor;

pub use provider::{
    ProLinkDeckSourceDiagnostics, ProLinkDeckSourceProvider, ProLinkDiscoveredDevice,
    ProLinkProviderError, ProLinkTimingObservation, ProLinkTrackIdentity, ProLinkTransportSnapshot,
};
pub use supervisor::{
    BridgeLaunchConfiguration, BridgeProcessDiagnostics, BridgeProcessSupervisor,
    BridgeSupervisorError, PRO_DJ_LINK_UDP_PORTS, ProLinkNetworkConflict,
    ensure_prolink_network_available,
};

pub const PROTOCOL_NAME: &str = "lumi-prolink-bridge";
pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct BridgeMessage {
    pub sequence: u64,
    pub observed_at_nanos: u64,
    pub event: BridgeEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BridgeEvent {
    Hello(Hello),
    SourceStatus(SourceStatus),
    DeviceFound(Device),
    DeviceLost(Device),
    DeckStatus(DeckStatus),
    Beat(Beat),
    TrackMetadata(TrackMetadata),
    TrackSignature(TrackSignature),
    Error(BridgeFailure),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hello {
    pub bridge_version: String,
    pub beat_link_version: String,
    pub read_only: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SourceCondition {
    Starting,
    Discovering,
    Ready,
    Degraded,
    Stopped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceStatus {
    pub status: SourceCondition,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Device {
    pub device_number: u8,
    pub device_name: String,
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeckStatus {
    pub device_number: u8,
    pub device_name: String,
    pub playing: bool,
    pub paused: bool,
    pub cued: bool,
    pub tempo_master: bool,
    pub on_air: bool,
    pub source_player: u8,
    pub source_slot: String,
    pub track_type: String,
    pub rekordbox_id: u32,
    pub track_bpm: f64,
    pub effective_bpm: f64,
    pub beat_number: u32,
    pub beat_within_bar: u8,
    pub raw_pitch: i32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Beat {
    pub device_number: u8,
    pub device_name: String,
    pub effective_bpm: f64,
    pub beat_within_bar: u8,
    pub tempo_master: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackMetadata {
    pub deck_number: u8,
    pub available: bool,
    pub source_player: Option<u8>,
    pub source_slot: Option<String>,
    pub track_type: Option<String>,
    pub rekordbox_id: Option<u32>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_seconds: Option<u32>,
    pub track_bpm: Option<f64>,
    pub musical_key: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackSignature {
    pub deck_number: u8,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeFailure {
    pub operation: String,
    pub message: String,
}

#[derive(Default)]
pub struct BridgeDecoder {
    last_sequence: Option<u64>,
    hello_received: bool,
}

impl BridgeDecoder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_sequence: None,
            hello_received: false,
        }
    }

    pub fn decode_line(&mut self, line: &str) -> Result<BridgeMessage, BridgeDecodeError> {
        let envelope: WireEnvelope = serde_json::from_str(line)?;
        if envelope.protocol != PROTOCOL_NAME {
            return Err(BridgeDecodeError::UnexpectedProtocol(envelope.protocol));
        }
        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(BridgeDecodeError::UnsupportedVersion(
                envelope.protocol_version,
            ));
        }
        let expected = self
            .last_sequence
            .map_or(1, |sequence| sequence.saturating_add(1));
        if envelope.sequence != expected {
            return Err(BridgeDecodeError::NonMonotoneSequence {
                expected,
                actual: envelope.sequence,
            });
        }
        if !self.hello_received && envelope.message_type != "hello" {
            return Err(BridgeDecodeError::HelloRequired);
        }

        let event = decode_event(&envelope.message_type, envelope.payload)?;
        validate_event(&event)?;
        if matches!(event, BridgeEvent::Hello(_)) {
            if self.hello_received {
                return Err(BridgeDecodeError::DuplicateHello);
            }
            self.hello_received = true;
        }
        self.last_sequence = Some(envelope.sequence);
        Ok(BridgeMessage {
            sequence: envelope.sequence,
            observed_at_nanos: envelope.observed_at_nanos,
            event,
        })
    }

    #[must_use]
    pub const fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireEnvelope {
    protocol: String,
    protocol_version: u16,
    sequence: u64,
    observed_at_nanos: u64,
    #[serde(rename = "type")]
    message_type: String,
    payload: Value,
}

fn decode_event(message_type: &str, payload: Value) -> Result<BridgeEvent, BridgeDecodeError> {
    match message_type {
        "hello" => decode_payload(payload).map(BridgeEvent::Hello),
        "sourceStatus" => decode_payload(payload).map(BridgeEvent::SourceStatus),
        "deviceFound" => decode_payload(payload).map(BridgeEvent::DeviceFound),
        "deviceLost" => decode_payload(payload).map(BridgeEvent::DeviceLost),
        "deckStatus" => decode_payload(payload).map(BridgeEvent::DeckStatus),
        "beat" => decode_payload(payload).map(BridgeEvent::Beat),
        "trackMetadata" => decode_payload(payload).map(BridgeEvent::TrackMetadata),
        "trackSignature" => decode_payload(payload).map(BridgeEvent::TrackSignature),
        "error" => decode_payload(payload).map(BridgeEvent::Error),
        _ => Err(BridgeDecodeError::UnknownEventType(message_type.to_owned())),
    }
}

fn decode_payload<T: for<'de> Deserialize<'de>>(payload: Value) -> Result<T, BridgeDecodeError> {
    serde_json::from_value(payload).map_err(BridgeDecodeError::InvalidJson)
}

fn validate_event(event: &BridgeEvent) -> Result<(), BridgeDecodeError> {
    match event {
        BridgeEvent::Hello(hello) => {
            if hello.bridge_version.trim().is_empty()
                || hello.beat_link_version.trim().is_empty()
                || !hello.read_only
            {
                return Err(BridgeDecodeError::InvalidPayload("hello"));
            }
        }
        BridgeEvent::DeviceFound(device) | BridgeEvent::DeviceLost(device) => {
            validate_device(device)?;
        }
        BridgeEvent::DeckStatus(status) => {
            let has_track_id = status.rekordbox_id != 0;
            let has_source_player = status.source_player != 0;
            let loaded_track_is_invalid = has_track_id
                && (!valid_bpm(status.track_bpm)
                    || !valid_bpm(status.effective_bpm)
                    || status.source_slot.trim().is_empty()
                    || status.track_type.trim().is_empty());
            if status.device_number == 0
                || (has_track_id && !has_source_player)
                || loaded_track_is_invalid
                || status.beat_within_bar > 4
            {
                return Err(BridgeDecodeError::InvalidPayload("deckStatus"));
            }
        }
        BridgeEvent::Beat(beat) => {
            if beat.device_number == 0
                || !valid_bpm(beat.effective_bpm)
                || !(1..=4).contains(&beat.beat_within_bar)
            {
                return Err(BridgeDecodeError::InvalidPayload("beat"));
            }
        }
        BridgeEvent::TrackMetadata(metadata) => {
            if metadata.deck_number == 0
                || (metadata.available
                    && (metadata.rekordbox_id.is_none()
                        || metadata.duration_seconds.is_none()
                        || metadata.track_bpm.is_none()))
            {
                return Err(BridgeDecodeError::InvalidPayload("trackMetadata"));
            }
        }
        BridgeEvent::TrackSignature(signature) => {
            if signature.deck_number == 0
                || signature.signature.as_deref().is_some_and(|value| {
                    value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            {
                return Err(BridgeDecodeError::InvalidPayload("trackSignature"));
            }
        }
        BridgeEvent::SourceStatus(status) => {
            if status.detail.trim().is_empty() {
                return Err(BridgeDecodeError::InvalidPayload("sourceStatus"));
            }
        }
        BridgeEvent::Error(failure) => {
            if failure.operation.trim().is_empty() || failure.message.trim().is_empty() {
                return Err(BridgeDecodeError::InvalidPayload("error"));
            }
        }
    }
    Ok(())
}

fn validate_device(device: &Device) -> Result<(), BridgeDecodeError> {
    if device.device_number == 0
        || device.device_name.trim().is_empty()
        || device.address.trim().is_empty()
    {
        return Err(BridgeDecodeError::InvalidPayload("device"));
    }
    Ok(())
}

fn valid_bpm(value: f64) -> bool {
    value.is_finite() && (20.0..=300.0).contains(&value)
}

#[derive(Debug, Error)]
pub enum BridgeDecodeError {
    #[error("invalid Pro DJ Link bridge JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unexpected bridge protocol '{0}'")]
    UnexpectedProtocol(String),
    #[error("unsupported Pro DJ Link bridge protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("bridge sequence mismatch: expected {expected}, got {actual}")]
    NonMonotoneSequence { expected: u64, actual: u64 },
    #[error("the first bridge envelope must be hello")]
    HelloRequired,
    #[error("the bridge emitted hello more than once")]
    DuplicateHello,
    #[error("unknown bridge event type '{0}'")]
    UnknownEventType(String),
    #[error("invalid bridge payload for {0}")]
    InvalidPayload(&'static str),
}
