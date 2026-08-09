//! Versioned, transport-independent Lumi wire protocol.

#![forbid(unsafe_code)]

mod decoder;
mod envelope;
mod idempotency;

pub use decoder::{DecodeError, MAX_MESSAGE_BYTES, MessageDecoder, PROTOCOL_VERSION};
pub use envelope::{MessageEnvelope, MessageType};
pub use idempotency::{CommandDisposition, CommandIdCache, InvalidCacheCapacity};
