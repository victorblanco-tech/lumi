//! Protected loopback administration surface for the foreground Mac app.

#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::{
    EngineRelayHandle, MAX_INVITATION_LIFETIME_MILLIS, PairingInvitationRequest,
    SharedGatewayState, random_hex,
};

const ADMIN_AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAXIMUM_ADMIN_LINE_BYTES: usize = 64 * 1_024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAdminServiceRecord {
    pub endpoint_host: String,
    pub endpoint_port: u16,
    pub admin_token: String,
    #[serde(rename = "processID")]
    pub process_id: u32,
    pub product_version: String,
    pub installation_id: String,
    pub certificate_fingerprint_sha256: String,
    pub lan_port: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum GatewayAdminRequest {
    Status,
    CreateInvitation,
    ApproveInvitation {
        invitation_id: String,
        short_code: String,
    },
    RevokeDevice {
        device_id: String,
    },
    TransferControl {
        device_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAdminResponse {
    pub ok: bool,
    pub status: GatewayAdminStatus,
    pub invitation: Option<GatewayPairingInvitation>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAdminStatus {
    pub engine_connected: bool,
    pub installation_id: String,
    pub certificate_fingerprint_sha256: String,
    pub lan_port: u16,
    pub paired_devices: Vec<GatewayPairedDeviceStatus>,
    pub controller_device_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPairedDeviceStatus {
    pub device_id: String,
    pub display_name: String,
    pub paired_at_unix_millis: u64,
    pub last_seen_unix_millis: u64,
    pub controller: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPairingInvitation {
    pub installation_id: String,
    pub invitation_id: String,
    pub invitation_secret: String,
    pub short_code: String,
    pub certificate_fingerprint_sha256: String,
    pub expires_at_unix_millis: u64,
    pub approved: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminAuthentication {
    admin_token: String,
}

pub struct GatewayAdminServer {
    listener: TcpListener,
    state: SharedGatewayState,
    relay: EngineRelayHandle,
    token: String,
    lan_port: u16,
}

impl GatewayAdminServer {
    pub async fn bind(
        state: SharedGatewayState,
        relay: EngineRelayHandle,
        record_path: &Path,
        product_version: String,
        lan_port: u16,
    ) -> Result<(Self, GatewayAdminRecordGuard), GatewayAdminError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let endpoint = listener.local_addr()?;
        let token = random_hex(32)?;
        let record = GatewayAdminServiceRecord {
            endpoint_host: "127.0.0.1".to_owned(),
            endpoint_port: endpoint.port(),
            admin_token: token.clone(),
            process_id: std::process::id(),
            product_version,
            installation_id: state.identity.installation_id.clone(),
            certificate_fingerprint_sha256: state.identity.certificate_fingerprint_sha256.clone(),
            lan_port,
        };
        write_protected_record(record_path, &record)?;
        Ok((
            Self {
                listener,
                state,
                relay,
                token,
                lan_port,
            },
            GatewayAdminRecordGuard {
                path: record_path.to_owned(),
                process_id: std::process::id(),
            },
        ))
    }

    pub async fn run(self) -> Result<(), GatewayAdminError> {
        loop {
            let (stream, peer) = self.listener.accept().await?;
            if !peer.ip().is_loopback() {
                continue;
            }
            let state = self.state.clone();
            let relay = self.relay.clone();
            let token = self.token.clone();
            let lan_port = self.lan_port;
            tokio::spawn(async move {
                let _ = serve_admin_client(stream, state, relay, token, lan_port).await;
            });
        }
    }
}

pub struct GatewayAdminRecordGuard {
    path: PathBuf,
    process_id: u32,
}

impl Drop for GatewayAdminRecordGuard {
    fn drop(&mut self) {
        let belongs_to_process = fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<GatewayAdminServiceRecord>(&bytes).ok())
            .is_some_and(|record| record.process_id == self.process_id);
        if belongs_to_process {
            let _ = fs::remove_file(&self.path);
        }
    }
}

async fn serve_admin_client(
    stream: TcpStream,
    state: SharedGatewayState,
    relay: EngineRelayHandle,
    expected_token: String,
    lan_port: u16,
) -> Result<(), GatewayAdminError> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let authentication = timeout(
        ADMIN_AUTHENTICATION_TIMEOUT,
        read_bounded_json::<AdminAuthentication>(&mut reader),
    )
    .await
    .map_err(|_| GatewayAdminError::AuthenticationTimeout)??;
    if !tokens_match(&expected_token, &authentication.admin_token) {
        return Err(GatewayAdminError::AuthenticationRejected);
    }
    loop {
        let request = match read_bounded_json::<GatewayAdminRequest>(&mut reader).await {
            Ok(request) => request,
            Err(GatewayAdminError::UnexpectedEnd) => return Ok(()),
            Err(error) => return Err(error),
        };
        let response = apply_request(request, &state, &relay, lan_port).await;
        let mut bytes = serde_json::to_vec(&response)?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;
        writer.flush().await?;
    }
}

async fn apply_request(
    request: GatewayAdminRequest,
    state: &SharedGatewayState,
    relay: &EngineRelayHandle,
    lan_port: u16,
) -> GatewayAdminResponse {
    let outcome = match request {
        GatewayAdminRequest::Status => Ok(None),
        GatewayAdminRequest::CreateInvitation => create_invitation(state).await.map(Some),
        GatewayAdminRequest::ApproveInvitation {
            invitation_id,
            short_code,
        } => {
            let mut registry = state.registry.lock().await;
            registry.approve(&invitation_id, &short_code).map(|()| None)
        }
        GatewayAdminRequest::RevokeDevice { device_id } => {
            let mut registry = state.registry.lock().await;
            if registry.revoke(&device_id) {
                let controller_matches = state
                    .command_guard
                    .lock()
                    .await
                    .controller()
                    .is_some_and(|controller| controller.device_id == device_id);
                if controller_matches {
                    state.command_guard.lock().await.revoke_control();
                }
                let saved = state
                    .trust_store
                    .save(&registry)
                    .map_err(|_| crate::PairingError::InvalidPersistedDevice)
                    .map(|()| None);
                if saved.is_ok() {
                    state.notify_trust_changed();
                }
                saved
            } else {
                Err(crate::PairingError::DeviceUnknown)
            }
        }
        GatewayAdminRequest::TransferControl { device_id } => {
            let mut registry = state.registry.lock().await;
            if registry.contains_device(&device_id) {
                match random_hex(24) {
                    Ok(lease) => {
                        let changed = registry.set_controller(&device_id).and_then(|()| {
                            state
                                .trust_store
                                .save(&registry)
                                .map_err(|_| crate::PairingError::InvalidPersistedDevice)
                        });
                        if changed.is_ok() {
                            state
                                .command_guard
                                .lock()
                                .await
                                .transfer_control(device_id, lease);
                            state.notify_trust_changed();
                        }
                        changed.map(|()| None)
                    }
                    Err(_) => Err(crate::PairingError::InvalidCredential),
                }
            } else {
                Err(crate::PairingError::DeviceUnknown)
            }
        }
    };
    let (ok, invitation, error_code) = match outcome {
        Ok(invitation) => (true, invitation, None),
        Err(error) => (false, None, Some(pairing_error_code(&error).to_owned())),
    };
    GatewayAdminResponse {
        ok,
        status: status(state, relay, lan_port).await,
        invitation,
        error_code,
    }
}

async fn create_invitation(
    state: &SharedGatewayState,
) -> Result<GatewayPairingInvitation, crate::PairingError> {
    let now = unix_millis();
    let invitation_id = random_hex(16).map_err(|_| crate::PairingError::InvalidInvitation)?;
    let invitation_secret = random_hex(32).map_err(|_| crate::PairingError::InvalidInvitation)?;
    let short_code_seed = u64::from_str_radix(
        &random_hex(8).map_err(|_| crate::PairingError::InvalidInvitation)?,
        16,
    )
    .map_err(|_| crate::PairingError::InvalidInvitation)?;
    let short_code = format!("{:06}", short_code_seed % 1_000_000);
    let expires = now.saturating_add(MAX_INVITATION_LIFETIME_MILLIS);
    let request = PairingInvitationRequest {
        invitation_id: invitation_id.clone(),
        invitation_secret: invitation_secret.clone(),
        short_code: short_code.clone(),
        certificate_fingerprint_sha256: state.identity.certificate_fingerprint_sha256.clone(),
        created_at_unix_millis: now,
        expires_at_unix_millis: expires,
    };
    state.registry.lock().await.create_invitation(request)?;
    Ok(GatewayPairingInvitation {
        installation_id: state.identity.installation_id.clone(),
        invitation_id,
        invitation_secret,
        short_code,
        certificate_fingerprint_sha256: state.identity.certificate_fingerprint_sha256.clone(),
        expires_at_unix_millis: expires,
        approved: false,
    })
}

async fn status(
    state: &SharedGatewayState,
    relay: &EngineRelayHandle,
    lan_port: u16,
) -> GatewayAdminStatus {
    let controller = state
        .command_guard
        .lock()
        .await
        .controller()
        .map(|controller| controller.device_id.clone());
    let devices = state
        .registry
        .lock()
        .await
        .paired_devices()
        .map(|device| GatewayPairedDeviceStatus {
            device_id: device.device_id.clone(),
            display_name: device.display_name.clone(),
            paired_at_unix_millis: device.paired_at_unix_millis,
            last_seen_unix_millis: device.last_seen_unix_millis,
            controller: controller.as_deref() == Some(device.device_id.as_str()),
        })
        .collect();
    GatewayAdminStatus {
        engine_connected: relay.connected(),
        installation_id: state.identity.installation_id.clone(),
        certificate_fingerprint_sha256: state.identity.certificate_fingerprint_sha256.clone(),
        lan_port,
        paired_devices: devices,
        controller_device_id: controller,
    }
}

async fn read_bounded_json<T>(
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
) -> Result<T, GatewayAdminError>
where
    T: for<'de> Deserialize<'de>,
{
    let mut bytes = Vec::with_capacity(512);
    let read = reader.read_until(b'\n', &mut bytes).await?;
    if read == 0 {
        return Err(GatewayAdminError::UnexpectedEnd);
    }
    if bytes.len() > MAXIMUM_ADMIN_LINE_BYTES.saturating_add(1) {
        return Err(GatewayAdminError::Oversized);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_protected_record(
    path: &Path,
    record: &GatewayAdminServiceRecord,
) -> Result<(), GatewayAdminError> {
    let parent = path
        .parent()
        .ok_or_else(|| GatewayAdminError::UntrustedRecord(path.to_owned()))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temporary = path.with_file_name(format!(
        ".remote-gateway-service.{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    serde_json::to_writer(&mut file, record)?;
    file.flush()?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn tokens_match(expected: &str, received: &str) -> bool {
    expected.len() == received.len() && bool::from(expected.as_bytes().ct_eq(received.as_bytes()))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn pairing_error_code(error: &crate::PairingError) -> &'static str {
    match error {
        crate::PairingError::InvalidInvitation => "invalidInvitation",
        crate::PairingError::InvitationUnknown => "invitationUnknown",
        crate::PairingError::InvitationExpired => "invitationExpired",
        crate::PairingError::ApprovalRequired => "approvalRequired",
        crate::PairingError::ShortCodeMismatch => "shortCodeMismatch",
        crate::PairingError::DeviceUnknown => "deviceUnknown",
        crate::PairingError::DeviceLimitReached => "deviceLimitReached",
        _ => "pairingRejected",
    }
}

#[derive(Debug, Error)]
pub enum GatewayAdminError {
    #[error("gateway admin authentication timed out")]
    AuthenticationTimeout,
    #[error("gateway admin authentication was rejected")]
    AuthenticationRejected,
    #[error("gateway admin connection ended")]
    UnexpectedEnd,
    #[error("gateway admin message exceeds its bounded size")]
    Oversized,
    #[error("gateway admin record path is invalid: {0}")]
    UntrustedRecord(PathBuf),
    #[error("gateway admin I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("gateway admin JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("gateway admin identity failed: {0}")]
    Identity(#[from] crate::IdentityError),
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
    use tokio::net::TcpStream;

    use super::{
        GatewayAdminRequest, GatewayAdminResponse, GatewayAdminServer, GatewayAdminServiceRecord,
    };
    use crate::{
        EngineRelayHandle, InstallationIdentity, PersistentTrustStore, SharedGatewayState,
    };

    #[tokio::test]
    async fn protected_admin_flow_creates_and_approves_one_invitation() -> Result<(), Box<dyn Error>>
    {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "lumi-remote-admin-test-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&directory)?;
        let identity = InstallationIdentity::load_or_create(&directory.join("identity"))?;
        let state = SharedGatewayState::load(
            identity,
            PersistentTrustStore::new(directory.join("trust.json")),
        )?;
        let relay = EngineRelayHandle::start(directory.join("missing-engine-record.json"));
        let record_path = directory.join("admin.json");
        let (server, _record_guard) = GatewayAdminServer::bind(
            state,
            relay,
            &record_path,
            "0.6.0-dev-test".to_owned(),
            42_424,
        )
        .await?;
        let record: GatewayAdminServiceRecord = serde_json::from_slice(&fs::read(&record_path)?)?;
        let task = tokio::spawn(server.run());

        let stream =
            TcpStream::connect((record.endpoint_host.as_str(), record.endpoint_port)).await?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        writer
            .write_all(format!("{}\n", json!({"adminToken": record.admin_token})).as_bytes())
            .await?;
        write_request(&mut writer, &GatewayAdminRequest::CreateInvitation).await?;
        let created = read_response(&mut reader).await?;
        assert!(created.ok, "admin create response: {created:?}");
        let invitation = created
            .invitation
            .ok_or_else(|| io::Error::other("admin response omitted pairing invitation"))?;
        assert_eq!(invitation.short_code.len(), 6);
        assert!(!invitation.approved);

        write_request(
            &mut writer,
            &GatewayAdminRequest::ApproveInvitation {
                invitation_id: invitation.invitation_id,
                short_code: invitation.short_code,
            },
        )
        .await?;
        let approved = read_response(&mut reader).await?;
        assert!(approved.ok);
        assert!(approved.invitation.is_none());

        task.abort();
        let _ = task.await;
        let _ = fs::remove_dir_all(directory);
        Ok(())
    }

    async fn write_request(
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        request: &GatewayAdminRequest,
    ) -> Result<(), Box<dyn Error>> {
        let mut bytes = serde_json::to_vec(request)?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;
        Ok(())
    }

    async fn read_response(
        reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    ) -> Result<GatewayAdminResponse, Box<dyn Error>> {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        Ok(serde_json::from_str(&line)?)
    }
}
