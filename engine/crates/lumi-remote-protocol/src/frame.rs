use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const REMOTE_PROTOCOL_VERSION: u16 = 1;
pub const MAX_REMOTE_FRAME_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteFrameKind {
    Hello,
    Snapshot,
    Projection,
    TransportAnchor,
    Command,
    CommandResult,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFrame {
    pub protocol_version: u16,
    pub frame_kind: RemoteFrameKind,
    pub sequence: u64,
    pub correlation_id: Option<String>,
    pub payload: Value,
}

impl RemoteFrame {
    pub fn decode(bytes: &[u8]) -> Result<Self, RemoteFrameError> {
        if bytes.len() > MAX_REMOTE_FRAME_BYTES {
            return Err(RemoteFrameError::Oversized);
        }
        let frame: Self = serde_json::from_slice(bytes).map_err(RemoteFrameError::InvalidJson)?;
        if frame.protocol_version != REMOTE_PROTOCOL_VERSION {
            return Err(RemoteFrameError::UnsupportedProtocol(
                frame.protocol_version,
            ));
        }
        if frame.sequence == 0 {
            return Err(RemoteFrameError::InvalidSequence);
        }
        if frame
            .correlation_id
            .as_ref()
            .is_some_and(|value| value.len() > 128 || value.chars().any(char::is_control))
        {
            return Err(RemoteFrameError::InvalidCorrelationId);
        }
        Ok(frame)
    }

    pub fn encode(&self) -> Result<Vec<u8>, RemoteFrameError> {
        if self.protocol_version != REMOTE_PROTOCOL_VERSION {
            return Err(RemoteFrameError::UnsupportedProtocol(self.protocol_version));
        }
        if self.sequence == 0 {
            return Err(RemoteFrameError::InvalidSequence);
        }
        let bytes = serde_json::to_vec(self).map_err(RemoteFrameError::InvalidJson)?;
        if bytes.len() > MAX_REMOTE_FRAME_BYTES {
            return Err(RemoteFrameError::Oversized);
        }
        Ok(bytes)
    }
}

#[derive(Debug, Error)]
pub enum RemoteFrameError {
    #[error("remote frame exceeds the bounded frame size")]
    Oversized,
    #[error("remote frame is invalid JSON: {0}")]
    InvalidJson(serde_json::Error),
    #[error("remote protocol version {0} is unsupported")]
    UnsupportedProtocol(u16),
    #[error("correlation ID is oversized or contains control characters")]
    InvalidCorrelationId,
    #[error("remote frame sequence must be non-zero")]
    InvalidSequence,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        MAX_REMOTE_FRAME_BYTES, REMOTE_PROTOCOL_VERSION, RemoteFrame, RemoteFrameError,
        RemoteFrameKind,
    };

    #[test]
    fn round_trips_a_bounded_frame() -> Result<(), RemoteFrameError> {
        let frame = RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Hello,
            sequence: 1,
            correlation_id: None,
            payload: json!({ "releaseChannel": "dev" }),
        };
        let encoded = frame.encode()?;
        assert_eq!(RemoteFrame::decode(&encoded)?, frame);
        Ok(())
    }

    #[test]
    fn rejects_oversized_input_before_parsing() {
        let encoded = vec![b' '; MAX_REMOTE_FRAME_BYTES + 1];
        assert!(matches!(
            RemoteFrame::decode(&encoded),
            Err(RemoteFrameError::Oversized)
        ));
    }

    #[test]
    fn rejects_a_zero_delivery_sequence() {
        let frame = RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Hello,
            sequence: 0,
            correlation_id: None,
            payload: json!({}),
        };
        assert!(matches!(
            frame.encode(),
            Err(RemoteFrameError::InvalidSequence)
        ));
    }
}
