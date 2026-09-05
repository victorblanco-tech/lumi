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
use lumi_stream::BoundedLineReader;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncWriteExt as _, BufReader};
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
    reader: BoundedLineReader<BufReader<OwnedReadHalf>>,
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
            reader: BoundedLineReader::new(BufReader::new(reader)),
            writer,
            next_command_sequence: 1,
        })
    }

    pub async fn next_frame(&mut self) -> Result<Option<RemoteFrame>, EngineClientError> {
        let Some(encoded) = self.reader.next_line(MAX_REMOTE_FRAME_BYTES).await? else {
            return Ok(None);
        };
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

    #[tokio::test]
    async fn production_client_preserves_a_frame_when_command_work_cancels_its_read()
    -> Result<(), Box<dyn std::error::Error>> {
        use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
        use tokio::net::TcpListener;
        use tokio::sync::oneshot;
        use tokio::time::{Duration, timeout};
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let (resume, paused) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let (reader, mut writer) = stream.into_split();
            BufReader::new(reader)
                .read_until(b'\n', &mut Vec::new())
                .await?;
            writer
                .write_all(b"{\"protocolVersion\":1,\"frameKind\":")
                .await?;
            paused.await.map_err(std::io::Error::other)?;
            writer
                .write_all(b"\"error\",\"sequence\":1,\"payload\":{\"reasonCode\":\"test\"}}\n")
                .await?;
            Ok::<(), std::io::Error>(())
        });
        let record = EngineRemoteServiceRecord {
            remote_gateway_endpoint: EngineRemoteEndpoint {
                host: "127.0.0.1".into(),
                port,
                protocol_version: 1,
            },
            remote_gateway_token: "a".repeat(64),
            process_id: 42,
            product_version: "test".into(),
        };
        let mut client = super::EngineProjectionClient::connect(&record).await?;
        assert!(
            timeout(Duration::from_millis(30), client.next_frame())
                .await
                .is_err()
        );
        resume
            .send(())
            .map_err(|_| std::io::Error::other("server ended"))?;
        let frame = timeout(Duration::from_secs(1), client.next_frame())
            .await??
            .ok_or_else(|| std::io::Error::other("missing frame"))?;
        assert_eq!(frame.payload["reasonCode"], "test");
        server.await??;
        Ok(())
    }
}
