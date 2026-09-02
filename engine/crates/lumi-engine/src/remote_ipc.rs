//! Private loopback boundary between the autonomous engine and Remote Gateway.
//!
//! This endpoint is intentionally distinct from the desktop control socket:
//! gateway disconnects never park the show and gateway traffic never receives
//! the desktop session credential or Library payloads.

#![forbid(unsafe_code)]

use std::io;
use std::time::Duration;

use lumi_remote_protocol::{
    REMOTE_PROTOCOL_VERSION, RemoteCommand, RemoteCommandKind, RemoteCommandResult, RemoteFrame,
    RemoteFrameKind, RemoteLiveProjection, RemoteTransportAnchor,
};
use serde::Deserialize;
use serde_json::json;
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::timeout;

const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_RESULT_TIMEOUT: Duration = Duration::from_secs(2);
const MAXIMUM_AUTHENTICATION_BYTES: usize = 512;
const MAXIMUM_GATEWAY_CONNECTIONS: usize = 1;

#[derive(Clone, Debug)]
pub(crate) enum EngineRemoteUpdate {
    Projection(Box<RemoteLiveProjection>),
    TransportAnchor {
        player_number: u8,
        anchor: RemoteTransportAnchor,
    },
    Unavailable,
}

#[derive(Debug)]
pub(crate) struct RemoteCommandRequest {
    pub command: RemoteCommand,
    pub response: oneshot::Sender<RemoteCommandResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewayAuthentication {
    remote_gateway_token: String,
}

pub(crate) async fn serve(
    listener: TcpListener,
    expected_token: String,
    latest_projection: watch::Receiver<Option<RemoteLiveProjection>>,
    updates: broadcast::Sender<EngineRemoteUpdate>,
    command_sender: mpsc::Sender<RemoteCommandRequest>,
) {
    let connection_gate =
        std::sync::Arc::new(tokio::sync::Semaphore::new(MAXIMUM_GATEWAY_CONNECTIONS));
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            return;
        };
        if !peer.ip().is_loopback() {
            continue;
        }
        let Ok(permit) = connection_gate.clone().try_acquire_owned() else {
            continue;
        };
        let token = expected_token.clone();
        let projection = latest_projection.clone();
        let receiver = updates.subscribe();
        let commands = command_sender.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _ = serve_client(stream, &token, projection, receiver, commands).await;
        });
    }
}

async fn serve_client(
    stream: TcpStream,
    expected_token: &str,
    latest_projection: watch::Receiver<Option<RemoteLiveProjection>>,
    mut updates: broadcast::Receiver<EngineRemoteUpdate>,
    command_sender: mpsc::Sender<RemoteCommandRequest>,
) -> Result<(), RemoteIpcError> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::with_capacity(256);
    timeout(
        AUTHENTICATION_TIMEOUT,
        read_bounded_line(&mut reader, &mut line, MAXIMUM_AUTHENTICATION_BYTES),
    )
    .await
    .map_err(|_| RemoteIpcError::AuthenticationTimeout)??;
    let authentication: GatewayAuthentication =
        serde_json::from_slice(&line).map_err(|_| RemoteIpcError::InvalidAuthentication)?;
    if !tokens_match(expected_token, &authentication.remote_gateway_token) {
        return Err(RemoteIpcError::AuthenticationRejected);
    }

    let mut sequence = 1_u64;
    let mut last_projection_revision = 0_u64;
    let bootstrap_projection = latest_projection.borrow().clone();
    if let Some(projection) = bootstrap_projection {
        last_projection_revision = projection.projection_revision;
        write_frame(
            &mut writer,
            &projection_frame(sequence, RemoteFrameKind::Snapshot, projection)?,
        )
        .await?;
        sequence = sequence.saturating_add(1);
    } else {
        write_frame(&mut writer, &unavailable_frame(sequence)).await?;
        sequence = sequence.saturating_add(1);
    }

    loop {
        line.clear();
        tokio::select! {
            update = updates.recv() => {
                let update = match update {
                    Ok(update) => update,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // A slow gateway must reconnect for one authoritative
                        // snapshot; it may never continue across a state gap.
                        return Err(RemoteIpcError::SlowConsumer);
                    }
                };
                let frame = match update {
                    EngineRemoteUpdate::Projection(projection) => {
                        if projection.projection_revision <= last_projection_revision {
                            continue;
                        }
                        last_projection_revision = projection.projection_revision;
                        projection_frame(sequence, RemoteFrameKind::Projection, *projection)?
                    }
                    EngineRemoteUpdate::TransportAnchor { player_number, anchor } => RemoteFrame {
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                        frame_kind: RemoteFrameKind::TransportAnchor,
                        sequence,
                        correlation_id: None,
                        payload: json!({
                            "playerNumber": player_number,
                            "anchor": anchor,
                        }),
                    },
                    EngineRemoteUpdate::Unavailable => unavailable_frame(sequence),
                };
                write_frame(&mut writer, &frame).await?;
                sequence = sequence.saturating_add(1);
            }
            read = read_optional_bounded_line(
                &mut reader,
                &mut line,
                lumi_remote_protocol::MAX_REMOTE_FRAME_BYTES,
            ) => {
                if !read? {
                    return Ok(());
                }
                let frame = RemoteFrame::decode(&line)?;
                if frame.frame_kind != RemoteFrameKind::Command {
                    return Err(RemoteIpcError::UnexpectedFrame);
                }
                let command: RemoteCommand = serde_json::from_value(frame.payload)
                    .map_err(RemoteIpcError::InvalidCommand)?;
                command.validate()?;
                if matches!(command.command, RemoteCommandKind::RequestSnapshot) {
                    let requested_projection = latest_projection.borrow().clone();
                    if let Some(projection) = requested_projection {
                        last_projection_revision = projection.projection_revision;
                        write_frame(
                            &mut writer,
                            &projection_frame(sequence, RemoteFrameKind::Snapshot, projection)?,
                        ).await?;
                        sequence = sequence.saturating_add(1);
                    }
                    continue;
                }
                let (response, response_receiver) = oneshot::channel();
                command_sender.try_send(RemoteCommandRequest {
                    command,
                    response,
                }).map_err(|_| RemoteIpcError::CommandQueueUnavailable)?;
                let result = timeout(COMMAND_RESULT_TIMEOUT, response_receiver)
                    .await
                    .map_err(|_| RemoteIpcError::CommandResultTimeout)?
                    .map_err(|_| RemoteIpcError::CommandQueueUnavailable)?;
                let correlation_id = Some(result.command_id.clone());
                write_frame(&mut writer, &RemoteFrame {
                    protocol_version: REMOTE_PROTOCOL_VERSION,
                    frame_kind: RemoteFrameKind::CommandResult,
                    sequence,
                    correlation_id,
                    payload: serde_json::to_value(result)?,
                }).await?;
                sequence = sequence.saturating_add(1);
            }
        }
    }
}

fn projection_frame(
    sequence: u64,
    frame_kind: RemoteFrameKind,
    projection: RemoteLiveProjection,
) -> Result<RemoteFrame, RemoteIpcError> {
    Ok(RemoteFrame {
        protocol_version: REMOTE_PROTOCOL_VERSION,
        frame_kind,
        sequence,
        correlation_id: None,
        payload: serde_json::to_value(projection)?,
    })
}

fn unavailable_frame(sequence: u64) -> RemoteFrame {
    RemoteFrame {
        protocol_version: REMOTE_PROTOCOL_VERSION,
        frame_kind: RemoteFrameKind::Error,
        sequence,
        correlation_id: None,
        payload: json!({
            "reasonCode": "connectedPlayersUnavailable",
            "message": "Lumi Remote becomes available when Pro DJ Link is selected."
        }),
    }
}

async fn write_frame<W>(writer: &mut W, frame: &RemoteFrame) -> Result<(), RemoteIpcError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut encoded = frame.encode()?;
    encoded.push(b'\n');
    writer.write_all(&encoded).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_bounded_line<R>(
    reader: &mut R,
    bytes: &mut Vec<u8>,
    maximum: usize,
) -> Result<(), RemoteIpcError>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    if !read_optional_bounded_line(reader, bytes, maximum).await? {
        return Err(RemoteIpcError::UnexpectedEnd);
    }
    Ok(())
}

async fn read_optional_bounded_line<R>(
    reader: &mut R,
    bytes: &mut Vec<u8>,
    maximum: usize,
) -> Result<bool, RemoteIpcError>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let read = reader.read_until(b'\n', bytes).await?;
    if read == 0 {
        return Ok(false);
    }
    if bytes.len() > maximum.saturating_add(1) {
        return Err(RemoteIpcError::Oversized);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(true)
}

fn tokens_match(expected: &str, received: &str) -> bool {
    expected.len() == received.len() && bool::from(expected.as_bytes().ct_eq(received.as_bytes()))
}

#[derive(Debug, Error)]
enum RemoteIpcError {
    #[error("gateway authentication timed out")]
    AuthenticationTimeout,
    #[error("gateway authentication is invalid")]
    InvalidAuthentication,
    #[error("gateway authentication was rejected")]
    AuthenticationRejected,
    #[error("gateway connection ended before authentication")]
    UnexpectedEnd,
    #[error("gateway frame exceeds the bounded size")]
    Oversized,
    #[error("gateway sent an unexpected frame kind")]
    UnexpectedFrame,
    #[error("gateway command is invalid: {0}")]
    InvalidCommand(serde_json::Error),
    #[error("gateway command queue is unavailable")]
    CommandQueueUnavailable,
    #[error("gateway command result timed out")]
    CommandResultTimeout,
    #[error("gateway fell behind the bounded engine stream")]
    SlowConsumer,
    #[error("gateway I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("gateway frame failed validation: {0}")]
    Frame(#[from] lumi_remote_protocol::RemoteFrameError),
    #[error("gateway command failed validation: {0}")]
    Command(#[from] lumi_remote_protocol::RemoteCommandError),
    #[error("gateway JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;
    use std::net::Ipv4Addr;

    use lumi_remote_protocol::{
        IntegrationHealth, OperationState, REMOTE_PROTOCOL_VERSION, RemoteCommand,
        RemoteCommandKind, RemoteCommandResult, RemoteCommandResultStatus, RemoteFrame,
        RemoteFrameKind, RemoteIntegrationStatus, RemoteLiveProjection,
    };
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::{broadcast, mpsc, watch};
    use tokio::time::{Duration, timeout};

    use super::{EngineRemoteUpdate, RemoteCommandRequest, serve};

    fn projection() -> RemoteLiveProjection {
        RemoteLiveProjection {
            projection_revision: 1,
            state_revision: 7,
            engine_version: "0.6.0-dev-4".to_owned(),
            operation_state: OperationState::Armed,
            leader_player_number: None,
            integrations: RemoteIntegrationStatus {
                pro_dj_link: IntegrationHealth::Starting,
                light_output: IntegrationHealth::Ready,
                ableton_link: IntegrationHealth::Unavailable,
                ableton_link_enabled: false,
                ableton_link_bpm_milli: None,
                timing_offset_millis: 0,
                pending_timing_offset_millis: None,
            },
            players: Vec::new(),
            live_plan: None,
            next_plan: None,
            theme_options: Vec::new(),
        }
    }

    async fn test_endpoint(
        token: &str,
    ) -> io::Result<(
        std::net::SocketAddr,
        tokio::task::JoinHandle<()>,
        mpsc::Receiver<RemoteCommandRequest>,
    )> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let (_latest_sender, latest_receiver) = watch::channel(Some(projection()));
        let (updates, _) = broadcast::channel::<EngineRemoteUpdate>(8);
        let (command_sender, command_receiver) = mpsc::channel(4);
        let task = tokio::spawn(serve(
            listener,
            token.to_owned(),
            latest_receiver,
            updates,
            command_sender,
        ));
        Ok((address, task, command_receiver))
    }

    #[tokio::test]
    async fn authenticates_streams_snapshot_and_round_trips_command_result()
    -> Result<(), Box<dyn Error>> {
        let token = "a".repeat(64);
        let (address, task, mut commands) = test_endpoint(&token).await?;
        let stream = TcpStream::connect(address).await?;
        let (reader, mut writer) = stream.into_split();
        writer
            .write_all(format!("{{\"remoteGatewayToken\":\"{token}\"}}\n").as_bytes())
            .await?;
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line).await?;
        let snapshot_bytes = line
            .strip_suffix(b"\n")
            .ok_or_else(|| io::Error::other("snapshot omitted newline delimiter"))?;
        let snapshot = RemoteFrame::decode(snapshot_bytes)?;
        assert_eq!(snapshot.frame_kind, RemoteFrameKind::Snapshot);

        let command = RemoteCommand {
            command_id: "remote-command-1".to_owned(),
            controller_lease_id: "controller-lease-1".to_owned(),
            issued_at_unix_millis: 10,
            command: RemoteCommandKind::SetAbletonLinkEnabled {
                enabled: true,
                expected_state_revision: 7,
            },
        };
        let frame = RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Command,
            sequence: 1,
            correlation_id: Some(command.command_id.clone()),
            payload: serde_json::to_value(command)?,
        };
        let mut bytes = frame.encode()?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;

        let request = timeout(Duration::from_secs(1), commands.recv())
            .await?
            .ok_or_else(|| io::Error::other("command channel closed before request"))?;
        request
            .response
            .send(RemoteCommandResult {
                command_id: request.command.command_id,
                status: RemoteCommandResultStatus::Accepted,
                state_revision: Some(8),
                plan_revision: None,
                reason_code: None,
            })
            .map_err(|_| io::Error::other("gateway dropped command-result receiver"))?;

        line.clear();
        reader.read_until(b'\n', &mut line).await?;
        let result_bytes = line
            .strip_suffix(b"\n")
            .ok_or_else(|| io::Error::other("command result omitted newline delimiter"))?;
        let result = RemoteFrame::decode(result_bytes)?;
        assert_eq!(result.frame_kind, RemoteFrameKind::CommandResult);
        assert_eq!(result.correlation_id.as_deref(), Some("remote-command-1"));
        task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn rejects_an_incorrect_gateway_token_without_streaming_state()
    -> Result<(), Box<dyn Error>> {
        let (address, task, _commands) = test_endpoint(&"a".repeat(64)).await?;
        let mut stream = TcpStream::connect(address).await?;
        stream
            .write_all(b"{\"remoteGatewayToken\":\"wrong\"}\n")
            .await?;
        let mut reader = BufReader::new(stream);
        let mut line = Vec::new();
        let received = timeout(Duration::from_secs(1), reader.read_until(b'\n', &mut line)).await?;
        match received {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            Ok(_) => return Err(io::Error::other("rejected gateway streamed state").into()),
            Err(error) => return Err(error.into()),
        }
        task.abort();
        Ok(())
    }
}
