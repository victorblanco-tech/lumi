//! Authenticated loopback client for the engine's scoped Remote projection.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::net::IpAddr;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use lumi_remote_protocol::{
    MAX_REMOTE_FRAME_BYTES, REMOTE_PROTOCOL_VERSION, RemoteCommand, RemoteFrame, RemoteFrameKind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

const MAXIMUM_SERVICE_RECORD_BYTES: u64 = 16 * 1_024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EngineRemoteEndpoint {
    pub host: String,
    pub port: u16,
    pub protocol_version: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EngineRemoteServiceRecord {
    pub remote_gateway_endpoint: EngineRemoteEndpoint,
    pub remote_gateway_token: String,
    #[serde(rename = "processID")]
    pub process_id: u32,
    pub product_version: String,
}

impl EngineRemoteServiceRecord {
    pub fn read(path: &Path) -> Result<Self, EngineClientError> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(EngineClientError::UntrustedServiceRecord);
        }
        if metadata.len() > MAXIMUM_SERVICE_RECORD_BYTES
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(EngineClientError::UntrustedServiceRecord);
        }
        let bytes = fs::read(path)?;
        let record: Self = serde_json::from_slice(&bytes)?;
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), EngineClientError> {
        let host: IpAddr = self
            .remote_gateway_endpoint
            .host
            .parse()
            .map_err(|_| EngineClientError::EngineMustRemainLoopback)?;
        if !host.is_loopback() {
            return Err(EngineClientError::EngineMustRemainLoopback);
        }
        if self.remote_gateway_endpoint.port == 0
            || self.remote_gateway_endpoint.protocol_version != REMOTE_PROTOCOL_VERSION
            || !(32..=256).contains(&self.remote_gateway_token.len())
            || self.process_id <= 1
            || self.product_version.is_empty()
        {
            return Err(EngineClientError::InvalidServiceRecord);
        }
        Ok(())
    }
}

pub struct EngineProjectionClient {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
    next_command_sequence: u64,
}

impl EngineProjectionClient {
    pub async fn connect(record: &EngineRemoteServiceRecord) -> Result<Self, EngineClientError> {
        record.validate()?;
        let stream = TcpStream::connect((
            record.remote_gateway_endpoint.host.as_str(),
            record.remote_gateway_endpoint.port,
        ))
        .await?;
        let (reader, mut writer) = stream.into_split();
        let authentication = EngineAuthentication {
            remote_gateway_token: &record.remote_gateway_token,
        };
        let mut encoded = serde_json::to_vec(&authentication)?;
        encoded.push(b'\n');
        writer.write_all(&encoded).await?;
        writer.flush().await?;
        Ok(Self {
            reader: BufReader::new(reader),
            writer,
            next_command_sequence: 1,
        })
    }

    pub async fn next_frame(&mut self) -> Result<Option<RemoteFrame>, EngineClientError> {
        let mut encoded = Vec::with_capacity(4_096);
        let read = self.reader.read_until(b'\n', &mut encoded).await?;
        if read == 0 {
            return Ok(None);
        }
        if encoded.len() > MAX_REMOTE_FRAME_BYTES.saturating_add(1) {
            return Err(EngineClientError::OversizedFrame);
        }
        if encoded.last() == Some(&b'\n') {
            encoded.pop();
        }
        Ok(Some(RemoteFrame::decode(&encoded)?))
    }

    pub async fn send_command(&mut self, command: &RemoteCommand) -> Result<(), EngineClientError> {
        command.validate()?;
        let frame = RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Command,
            sequence: self.next_command_sequence,
            correlation_id: Some(command.command_id.clone()),
            payload: serde_json::to_value(command)?,
        };
        self.next_command_sequence = self.next_command_sequence.saturating_add(1);
        let mut encoded = frame.encode()?;
        encoded.push(b'\n');
        self.writer.write_all(&encoded).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineAuthentication<'a> {
    remote_gateway_token: &'a str,
}

#[derive(Debug, Error)]
pub enum EngineClientError {
    #[error("engine service record is not a protected regular file")]
    UntrustedServiceRecord,
    #[error("engine Remote endpoint must remain loopback-only")]
    EngineMustRemainLoopback,
    #[error("engine Remote service record is invalid")]
    InvalidServiceRecord,
    #[error("engine Remote frame exceeds its bounded size")]
    OversizedFrame,
    #[error("engine Remote I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("engine Remote JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("engine Remote frame failed validation: {0}")]
    Frame(#[from] lumi_remote_protocol::RemoteFrameError),
    #[error("engine Remote command failed validation: {0}")]
    Command(#[from] lumi_remote_protocol::RemoteCommandError),
}

#[cfg(test)]
mod tests {
    use super::{EngineClientError, EngineRemoteEndpoint, EngineRemoteServiceRecord};

    #[test]
    fn rejects_a_non_loopback_engine_endpoint() {
        let record = EngineRemoteServiceRecord {
            remote_gateway_endpoint: EngineRemoteEndpoint {
                host: "192.168.1.10".to_owned(),
                port: 17_000,
                protocol_version: 1,
            },
            remote_gateway_token: "a".repeat(64),
            process_id: 42,
            product_version: "0.6.0-dev-4".to_owned(),
        };
        assert!(matches!(
            record.validate(),
            Err(EngineClientError::EngineMustRemainLoopback)
        ));
    }
}
