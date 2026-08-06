use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::MessageEnvelope;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_MESSAGE_BYTES: usize = 131_072;
const MAX_IDENTIFIER_BYTES: usize = 128;

/// Safely decodes one complete protocol message.
pub struct MessageDecoder;

impl MessageDecoder {
    /// Validates size, JSON shape, required identifiers, and protocol version.
    pub fn decode(input: &[u8]) -> Result<MessageEnvelope, DecodeError> {
        if input.len() > MAX_MESSAGE_BYTES {
            return Err(DecodeError::Oversized {
                actual: input.len(),
                maximum: MAX_MESSAGE_BYTES,
            });
        }

        let envelope: MessageEnvelope =
            serde_json::from_slice(input).map_err(|_| DecodeError::Malformed)?;

        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(DecodeError::UnsupportedProtocolVersion(
                envelope.protocol_version,
            ));
        }

        validate_identifier("messageId", &envelope.message_id)?;
        validate_identifier("correlationId", &envelope.correlation_id)?;

        if envelope.sent_at.is_empty() {
            return Err(DecodeError::InvalidField("sentAt"));
        }

        Ok(envelope)
    }
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), DecodeError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return Err(DecodeError::InvalidField(field));
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Oversized { actual: usize, maximum: usize },
    Malformed,
    UnsupportedProtocolVersion(u16),
    InvalidField(&'static str),
}

impl Display for DecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oversized { actual, maximum } => {
                write!(
                    formatter,
                    "message has {actual} bytes; maximum is {maximum}"
                )
            }
            Self::Malformed => formatter.write_str("message is not a valid protocol envelope"),
            Self::UnsupportedProtocolVersion(version) => {
                write!(formatter, "protocol version {version} is unsupported")
            }
            Self::InvalidField(field) => write!(formatter, "field {field} is invalid"),
        }
    }
}

impl Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::{DecodeError, MAX_MESSAGE_BYTES, MessageDecoder};

    #[test]
    fn rejects_oversized_input_before_json_decoding() {
        let input = vec![b' '; MAX_MESSAGE_BYTES + 1];
        let result = MessageDecoder::decode(&input);

        assert_eq!(
            result,
            Err(DecodeError::Oversized {
                actual: MAX_MESSAGE_BYTES + 1,
                maximum: MAX_MESSAGE_BYTES,
            })
        );
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(
            MessageDecoder::decode(br#"{"protocolVersion":1"#),
            Err(DecodeError::Malformed)
        );
    }
}
