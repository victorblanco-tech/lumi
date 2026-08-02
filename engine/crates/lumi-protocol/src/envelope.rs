use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Identifies the semantic payload carried by an envelope.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageType {
    Command,
    Snapshot,
    Event,
    Error,
}

/// Transport-independent protocol v1 envelope.
///
/// Application boundaries must map this DTO into their own domain or
/// presentation types.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEnvelope {
    pub protocol_version: u16,
    pub message_type: MessageType,
    pub message_id: String,
    pub sequence: u64,
    pub correlation_id: String,
    pub sent_at: String,
    pub payload: Map<String, Value>,
}
