//! TLS LAN listener and isolated engine relay for Lumi Remote clients.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumi_remote_protocol::{
    MAX_REMOTE_FRAME_BYTES, REMOTE_PROTOCOL_VERSION, RemoteClientHello, RemoteCommand,
    RemoteCommandResult, RemoteCommandResultStatus, RemoteFrame, RemoteFrameKind,
    RemoteLiveProjection, RemoteServerHello,
};
use lumi_stream::BoundedLineReader;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use serde_json::json;
use thiserror::Error;
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot, watch};
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

use crate::{
    AttemptRateLimiter, CommandAdmission, EngineProjectionClient, EngineRemoteServiceRecord,
    GatewayCommandGuard, InstallationIdentity, PairingError, PairingRegistry, PersistentTrustStore,
    RateLimitError, ReleaseChannel, random_hex,
};

const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const CLIENT_HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const ENGINE_RECONNECT_INTERVAL: Duration = Duration::from_millis(500);
const ENGINE_COMMAND_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_EVENT_CAPACITY: usize = 128;
const ENGINE_COMMAND_CAPACITY: usize = 32;
const MAXIMUM_AUTHENTICATION_ATTEMPTS: usize = 8;
const AUTHENTICATION_RATE_WINDOW_MILLIS: u64 = 60_000;

#[derive(Clone)]
pub struct SharedGatewayState {
    pub registry: Arc<Mutex<PairingRegistry>>,
    pub command_guard: Arc<Mutex<GatewayCommandGuard>>,
    pub trust_store: PersistentTrustStore,
    pub identity: InstallationIdentity,
    trust_revision: watch::Sender<u64>,
    command_limiter: Arc<Mutex<AttemptRateLimiter>>,
}

impl SharedGatewayState {
    pub fn load(
        identity: InstallationIdentity,
        trust_store: PersistentTrustStore,
    ) -> Result<Self, GatewayNetworkError> {
        let registry = trust_store.load()?;
        let mut command_guard = GatewayCommandGuard::new(crate::DEFAULT_COMMAND_LEDGER_CAPACITY)?;
        if let Some(controller_device_id) = registry.controller_device_id() {
            command_guard.transfer_control(controller_device_id.to_owned(), random_hex(24)?);
        }
        Ok(Self {
            registry: Arc::new(Mutex::new(registry)),
            command_guard: Arc::new(Mutex::new(command_guard)),
            trust_store,
            identity,
            trust_revision: watch::channel(0).0,
            command_limiter: Arc::new(Mutex::new(AttemptRateLimiter::new(32, 1_000)?)),
        })
    }

    pub fn notify_trust_changed(&self) {
        let next = self.trust_revision.borrow().saturating_add(1);
        self.trust_revision.send_replace(next);
    }

    fn trust_changes(&self) -> watch::Receiver<u64> {
        self.trust_revision.subscribe()
    }

    /// Always called while holding registry, then guard, in that order. No
    /// in-memory ownership changes become visible unless persistence succeeds.
    pub(crate) async fn commit_registry(
        &self,
        registry: &mut PairingRegistry,
        candidate: PairingRegistry,
        rotate_lease: bool,
    ) -> Result<(), GatewayNetworkError> {
        let mut guard = self.command_guard.lock().await;
        let owner = candidate.controller_device_id();
        let ownership_changed = guard.controller().map(|c| c.device_id.as_str()) != owner;
        let lease = if (ownership_changed || rotate_lease) && owner.is_some() {
            Some(random_hex(24)?)
        } else {
            None
        };
        self.trust_store.save(&candidate)?;
        if ownership_changed || rotate_lease {
            if let (Some(owner), Some(lease)) = (owner, lease) {
                guard.transfer_control(owner.to_owned(), lease);
            } else {
                guard.revoke_control();
            }
        }
        *registry = candidate;
        Ok(())
    }

    async fn authenticate(
        &self,
        hello: RemoteClientHello,
        now: u64,
    ) -> Result<(String, RemoteServerHello), GatewayNetworkError> {
        hello.validate()?;
        let mut registry = self.registry.lock().await;
        let mut candidate = registry.clone();
        let (device_id, paired) = match hello {
            RemoteClientHello::Authenticate {
                device_id,
                credential,
                client_version,
            } => {
                candidate.authenticate(&device_id, &credential, now)?;
                candidate.set_client_version(&device_id, client_version);
                (device_id, false)
            }
            RemoteClientHello::Pair {
                invitation_id,
                invitation_secret,
                device_id,
                display_name,
                device_credential,
                client_version,
            } => {
                candidate.exchange(
                    &invitation_id,
                    &invitation_secret,
                    device_id.clone(),
                    display_name,
                    &device_credential,
                    now,
                )?;
                candidate.set_client_version(&device_id, client_version);
                (device_id, true)
            }
        };
        self.commit_registry(&mut registry, candidate, false)
            .await?;
        let lease = self
            .command_guard
            .lock()
            .await
            .controller_lease_for(&device_id);
        let controller_display_name = registry
            .paired_devices()
            .find(|device| device.controller)
            .map(|device| device.display_name.clone());
        let response = if paired {
            RemoteServerHello::Paired {
                installation_id: self.identity.installation_id.clone(),
                controller_lease_id: lease,
                controller_display_name,
            }
        } else {
            RemoteServerHello::Authenticated {
                installation_id: self.identity.installation_id.clone(),
                controller_lease_id: lease,
                controller_display_name,
            }
        };
        Ok((device_id, response))
    }
}

#[derive(Clone)]
pub struct EngineRelayHandle {
    latest_projection: watch::Receiver<Option<RemoteLiveProjection>>,
    frames: broadcast::Sender<RemoteFrame>,
    commands: mpsc::Sender<EngineCommandRequest>,
    connected: watch::Receiver<bool>,
}

impl EngineRelayHandle {
    pub fn start(
        service_record_path: PathBuf,
        command_guard: Arc<Mutex<GatewayCommandGuard>>,
    ) -> Self {
        let (latest_sender, latest_projection) = watch::channel(None);
        let (frames, _) = broadcast::channel(CLIENT_EVENT_CAPACITY);
        let (commands, command_receiver) = mpsc::channel(ENGINE_COMMAND_CAPACITY);
        let (connected_sender, connected) = watch::channel(false);
        let published_frames = frames.clone();
        tokio::spawn(async move {
            run_engine_relay(
                service_record_path,
                latest_sender,
                published_frames,
                command_receiver,
                connected_sender,
                command_guard,
            )
            .await;
        });
        Self {
            latest_projection,
            frames,
            commands,
            connected,
        }
    }

    pub fn connected(&self) -> bool {
        *self.connected.borrow()
    }

    pub fn latest_projection(&self) -> Option<RemoteLiveProjection> {
        self.latest_projection.borrow().clone()
    }

    async fn submit(&self, command: RemoteCommand) -> Result<(), GatewayNetworkError> {
        if !self.connected() {
            return Err(GatewayNetworkError::EngineUnavailable);
        }
        let (response, receiver) = oneshot::channel();
        self.commands
            .try_send(EngineCommandRequest {
                command,
                response,
                expires_at: tokio::time::Instant::now() + ENGINE_COMMAND_RESPONSE_TIMEOUT,
            })
            .map_err(|_| GatewayNetworkError::EngineUnavailable)?;
        await_engine_command_response(receiver, ENGINE_COMMAND_RESPONSE_TIMEOUT).await
    }
}

async fn await_engine_command_response(
    receiver: oneshot::Receiver<Result<(), GatewayNetworkError>>,
    maximum_duration: Duration,
) -> Result<(), GatewayNetworkError> {
    timeout(maximum_duration, receiver)
        .await
        .map_err(|_| GatewayNetworkError::EngineUnavailable)?
        .map_err(|_| GatewayNetworkError::EngineUnavailable)?
}

struct EngineCommandRequest {
    command: RemoteCommand,
    response: oneshot::Sender<Result<(), GatewayNetworkError>>,
    expires_at: tokio::time::Instant,
}

pub struct GatewayNetworkServer {
    listener: TcpListener,
    tls: TlsAcceptor,
    state: SharedGatewayState,
    relay: EngineRelayHandle,
    client_gate: Arc<tokio::sync::Semaphore>,
    limiter: Arc<Mutex<AttemptRateLimiter>>,
    _advertisement: BonjourAdvertisement,
}

impl GatewayNetworkServer {
    pub async fn bind(
        state: SharedGatewayState,
        relay: EngineRelayHandle,
        release_channel: ReleaseChannel,
        display_name: &str,
        maximum_clients: usize,
    ) -> Result<Self, GatewayNetworkError> {
        if maximum_clients == 0 || maximum_clients > crate::MAX_REMOTE_CLIENTS {
            return Err(GatewayNetworkError::InvalidClientLimit);
        }
        let listener = TcpListener::bind(("0.0.0.0", 0)).await?;
        let port = listener.local_addr()?.port();
        let tls = TlsAcceptor::from(state.identity.tls_server_config()?);
        let advertisement =
            BonjourAdvertisement::register(release_channel, display_name, &state.identity, port)?;
        Ok(Self {
            listener,
            tls,
            state,
            relay,
            client_gate: Arc::new(tokio::sync::Semaphore::new(maximum_clients)),
            limiter: Arc::new(Mutex::new(AttemptRateLimiter::new(
                MAXIMUM_AUTHENTICATION_ATTEMPTS,
                AUTHENTICATION_RATE_WINDOW_MILLIS,
            )?)),
            _advertisement: advertisement,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, GatewayNetworkError> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn run(self) -> Result<(), GatewayNetworkError> {
        loop {
            let (stream, peer) = self.listener.accept().await?;
            let Ok(permit) = self.client_gate.clone().try_acquire_owned() else {
                continue;
            };
            let tls = self.tls.clone();
            let state = self.state.clone();
            let relay = self.relay.clone();
            let limiter = self.limiter.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let _ = serve_tls_client(stream, peer, tls, state, relay, limiter).await;
            });
        }
    }
}

async fn serve_tls_client(
    stream: TcpStream,
    peer: SocketAddr,
    tls: TlsAcceptor,
    state: SharedGatewayState,
    relay: EngineRelayHandle,
    limiter: Arc<Mutex<AttemptRateLimiter>>,
) -> Result<(), GatewayNetworkError> {
    let tls_stream = timeout(TLS_HANDSHAKE_TIMEOUT, tls.accept(stream))
        .await
        .map_err(|_| GatewayNetworkError::HandshakeTimeout)??;
    let (reader, mut writer) = tokio::io::split(tls_stream);
    let mut reader = BoundedLineReader::new(BufReader::new(reader));
    let hello_frame = timeout(CLIENT_HELLO_TIMEOUT, read_frame(&mut reader))
        .await
        .map_err(|_| GatewayNetworkError::AuthenticationTimeout)??
        .ok_or(GatewayNetworkError::AuthenticationEnded)?;
    if hello_frame.frame_kind != RemoteFrameKind::Hello {
        return Err(GatewayNetworkError::UnexpectedFrame);
    }
    let hello: RemoteClientHello = serde_json::from_value(hello_frame.payload)?;
    if limiter
        .lock()
        .await
        .check(&peer.ip().to_string(), rate_limit_millis())
        .is_err()
    {
        let _ = write_error(&mut writer, "rateLimited", 1).await;
        return Err(GatewayNetworkError::RateLimit(RateLimitError::Limited));
    }
    // Subscribe before authentication: a transfer during the Hello/snapshot
    // write must invalidate this session too, not disappear between awaits.
    let mut trust_changes = state.trust_changes();
    let (device_id, server_hello) = match state.authenticate(hello, unix_millis()).await {
        Ok(authenticated) => authenticated,
        Err(error) => {
            if should_rate_limit_authentication_failure(&error)
                && limiter
                    .lock()
                    .await
                    .record(&peer.ip().to_string(), rate_limit_millis())
                    .is_err()
            {
                let _ = write_error(&mut writer, "rateLimited", 1).await;
                return Err(GatewayNetworkError::RateLimit(RateLimitError::Limited));
            }
            let _ = write_error(&mut writer, network_error_code(&error), 1).await;
            return Err(error);
        }
    };

    let mut delivery_sequence = 1_u64;
    write_frame(
        &mut writer,
        &RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Hello,
            sequence: delivery_sequence,
            correlation_id: None,
            payload: serde_json::to_value(server_hello)?,
        },
    )
    .await?;
    delivery_sequence = delivery_sequence.saturating_add(1);

    let mut last_projection_revision = 0_u64;
    let mut updates = relay.frames.subscribe();
    if let Some(snapshot) = relay.latest_projection() {
        last_projection_revision = snapshot.projection_revision;
        write_projection(&mut writer, snapshot, delivery_sequence).await?;
        delivery_sequence = delivery_sequence.saturating_add(1);
    } else {
        write_error(&mut writer, "engineUnavailable", delivery_sequence).await?;
        delivery_sequence = delivery_sequence.saturating_add(1);
    }

    loop {
        tokio::select! {
            changed = trust_changes.changed() => {
                changed.map_err(|_| GatewayNetworkError::AuthenticationEnded)?;
                // Re-authentication is mandatory after revoke or Controller
                // transfer. No old connection may retain a stale grant.
                return Ok(());
            }
            update = updates.recv() => {
                let mut frame = match update {
                    Ok(frame) => frame,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        return Err(GatewayNetworkError::SlowClient);
                    }
                };
                if frame.frame_kind == RemoteFrameKind::CommandResult {
                    let result: RemoteCommandResult = serde_json::from_value(frame.payload)?;
                    let Some(result) = state.command_guard.lock().await
                        .result_for_device(&device_id, &result.command_id) else { continue; };
                    frame.correlation_id = Some(result.command_id.clone());
                    frame.payload = serde_json::to_value(result)?;
                }
                if matches!(frame.frame_kind, RemoteFrameKind::Snapshot | RemoteFrameKind::Projection) {
                    let projection: RemoteLiveProjection = serde_json::from_value(frame.payload.clone())?;
                    if projection.projection_revision <= last_projection_revision {
                        continue;
                    }
                    last_projection_revision = projection.projection_revision;
                }
                frame.sequence = delivery_sequence;
                write_frame(&mut writer, &frame).await?;
                delivery_sequence = delivery_sequence.saturating_add(1);
            }
            incoming = read_frame(&mut reader) => {
                let Some(frame) = incoming? else { return Ok(()); };
                if frame.frame_kind != RemoteFrameKind::Command {
                    return Err(GatewayNetworkError::UnexpectedFrame);
                }
                let command: RemoteCommand = serde_json::from_value(frame.payload)?;
                command.validate().map_err(crate::CommandAuthorizationError::InvalidCommand)
                    .map_err(crate::CommandGuardError::Unauthorized)?;
                if state.command_limiter.lock().await.record(&device_id, rate_limit_millis()).is_err() {
                    write_command_result(&mut writer, RemoteCommandResult {
                        command_id: command.command_id,
                        status: RemoteCommandResultStatus::Rejected,
                        state_revision: None, plan_revision: None,
                        reason_code: Some("rateLimited".into()),
                    }, delivery_sequence).await?;
                    delivery_sequence = delivery_sequence.saturating_add(1);
                    continue;
                }
                let admission = state.command_guard.lock().await.admit(&device_id, &command)?;
                if let CommandAdmission::Completed(result) = admission {
                    write_command_result(&mut writer, result, delivery_sequence).await?;
                    delivery_sequence = delivery_sequence.saturating_add(1);
                    continue;
                }
                if admission == CommandAdmission::Pending {
                    write_command_result(
                        &mut writer,
                        RemoteCommandResult {
                            command_id: command.command_id,
                            status: RemoteCommandResultStatus::Rejected,
                            state_revision: None,
                            plan_revision: None,
                            reason_code: Some("commandOutcomePending".to_owned()),
                        },
                        delivery_sequence,
                    ).await?;
                    delivery_sequence = delivery_sequence.saturating_add(1);
                    continue;
                }
                if matches!(command.command, lumi_remote_protocol::RemoteCommandKind::RequestSnapshot) {
                    if let Some(snapshot) = relay.latest_projection() {
                        last_projection_revision = snapshot.projection_revision;
                        write_projection(&mut writer, snapshot, delivery_sequence).await?;
                        delivery_sequence = delivery_sequence.saturating_add(1);
                    }
                    continue;
                }
                let forwarded = state.command_guard.lock().await.forwarded_command(&device_id, &command);
                if let Err(error) = relay.submit(forwarded.clone()).await {
                    // A failed transport may have written part/all of a frame.
                    // Retain the unresolved ledger entry: never resend blindly.
                    write_command_result(
                        &mut writer,
                        RemoteCommandResult {
                            command_id: command.command_id,
                            status: RemoteCommandResultStatus::Rejected,
                            state_revision: None,
                            plan_revision: None,
                            reason_code: Some("commandOutcomeUnknown".to_owned()),
                        },
                        delivery_sequence,
                    ).await?;
                    delivery_sequence = delivery_sequence.saturating_add(1);
                    if !matches!(error, GatewayNetworkError::EngineUnavailable) {
                        return Err(error);
                    }
                }
            }
        }
    }
}

async fn run_engine_relay(
    service_record_path: PathBuf,
    latest: watch::Sender<Option<RemoteLiveProjection>>,
    frames: broadcast::Sender<RemoteFrame>,
    mut commands: mpsc::Receiver<EngineCommandRequest>,
    connected: watch::Sender<bool>,
    command_guard: Arc<Mutex<GatewayCommandGuard>>,
) {
    loop {
        let record = EngineRemoteServiceRecord::read(&service_record_path);
        let client = match record {
            Ok(record) => match timeout(
                ENGINE_COMMAND_RESPONSE_TIMEOUT,
                EngineProjectionClient::connect(&record),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    wait_before_engine_retry(&mut commands).await;
                    continue;
                }
            },
            Err(_) => {
                wait_before_engine_retry(&mut commands).await;
                continue;
            }
        };
        let mut client = match client {
            Ok(client) => client,
            Err(_) => {
                wait_before_engine_retry(&mut commands).await;
                continue;
            }
        };
        connected.send_replace(true);
        let mut reconnect = false;
        while !reconnect {
            tokio::select! {
                frame = client.next_frame() => {
                    match frame {
                        Ok(Some(frame)) => {
                            if frame.frame_kind == RemoteFrameKind::CommandResult {
                                if let Ok(result) = serde_json::from_value::<RemoteCommandResult>(frame.payload.clone()) {
                                    command_guard.lock().await.record_result(result);
                                } else { reconnect = true; continue; }
                            }
                            if matches!(frame.frame_kind, RemoteFrameKind::Snapshot | RemoteFrameKind::Projection) {
                                match serde_json::from_value::<RemoteLiveProjection>(frame.payload.clone()) {
                                    Ok(projection) if projection.validate().is_ok() => {
                                        latest.send_replace(Some(projection));
                                    }
                                    _ => {
                                        reconnect = true;
                                        continue;
                                    }
                                }
                            } else if frame.frame_kind == RemoteFrameKind::Error {
                                latest.send_replace(None);
                            }
                            let _ = frames.send(frame);
                        }
                        _ => reconnect = true,
                    }
                }
                request = commands.recv() => {
                    let Some(request) = request else { return; };
                    if request.response.is_closed() { continue; }
                    if tokio::time::Instant::now() >= request.expires_at ||
                        command_guard.lock().await.authorize_forwarded(&request.command).is_err() {
                        command_guard.lock().await.record_result(RemoteCommandResult {
                            command_id: request.command.command_id,
                            status: RemoteCommandResultStatus::Rejected,
                            state_revision: None, plan_revision: None,
                            reason_code: Some("commandExpiredOrControlRevoked".to_owned()),
                        });
                        let _ = request.response.send(Err(GatewayNetworkError::EngineUnavailable));
                        continue;
                    }
                    let outcome = tokio::time::timeout_at(request.expires_at, client.send_command(&request.command))
                        .await.map_err(|_| GatewayNetworkError::EngineUnavailable)
                        .and_then(|result| result.map_err(|_| GatewayNetworkError::EngineUnavailable));
                    let failed = outcome.is_err();
                    let _ = request.response.send(outcome);
                    if failed { reconnect = true; }
                }
            }
        }
        connected.send_replace(false);
        latest.send_replace(None);
        let _ = frames.send(RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Error,
            sequence: 1,
            correlation_id: None,
            payload: json!({"reasonCode": "engineUnavailable"}),
        });
    }
}

async fn wait_before_engine_retry(commands: &mut mpsc::Receiver<EngineCommandRequest>) {
    tokio::select! {
        _ = tokio::time::sleep(ENGINE_RECONNECT_INTERVAL) => {}
        request = commands.recv() => {
            if let Some(request) = request {
                let _ = request.response.send(Err(GatewayNetworkError::EngineUnavailable));
            }
        }
    }
}

async fn read_frame<R>(
    reader: &mut BoundedLineReader<R>,
) -> Result<Option<RemoteFrame>, GatewayNetworkError>
where
    R: AsyncBufRead + Unpin,
{
    let bytes = reader
        .next_line(MAX_REMOTE_FRAME_BYTES)
        .await
        .map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidData {
                GatewayNetworkError::OversizedFrame
            } else {
                GatewayNetworkError::Io(error)
            }
        })?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    Ok(Some(RemoteFrame::decode(&bytes)?))
}

async fn write_frame<W>(writer: &mut W, frame: &RemoteFrame) -> Result<(), GatewayNetworkError>
where
    W: AsyncWrite + Unpin,
{
    write_frame_with_timeout(writer, frame, CLIENT_WRITE_TIMEOUT).await
}

async fn write_frame_with_timeout<W>(
    writer: &mut W,
    frame: &RemoteFrame,
    maximum_duration: Duration,
) -> Result<(), GatewayNetworkError>
where
    W: AsyncWrite + Unpin,
{
    let mut bytes = frame.encode()?;
    bytes.push(b'\n');
    timeout(maximum_duration, async {
        writer.write_all(&bytes).await?;
        writer.flush().await
    })
    .await
    .map_err(|_| GatewayNetworkError::SlowClient)??;
    Ok(())
}

async fn write_projection<W>(
    writer: &mut W,
    projection: RemoteLiveProjection,
    sequence: u64,
) -> Result<(), GatewayNetworkError>
where
    W: AsyncWrite + Unpin,
{
    write_frame(
        writer,
        &RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Snapshot,
            sequence,
            correlation_id: None,
            payload: serde_json::to_value(projection)?,
        },
    )
    .await
}

async fn write_error<W>(
    writer: &mut W,
    reason: &str,
    sequence: u64,
) -> Result<(), GatewayNetworkError>
where
    W: AsyncWrite + Unpin,
{
    write_frame(
        writer,
        &RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Error,
            sequence,
            correlation_id: None,
            payload: json!({"reasonCode": reason}),
        },
    )
    .await
}

async fn write_command_result<W>(
    writer: &mut W,
    result: RemoteCommandResult,
    sequence: u64,
) -> Result<(), GatewayNetworkError>
where
    W: AsyncWrite + Unpin,
{
    write_frame(
        writer,
        &RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::CommandResult,
            sequence,
            correlation_id: Some(result.command_id.clone()),
            payload: serde_json::to_value(result)?,
        },
    )
    .await
}

struct BonjourAdvertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl BonjourAdvertisement {
    fn register(
        release_channel: ReleaseChannel,
        display_name: &str,
        identity: &InstallationIdentity,
        port: u16,
    ) -> Result<Self, GatewayNetworkError> {
        let service_type = format!("{}.local.", release_channel.bonjour_service_type());
        let properties: HashMap<String, String> = release_channel
            .discovery_metadata(&identity.installation_id, port)?
            .into_iter()
            .collect();
        let info = ServiceInfo::new(
            &service_type,
            display_name,
            &format!("{}.", identity.server_name),
            "",
            port,
            properties,
        )?
        .enable_addr_auto();
        let fullname = info.get_fullname().to_owned();
        let daemon = ServiceDaemon::new()?;
        daemon.register(info)?;
        Ok(Self { daemon, fullname })
    }
}

impl Drop for BonjourAdvertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

fn rate_limit_millis() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn network_error_code(error: &GatewayNetworkError) -> &'static str {
    match error {
        GatewayNetworkError::Pairing(PairingError::ApprovalRequired) => "approvalRequired",
        GatewayNetworkError::Pairing(PairingError::InvitationExpired) => "invitationExpired",
        GatewayNetworkError::Pairing(PairingError::InvitationUnknown) => "invitationUnknown",
        GatewayNetworkError::Pairing(PairingError::DeviceUnknown) => "deviceRevoked",
        GatewayNetworkError::Pairing(PairingError::CredentialMismatch) => "credentialMismatch",
        _ => "authenticationRejected",
    }
}

fn should_rate_limit_authentication_failure(error: &GatewayNetworkError) -> bool {
    !matches!(
        error,
        GatewayNetworkError::Pairing(PairingError::ApprovalRequired)
    )
}

#[derive(Debug, Error)]
pub enum GatewayNetworkError {
    #[error("remote client limit is invalid")]
    InvalidClientLimit,
    #[error("remote TLS handshake timed out")]
    HandshakeTimeout,
    #[error("remote authentication timed out")]
    AuthenticationTimeout,
    #[error("remote connection ended before authentication")]
    AuthenticationEnded,
    #[error("remote client sent an unexpected frame")]
    UnexpectedFrame,
    #[error("remote client frame exceeds its bounded size")]
    OversizedFrame,
    #[error("remote client fell behind the bounded projection stream")]
    SlowClient,
    #[error("Lumi engine Remote projection is unavailable")]
    EngineUnavailable,
    #[error("remote network I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("remote TLS handshake failed: {0}")]
    Tls(#[from] tokio_rustls::rustls::Error),
    #[error("remote JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("remote frame validation failed: {0}")]
    Frame(#[from] lumi_remote_protocol::RemoteFrameError),
    #[error("remote client authentication contract failed: {0}")]
    Authentication(#[from] lumi_remote_protocol::RemoteAuthenticationError),
    #[error("remote pairing failed: {0}")]
    Pairing(#[from] PairingError),
    #[error("remote command was rejected: {0}")]
    Command(#[from] crate::CommandGuardError),
    #[error("remote attempt was rate limited: {0}")]
    RateLimit(#[from] RateLimitError),
    #[error("remote gateway configuration failed: {0}")]
    Configuration(#[from] crate::GatewayConfigError),
    #[error("remote identity failed: {0}")]
    Identity(#[from] crate::IdentityError),
    #[error("remote trust store failed: {0}")]
    TrustStore(#[from] crate::TrustStoreError),
    #[error("Bonjour failed: {0}")]
    Bonjour(#[from] mdns_sd::Error),
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod controller_tests;

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::io;
    use std::sync::Arc;
    use std::time::Duration;

    use lumi_remote_protocol::{
        MAX_REMOTE_FRAME_BYTES, REMOTE_PROTOCOL_VERSION, RemoteClientHello, RemoteFrame,
        RemoteFrameKind, RemoteServerHello,
    };
    use tokio::io::{AsyncWriteExt as _, BufReader, duplex};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_rustls::TlsConnector;
    use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};

    use crate::{
        EngineRelayHandle, InstallationIdentity, PairingInvitationRequest, PairingRegistry,
        PersistentTrustStore, SharedGatewayState, random_hex,
    };

    use super::{
        GatewayNetworkError, await_engine_command_response, read_frame, serve_tls_client,
        write_frame_with_timeout,
    };

    #[tokio::test]
    async fn production_relay_records_engine_outcome_without_a_connected_phone()
    -> Result<(), Box<dyn Error>> {
        use crate::{CommandAdmission, GatewayCommandGuard};
        use lumi_remote_protocol::{
            RemoteCommand, RemoteCommandKind, RemoteCommandResult, RemoteCommandResultStatus,
        };
        use std::os::unix::fs::PermissionsExt as _;
        use tokio::sync::{Mutex, broadcast, mpsc, oneshot, watch};
        let directory =
            std::env::temp_dir().join(format!("lumi-relay-outcome-{}", random_hex(12)?));
        fs::create_dir_all(&directory)?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let record_path = directory.join("engine.json");
        fs::write(
            &record_path,
            serde_json::to_vec(&serde_json::json!({
                "remoteGatewayEndpoint": { "host": "127.0.0.1", "port": listener.local_addr()?.port(), "protocolVersion": 1 },
                "remoteGatewayToken": "a".repeat(64), "processID": 42, "productVersion": "0.6.2-test"
            }))?,
        )?;
        fs::set_permissions(&record_path, fs::Permissions::from_mode(0o600))?;
        let guard = Arc::new(Mutex::new(GatewayCommandGuard::new(64)?));
        let command = RemoteCommand {
            command_id: "same-client-id".into(),
            controller_lease_id: "lease".into(),
            issued_at_unix_millis: 1,
            command: RemoteCommandKind::SetOutputTimingOffset {
                millis: -20,
                expected_state_revision: 7,
            },
        };
        guard
            .lock()
            .await
            .transfer_control("phone".into(), "lease".into());
        assert_eq!(
            guard.lock().await.admit("phone", &command)?,
            CommandAdmission::Accepted
        );
        let forwarded = guard.lock().await.forwarded_command("phone", &command);
        let expected = forwarded.clone();
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let (reader, mut writer) = stream.into_split();
            let mut reader = lumi_stream::BoundedLineReader::new(BufReader::new(reader));
            let _authentication = reader.next_line(512).await?;
            let frame = read_frame(&mut reader)
                .await?
                .ok_or(io::Error::other("missing command"))?;
            let received: RemoteCommand = serde_json::from_value(frame.payload)?;
            assert_eq!(received, expected);
            super::write_command_result(
                &mut writer,
                RemoteCommandResult {
                    command_id: received.command_id,
                    status: RemoteCommandResultStatus::Conflict,
                    state_revision: Some(9),
                    plan_revision: None,
                    reason_code: Some("stateRevisionConflict".into()),
                },
                1,
            )
            .await?;
            Ok::<(), GatewayNetworkError>(())
        });
        let (latest, _) = watch::channel(None);
        let (frames, mut results) = broadcast::channel(8);
        let (commands, receiver) = mpsc::channel(8);
        let (connected, mut connection) = watch::channel(false);
        let relay = tokio::spawn(super::run_engine_relay(
            record_path,
            latest,
            frames,
            receiver,
            connected,
            guard.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(2), async {
            while !*connection.borrow() {
                connection.changed().await?;
            }
            Ok::<(), tokio::sync::watch::error::RecvError>(())
        })
        .await??;
        let (response, sent) = oneshot::channel();
        commands
            .send(super::EngineCommandRequest {
                command: forwarded.clone(),
                response,
                expires_at: tokio::time::Instant::now() + Duration::from_secs(2),
            })
            .await?;
        sent.await??;
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if results.recv().await?.frame_kind == RemoteFrameKind::CommandResult {
                    break;
                }
            }
            Ok::<(), broadcast::error::RecvError>(())
        })
        .await??;
        peer.await??;
        let replay = guard
            .lock()
            .await
            .result_for_device("phone", &forwarded.command_id)
            .ok_or(io::Error::other("engine result was not retained"))?;
        assert_eq!(replay.command_id, command.command_id);
        assert_eq!(replay.status, RemoteCommandResultStatus::Conflict);
        assert_eq!(replay.state_revision, Some(9));
        assert_eq!(
            guard.lock().await.admit("phone", &command)?,
            CommandAdmission::Completed(replay)
        );
        relay.abort();
        let _ = relay.await;
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[tokio::test]
    async fn bounded_reader_rejects_oversized_and_malformed_network_frames() {
        let mut oversized = vec![b' '; MAX_REMOTE_FRAME_BYTES + 1];
        oversized.push(b'\n');
        let mut oversized_reader =
            lumi_stream::BoundedLineReader::new(BufReader::new(oversized.as_slice()));
        assert!(matches!(
            read_frame(&mut oversized_reader).await,
            Err(GatewayNetworkError::OversizedFrame)
        ));

        let malformed = b"{not-json}\n";
        let mut malformed_reader =
            lumi_stream::BoundedLineReader::new(BufReader::new(malformed.as_slice()));
        assert!(matches!(
            read_frame(&mut malformed_reader).await,
            Err(GatewayNetworkError::Frame(_))
        ));
    }

    #[tokio::test]
    async fn a_client_that_stops_reading_is_evicted_by_the_write_deadline() {
        let (_reader, mut writer) = duplex(16);
        let frame = RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Error,
            sequence: 1,
            correlation_id: None,
            payload: serde_json::json!({"detail": "x".repeat(8_192)}),
        };

        assert!(matches!(
            write_frame_with_timeout(&mut writer, &frame, Duration::from_millis(20)).await,
            Err(GatewayNetworkError::SlowClient)
        ));
    }

    #[tokio::test]
    async fn a_stalled_engine_command_cannot_hold_a_remote_client_forever() {
        let (_sender, receiver) = tokio::sync::oneshot::channel();

        assert!(matches!(
            await_engine_command_response(receiver, Duration::from_millis(20)).await,
            Err(GatewayNetworkError::EngineUnavailable)
        ));
    }

    #[tokio::test]
    async fn pinned_tls_authenticates_a_persisted_device_and_grants_one_controller()
    -> Result<(), Box<dyn Error>> {
        let suffix = random_hex(8)?;
        let directory = std::env::temp_dir().join(format!(
            "lumi-remote-tls-test-{}-{}",
            std::process::id(),
            suffix
        ));
        let identity = InstallationIdentity::load_or_create(&directory.join("identity"))?;
        let trust_store = PersistentTrustStore::new(directory.join("trust.json"));
        let credential = random_hex(32)?;
        let invitation_secret = random_hex(32)?;
        let mut registry = PairingRegistry::default();
        registry.create_invitation(PairingInvitationRequest {
            invitation_id: "invitation-123456".to_owned(),
            invitation_secret: invitation_secret.clone(),
            short_code: "123456".to_owned(),
            certificate_fingerprint_sha256: identity.certificate_fingerprint_sha256.clone(),
            created_at_unix_millis: 10,
            expires_at_unix_millis: 1_000,
        })?;
        registry.approve("invitation-123456", "123456")?;
        registry.exchange(
            "invitation-123456",
            &invitation_secret,
            "iphone-1".to_owned(),
            "Test iPhone".to_owned(),
            &credential,
            20,
        )?;
        trust_store.save(&registry)?;
        let state = SharedGatewayState::load(identity.clone(), trust_store)?;
        let relay = EngineRelayHandle::start(
            directory.join("missing-engine-service.json"),
            state.command_guard.clone(),
        );
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let acceptor = tokio_rustls::TlsAcceptor::from(identity.tls_server_config()?);
        let server = tokio::spawn(async move {
            let (stream, peer) = listener.accept().await?;
            let limiter = crate::AttemptRateLimiter::new(8, 60_000)?;
            serve_tls_client(
                stream,
                peer,
                acceptor,
                state,
                relay,
                Arc::new(tokio::sync::Mutex::new(limiter)),
            )
            .await
        });

        let mut roots = RootCertStore::empty();
        roots.add(CertificateDer::from(identity.certificate_der.clone()))?;
        let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let stream = TcpStream::connect(address).await?;
        let server_name = ServerName::try_from(identity.server_name.clone())?;
        let stream = connector.connect(server_name, stream).await?;
        let (reader, mut writer) = tokio::io::split(stream);
        let hello = RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Hello,
            sequence: 1,
            correlation_id: None,
            payload: serde_json::to_value(RemoteClientHello::Authenticate {
                device_id: "iphone-1".to_owned(),
                credential,
                client_version: None,
            })?,
        };
        let mut bytes = hello.encode()?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;
        let mut reader = lumi_stream::BoundedLineReader::new(BufReader::new(reader));
        let response = read_frame(&mut reader)
            .await?
            .ok_or_else(|| io::Error::other("TLS server omitted authentication response"))?;
        let hello: RemoteServerHello = serde_json::from_value(response.payload)?;
        assert!(matches!(
            hello,
            RemoteServerHello::Authenticated {
                controller_lease_id: Some(_),
                ..
            }
        ));
        drop(reader);
        drop(writer);
        // Authentication is the contract under test. Cancelling the isolated
        // server task avoids treating the client's normal TLS teardown as a
        // platform-dependent connection-reset failure.
        server.abort();
        let _ = server.await;
        let _ = fs::remove_dir_all(directory);
        Ok(())
    }
}
