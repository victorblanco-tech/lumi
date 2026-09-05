//! Isolated buffering and authorization boundary for Lumi Remote.
//!
//! Network discovery and TLS are deliberately layered outside these types so
//! all queueing and command-policy behavior remains deterministic and can be
//! stress-tested without a network or Apple UI process.

#![forbid(unsafe_code)]

mod admin;
mod engine_client;
mod identity;
mod network;
mod trust_store;

pub use admin::{
    GatewayAdminRecordGuard, GatewayAdminRequest, GatewayAdminResponse, GatewayAdminServer,
    GatewayAdminServiceRecord, GatewayAdminStatus, GatewayPairedDeviceStatus,
    GatewayPairingInvitation,
};
pub use engine_client::{
    EngineClientError, EngineProjectionClient, EngineRemoteEndpoint, EngineRemoteServiceRecord,
};
pub use identity::{IdentityError, InstallationIdentity, random_hex};
pub use network::{
    EngineRelayHandle, GatewayNetworkError, GatewayNetworkServer, SharedGatewayState,
};
pub use trust_store::{PersistentTrustStore, TrustStoreError};

use std::collections::{BTreeMap, VecDeque};
use std::net::IpAddr;

use lumi_remote_protocol::{
    RemoteCommand, RemoteCommandResult, RemoteFrame, RemoteFrameKind, RemoteLiveProjection,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq as _;
use thiserror::Error;

pub const DEFAULT_CRITICAL_QUEUE_CAPACITY: usize = 64;
pub const MAX_REMOTE_CLIENTS: usize = 8;
pub const MAX_PAIRED_DEVICES: usize = 8;
pub const MAX_INVITATION_LIFETIME_MILLIS: u64 = 5 * 60 * 1_000;
pub const DEFAULT_COMMAND_LEDGER_CAPACITY: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseChannel {
    Dev,
    Rc,
    Production,
}

impl ReleaseChannel {
    pub const fn bonjour_service_type(self) -> &'static str {
        match self {
            Self::Dev => "_lumi-remote-dev._tcp",
            Self::Rc => "_lumi-remote-rc._tcp",
            Self::Production => "_lumi-remote._tcp",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Rc => "rc",
            Self::Production => "production",
        }
    }

    pub fn discovery_metadata(
        self,
        installation_id: &str,
        port: u16,
    ) -> Result<BTreeMap<String, String>, GatewayConfigError> {
        if installation_id.is_empty()
            || installation_id.len() > 128
            || installation_id.chars().any(char::is_control)
        {
            return Err(GatewayConfigError::InvalidInstallationIdentity);
        }
        Ok(BTreeMap::from([
            ("id".to_owned(), installation_id.to_owned()),
            (
                "pv".to_owned(),
                lumi_remote_protocol::REMOTE_PROTOCOL_VERSION.to_string(),
            ),
            ("channel".to_owned(), self.as_str().to_owned()),
            ("port".to_owned(), port.to_string()),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayConfig {
    pub release_channel: ReleaseChannel,
    pub engine_host: IpAddr,
    pub maximum_clients: usize,
    pub critical_queue_capacity: usize,
}

impl GatewayConfig {
    pub fn validate(&self) -> Result<(), GatewayConfigError> {
        if !self.engine_host.is_loopback() {
            return Err(GatewayConfigError::EngineMustRemainLoopback);
        }
        if !(1..=MAX_REMOTE_CLIENTS).contains(&self.maximum_clients) {
            return Err(GatewayConfigError::InvalidClientLimit);
        }
        if !(8..=256).contains(&self.critical_queue_capacity) {
            return Err(GatewayConfigError::InvalidCriticalQueueCapacity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GatewayConfigError {
    #[error("the Remote Gateway may connect to the Lumi engine only over loopback")]
    EngineMustRemainLoopback,
    #[error("maximum client count must be between 1 and 8")]
    InvalidClientLimit,
    #[error("critical queue capacity must be between 8 and 256")]
    InvalidCriticalQueueCapacity,
    #[error("installation identity is invalid")]
    InvalidInstallationIdentity,
}

#[derive(Clone, Debug)]
pub struct ProjectionHub {
    latest_projection: Option<RemoteLiveProjection>,
    clients: BTreeMap<u64, ClientBuffer>,
    next_client_id: u64,
    maximum_clients: usize,
    critical_queue_capacity: usize,
}

impl ProjectionHub {
    pub fn new(
        maximum_clients: usize,
        critical_queue_capacity: usize,
    ) -> Result<Self, GatewayConfigError> {
        let config = GatewayConfig {
            release_channel: ReleaseChannel::Dev,
            engine_host: IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            maximum_clients,
            critical_queue_capacity,
        };
        config.validate()?;
        Ok(Self {
            latest_projection: None,
            clients: BTreeMap::new(),
            next_client_id: 1,
            maximum_clients,
            critical_queue_capacity,
        })
    }

    pub fn connect(&mut self) -> Result<u64, HubError> {
        if self.clients.len() >= self.maximum_clients {
            return Err(HubError::ClientLimitReached);
        }
        let client_id = self.next_client_id;
        self.next_client_id = self.next_client_id.saturating_add(1);
        let mut client = ClientBuffer::new(self.critical_queue_capacity);
        if let Some(projection) = &self.latest_projection {
            client.enqueue_snapshot(projection.clone())?;
        }
        self.clients.insert(client_id, client);
        Ok(client_id)
    }

    pub fn disconnect(&mut self, client_id: u64) {
        self.clients.remove(&client_id);
    }

    pub fn publish_projection(
        &mut self,
        projection: RemoteLiveProjection,
    ) -> Result<Vec<u64>, HubError> {
        projection.validate().map_err(HubError::InvalidProjection)?;
        if self
            .latest_projection
            .as_ref()
            .is_some_and(|latest| projection.projection_revision <= latest.projection_revision)
        {
            return Err(HubError::NonIncreasingProjectionRevision);
        }
        self.latest_projection = Some(projection.clone());
        let mut saturated = Vec::new();
        for (client_id, client) in &mut self.clients {
            if client.enqueue_projection(projection.clone()).is_err() {
                saturated.push(*client_id);
            }
        }
        for client_id in &saturated {
            self.clients.remove(client_id);
        }
        Ok(saturated)
    }

    pub fn publish_transport_anchor(
        &mut self,
        player_number: u8,
        frame: RemoteFrame,
    ) -> Result<(), HubError> {
        if frame.frame_kind != RemoteFrameKind::TransportAnchor || !(1..=6).contains(&player_number)
        {
            return Err(HubError::InvalidTransportAnchor);
        }
        for client in self.clients.values_mut() {
            client.enqueue_transport_anchor(player_number, frame.clone());
        }
        Ok(())
    }

    pub fn next_frame(&mut self, client_id: u64) -> Result<Option<RemoteFrame>, HubError> {
        self.clients
            .get_mut(&client_id)
            .map(ClientBuffer::next_frame)
            .ok_or(HubError::UnknownClient)
    }

    pub fn metrics(&self, client_id: u64) -> Option<ClientBufferMetrics> {
        self.clients.get(&client_id).map(ClientBuffer::metrics)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientBufferMetrics {
    pub critical_depth: usize,
    pub latest_anchor_count: usize,
    pub coalesced_anchor_count: u64,
}

#[derive(Clone, Debug)]
struct ClientBuffer {
    critical: VecDeque<RemoteFrame>,
    latest_anchors: BTreeMap<u8, RemoteFrame>,
    critical_capacity: usize,
    coalesced_anchor_count: u64,
    next_delivery_sequence: u64,
}

impl ClientBuffer {
    fn new(critical_capacity: usize) -> Self {
        Self {
            critical: VecDeque::with_capacity(critical_capacity),
            latest_anchors: BTreeMap::new(),
            critical_capacity,
            coalesced_anchor_count: 0,
            next_delivery_sequence: 1,
        }
    }

    fn enqueue_snapshot(&mut self, projection: RemoteLiveProjection) -> Result<(), HubError> {
        self.enqueue_projection_kind(projection, RemoteFrameKind::Snapshot)
    }

    fn enqueue_projection(&mut self, projection: RemoteLiveProjection) -> Result<(), HubError> {
        self.enqueue_projection_kind(projection, RemoteFrameKind::Projection)
    }

    fn enqueue_projection_kind(
        &mut self,
        projection: RemoteLiveProjection,
        frame_kind: RemoteFrameKind,
    ) -> Result<(), HubError> {
        let payload = serde_json::to_value(projection).map_err(HubError::SerializeProjection)?;
        self.enqueue_critical(RemoteFrame {
            protocol_version: lumi_remote_protocol::REMOTE_PROTOCOL_VERSION,
            frame_kind,
            // Sequence is assigned only when this client receives the frame.
            // Source transport updates may be coalesced before that point.
            sequence: 0,
            correlation_id: None,
            payload,
        })
    }

    fn enqueue_critical(&mut self, frame: RemoteFrame) -> Result<(), HubError> {
        if self.critical.len() >= self.critical_capacity {
            return Err(HubError::CriticalQueueSaturated);
        }
        self.critical.push_back(frame);
        Ok(())
    }

    fn enqueue_transport_anchor(&mut self, player_number: u8, frame: RemoteFrame) {
        if self.latest_anchors.insert(player_number, frame).is_some() {
            self.coalesced_anchor_count = self.coalesced_anchor_count.saturating_add(1);
        }
    }

    fn next_frame(&mut self) -> Option<RemoteFrame> {
        let mut frame = self.critical.pop_front().or_else(|| {
            let player_number = *self.latest_anchors.keys().next()?;
            self.latest_anchors.remove(&player_number)
        })?;
        frame.sequence = self.next_delivery_sequence;
        self.next_delivery_sequence = self.next_delivery_sequence.saturating_add(1);
        Some(frame)
    }

    fn metrics(&self) -> ClientBufferMetrics {
        ClientBufferMetrics {
            critical_depth: self.critical.len(),
            latest_anchor_count: self.latest_anchors.len(),
            coalesced_anchor_count: self.coalesced_anchor_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerLease {
    pub device_id: String,
    pub lease_id: String,
}

#[derive(Clone, Debug, Default)]
pub struct GatewayCommandPolicy {
    controller: Option<ControllerLease>,
}

impl GatewayCommandPolicy {
    pub fn transfer_control(&mut self, device_id: String, lease_id: String) {
        self.controller = Some(ControllerLease {
            device_id,
            lease_id,
        });
    }

    pub fn revoke_control(&mut self) {
        self.controller = None;
    }

    pub fn controller(&self) -> Option<&ControllerLease> {
        self.controller.as_ref()
    }

    pub fn authorize(
        &self,
        device_id: &str,
        command: &RemoteCommand,
    ) -> Result<(), CommandAuthorizationError> {
        command
            .validate()
            .map_err(CommandAuthorizationError::InvalidCommand)?;
        if !command.is_mutating() {
            return Ok(());
        }
        let Some(controller) = &self.controller else {
            return Err(CommandAuthorizationError::NoControllerLease);
        };
        if controller.device_id != device_id || controller.lease_id != command.controller_lease_id {
            return Err(CommandAuthorizationError::ControllerLeaseMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum HubError {
    #[error("remote client limit reached")]
    ClientLimitReached,
    #[error("remote client is unknown")]
    UnknownClient,
    #[error("remote client critical queue saturated")]
    CriticalQueueSaturated,
    #[error("remote transport anchor is invalid")]
    InvalidTransportAnchor,
    #[error("remote projection revision must increase")]
    NonIncreasingProjectionRevision,
    #[error("remote projection failed validation: {0}")]
    InvalidProjection(lumi_remote_protocol::ProjectionError),
    #[error("remote projection serialization failed: {0}")]
    SerializeProjection(serde_json::Error),
}

#[derive(Debug, Error)]
pub enum CommandAuthorizationError {
    #[error("remote command failed validation: {0}")]
    InvalidCommand(lumi_remote_protocol::RemoteCommandError),
    #[error("no iPhone currently owns the Controller lease")]
    NoControllerLease,
    #[error("remote command does not match the active Controller lease")]
    ControllerLeaseMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandAdmission {
    Accepted,
    Pending,
    Completed(RemoteCommandResult),
}

type CommandKey = (String, String, String); // authenticated device, lease, client ID

#[derive(Clone, Debug)]
struct CommandOutcome {
    command: RemoteCommand,
    result: Option<RemoteCommandResult>,
}

#[derive(Clone, Debug)]
pub struct GatewayCommandGuard {
    policy: GatewayCommandPolicy,
    accepted: BTreeMap<CommandKey, CommandOutcome>,
    forwarded: BTreeMap<String, CommandKey>,
    insertion_order: VecDeque<CommandKey>,
    capacity: usize,
}

impl GatewayCommandGuard {
    pub fn new(capacity: usize) -> Result<Self, CommandGuardError> {
        if !(64..=8_192).contains(&capacity) {
            return Err(CommandGuardError::InvalidCapacity);
        }
        Ok(Self {
            policy: GatewayCommandPolicy::default(),
            accepted: BTreeMap::new(),
            forwarded: BTreeMap::new(),
            insertion_order: VecDeque::with_capacity(capacity),
            capacity,
        })
    }

    pub fn transfer_control(&mut self, device_id: String, lease_id: String) {
        self.policy.transfer_control(device_id, lease_id);
    }

    pub fn revoke_control(&mut self) {
        self.policy.revoke_control();
    }

    pub fn controller(&self) -> Option<&ControllerLease> {
        self.policy.controller()
    }

    pub fn grant_first_controller(&mut self, device_id: &str, lease_id: String) -> Option<String> {
        if self.policy.controller().is_none() {
            self.policy
                .transfer_control(device_id.to_owned(), lease_id.clone());
            return Some(lease_id);
        }
        self.controller_lease_for(device_id)
    }

    pub fn controller_lease_for(&self, device_id: &str) -> Option<String> {
        self.policy
            .controller()
            .filter(|controller| controller.device_id == device_id)
            .map(|controller| controller.lease_id.clone())
    }

    pub fn admit(
        &mut self,
        device_id: &str,
        command: &RemoteCommand,
    ) -> Result<CommandAdmission, CommandGuardError> {
        self.policy
            .authorize(device_id, command)
            .map_err(CommandGuardError::Unauthorized)?;
        if !command.is_mutating() {
            return Ok(CommandAdmission::Accepted);
        }
        let key = (
            device_id.to_owned(),
            command.controller_lease_id.clone(),
            command.command_id.clone(),
        );
        if let Some(outcome) = self.accepted.get(&key) {
            if outcome.command != *command {
                return Err(CommandGuardError::CommandIdReused);
            }
            return Ok(match &outcome.result {
                Some(result) => CommandAdmission::Completed(result.clone()),
                None => CommandAdmission::Pending,
            });
        }
        if self.insertion_order.len() == self.capacity {
            // Never forget an in-flight/uncertain command and then execute it
            // again. Only a known terminal outcome may leave the bounded ledger.
            let index = self
                .insertion_order
                .iter()
                .position(|key| {
                    self.accepted
                        .get(key)
                        .is_some_and(|entry| entry.result.is_some())
                })
                .ok_or(CommandGuardError::LedgerFull)?;
            if let Some(oldest) = self.insertion_order.remove(index) {
                self.forwarded.remove(&forwarded_command_id(&oldest));
                self.accepted.remove(&oldest);
            }
        }
        self.forwarded
            .insert(forwarded_command_id(&key), key.clone());
        self.accepted.insert(
            key.clone(),
            CommandOutcome {
                command: command.clone(),
                result: None,
            },
        );
        self.insertion_order.push_back(key);
        Ok(CommandAdmission::Accepted)
    }

    pub fn forwarded_command(&self, device_id: &str, command: &RemoteCommand) -> RemoteCommand {
        let mut forwarded = command.clone();
        forwarded.command_id = forwarded_command_id(&(
            device_id.to_owned(),
            command.controller_lease_id.clone(),
            command.command_id.clone(),
        ));
        forwarded
    }

    /// Called by the production relay, even when the requesting phone has
    /// disconnected. Admission and a successful socket write are not outcomes.
    pub fn record_result(&mut self, result: RemoteCommandResult) {
        if let Some(key) = self.forwarded.get(&result.command_id)
            && let Some(outcome) = self.accepted.get_mut(key)
            && outcome.result.is_none()
        {
            let mut result = result;
            result.command_id = key.2.clone();
            outcome.result = Some(result);
        }
    }

    pub fn result_for_device(
        &self,
        device_id: &str,
        forwarded_id: &str,
    ) -> Option<RemoteCommandResult> {
        let key = self.forwarded.get(forwarded_id)?;
        if key.0 != device_id {
            return None;
        }
        self.accepted.get(key)?.result.clone()
    }

    pub fn authorize_forwarded(&self, command: &RemoteCommand) -> Result<(), CommandGuardError> {
        let key = self
            .forwarded
            .get(&command.command_id)
            .ok_or(CommandGuardError::UnknownCommand)?;
        let original = &self
            .accepted
            .get(key)
            .ok_or(CommandGuardError::UnknownCommand)?
            .command;
        self.policy
            .authorize(&key.0, original)
            .map_err(CommandGuardError::Unauthorized)
    }
}

fn forwarded_command_id(key: &CommandKey) -> String {
    let mut digest = Sha256::new();
    for value in [&key.0, &key.1, &key.2] {
        digest.update(value.len().to_be_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[derive(Debug, Error)]
pub enum CommandGuardError {
    #[error("remote command ledger capacity must be between 64 and 8192")]
    InvalidCapacity,
    #[error("command ID was reused with different content")]
    CommandIdReused,
    #[error("command ledger is full of unresolved outcomes")]
    LedgerFull,
    #[error("command was not admitted by this gateway")]
    UnknownCommand,
    #[error(transparent)]
    Unauthorized(CommandAuthorizationError),
}

#[derive(Clone, Debug)]
pub struct AttemptRateLimiter {
    maximum_attempts: usize,
    window_millis: u64,
    attempts: BTreeMap<String, VecDeque<u64>>,
}

impl AttemptRateLimiter {
    pub fn new(maximum_attempts: usize, window_millis: u64) -> Result<Self, RateLimitError> {
        if maximum_attempts == 0 || maximum_attempts > 100 || window_millis == 0 {
            return Err(RateLimitError::InvalidConfiguration);
        }
        Ok(Self {
            maximum_attempts,
            window_millis,
            attempts: BTreeMap::new(),
        })
    }

    pub fn record(&mut self, key: &str, now_unix_millis: u64) -> Result<(), RateLimitError> {
        self.check(key, now_unix_millis)?;
        self.attempts
            .entry(key.to_owned())
            .or_default()
            .push_back(now_unix_millis);
        Ok(())
    }

    /// Check before authentication work. Expired identities are removed and
    /// an address flood cannot create an unbounded map or evict active limits.
    pub fn check(&mut self, key: &str, now_unix_millis: u64) -> Result<(), RateLimitError> {
        if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
            return Err(RateLimitError::InvalidKey);
        }
        self.attempts.retain(|_, attempts| {
            while attempts
                .front()
                .is_some_and(|value| now_unix_millis.saturating_sub(*value) >= self.window_millis)
            {
                attempts.pop_front();
            }
            !attempts.is_empty()
        });
        if self
            .attempts
            .get(key)
            .is_some_and(|attempts| attempts.len() >= self.maximum_attempts)
            || (!self.attempts.contains_key(key) && self.attempts.len() >= 1_024)
        {
            return Err(RateLimitError::Limited);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RateLimitError {
    #[error("rate limit configuration is invalid")]
    InvalidConfiguration,
    #[error("rate limit key is invalid")]
    InvalidKey,
    #[error("too many attempts")]
    Limited,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitationRequest {
    pub invitation_id: String,
    pub invitation_secret: String,
    pub short_code: String,
    pub certificate_fingerprint_sha256: String,
    pub created_at_unix_millis: u64,
    pub expires_at_unix_millis: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingInvitation {
    invitation_id: String,
    secret_sha256: [u8; 32],
    short_code: String,
    certificate_fingerprint_sha256: String,
    expires_at_unix_millis: u64,
    approved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitationDetails {
    pub invitation_id: String,
    pub short_code: String,
    pub certificate_fingerprint_sha256: String,
    pub expires_at_unix_millis: u64,
    pub approved: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub device_id: String,
    pub display_name: String,
    pub credential_sha256: [u8; 32],
    pub paired_at_unix_millis: u64,
    pub last_seen_unix_millis: u64,
    #[serde(default)]
    pub controller: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingRegistrySnapshot {
    pub devices: Vec<PairedDevice>,
}

#[derive(Clone, Debug, Default)]
pub struct PairingRegistry {
    pending: BTreeMap<String, PendingInvitation>,
    devices: BTreeMap<String, PairedDevice>,
}

impl PairingRegistry {
    pub fn from_snapshot(snapshot: PairingRegistrySnapshot) -> Result<Self, PairingError> {
        if snapshot.devices.len() > MAX_PAIRED_DEVICES {
            return Err(PairingError::DeviceLimitReached);
        }
        let mut devices = BTreeMap::new();
        let mut controller_count = 0_usize;
        for device in snapshot.devices {
            let is_controller = device.controller;
            validate_public_identifier(&device.device_id, 128)?;
            validate_public_identifier(&device.display_name, 128)?;
            if device.paired_at_unix_millis == 0
                || device.last_seen_unix_millis < device.paired_at_unix_millis
                || devices.insert(device.device_id.clone(), device).is_some()
            {
                return Err(PairingError::InvalidPersistedDevice);
            }
            if is_controller {
                controller_count = controller_count.saturating_add(1);
            }
        }
        if controller_count > 1 {
            return Err(PairingError::InvalidPersistedDevice);
        }
        Ok(Self {
            pending: BTreeMap::new(),
            devices,
        })
    }

    pub fn snapshot(&self) -> PairingRegistrySnapshot {
        PairingRegistrySnapshot {
            devices: self.devices.values().cloned().collect(),
        }
    }

    pub fn create_invitation(
        &mut self,
        request: PairingInvitationRequest,
    ) -> Result<(), PairingError> {
        validate_public_identifier(&request.invitation_id, 128)?;
        if request.invitation_secret.len() < 32
            || request.invitation_secret.len() > 256
            || request.short_code.len() != 6
            || !request
                .short_code
                .chars()
                .all(|character| character.is_ascii_digit())
            || !valid_sha256_fingerprint(&request.certificate_fingerprint_sha256)
        {
            return Err(PairingError::InvalidInvitation);
        }
        let lifetime = request
            .expires_at_unix_millis
            .checked_sub(request.created_at_unix_millis)
            .ok_or(PairingError::InvalidInvitation)?;
        if lifetime == 0 || lifetime > MAX_INVITATION_LIFETIME_MILLIS {
            return Err(PairingError::InvalidInvitation);
        }
        self.pending.clear();
        self.pending.insert(
            request.invitation_id.clone(),
            PendingInvitation {
                invitation_id: request.invitation_id,
                secret_sha256: hash_secret(request.invitation_secret.as_bytes()),
                short_code: request.short_code,
                certificate_fingerprint_sha256: request.certificate_fingerprint_sha256,
                expires_at_unix_millis: request.expires_at_unix_millis,
                approved: false,
            },
        );
        Ok(())
    }

    pub fn approve(&mut self, invitation_id: &str, short_code: &str) -> Result<(), PairingError> {
        let invitation = self
            .pending
            .get_mut(invitation_id)
            .ok_or(PairingError::InvitationUnknown)?;
        if invitation.short_code != short_code {
            return Err(PairingError::ShortCodeMismatch);
        }
        invitation.approved = true;
        Ok(())
    }

    pub fn invitation_details(&self, invitation_id: &str) -> Option<PairingInvitationDetails> {
        self.pending
            .get(invitation_id)
            .map(|invitation| PairingInvitationDetails {
                invitation_id: invitation.invitation_id.clone(),
                short_code: invitation.short_code.clone(),
                certificate_fingerprint_sha256: invitation.certificate_fingerprint_sha256.clone(),
                expires_at_unix_millis: invitation.expires_at_unix_millis,
                approved: invitation.approved,
            })
    }

    pub fn exchange(
        &mut self,
        invitation_id: &str,
        invitation_secret: &str,
        device_id: String,
        display_name: String,
        device_credential: &str,
        now_unix_millis: u64,
    ) -> Result<PairedDevice, PairingError> {
        let invitation = self
            .pending
            .get(invitation_id)
            .ok_or(PairingError::InvitationUnknown)?;
        if now_unix_millis > invitation.expires_at_unix_millis {
            return Err(PairingError::InvitationExpired);
        }
        if !invitation.approved {
            return Err(PairingError::ApprovalRequired);
        }
        let supplied_hash = hash_secret(invitation_secret.as_bytes());
        if invitation.secret_sha256.ct_eq(&supplied_hash).unwrap_u8() != 1 {
            return Err(PairingError::InvitationSecretMismatch);
        }
        validate_public_identifier(&device_id, 128)?;
        validate_public_identifier(&display_name, 128)?;
        if device_credential.len() < 32 || device_credential.len() > 256 {
            return Err(PairingError::InvalidCredential);
        }
        if !self.devices.contains_key(&device_id) && self.devices.len() >= MAX_PAIRED_DEVICES {
            return Err(PairingError::DeviceLimitReached);
        }
        self.pending.remove(invitation_id);
        let device = PairedDevice {
            device_id: device_id.clone(),
            display_name,
            credential_sha256: hash_secret(device_credential.as_bytes()),
            paired_at_unix_millis: now_unix_millis,
            last_seen_unix_millis: now_unix_millis,
            controller: self.devices.is_empty(),
        };
        self.devices.insert(device_id, device.clone());
        Ok(device)
    }

    pub fn authenticate(
        &mut self,
        device_id: &str,
        credential: &str,
        now_unix_millis: u64,
    ) -> Result<(), PairingError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or(PairingError::DeviceUnknown)?;
        let supplied_hash = hash_secret(credential.as_bytes());
        if device.credential_sha256.ct_eq(&supplied_hash).unwrap_u8() != 1 {
            return Err(PairingError::CredentialMismatch);
        }
        device.last_seen_unix_millis = now_unix_millis;
        Ok(())
    }

    pub fn revoke(&mut self, device_id: &str) -> bool {
        self.devices.remove(device_id).is_some()
    }

    pub fn paired_devices(&self) -> impl Iterator<Item = &PairedDevice> {
        self.devices.values()
    }

    pub fn contains_device(&self, device_id: &str) -> bool {
        self.devices.contains_key(device_id)
    }

    pub fn controller_device_id(&self) -> Option<&str> {
        self.devices
            .values()
            .find(|device| device.controller)
            .map(|device| device.device_id.as_str())
    }

    pub fn set_controller(&mut self, device_id: &str) -> Result<(), PairingError> {
        if !self.devices.contains_key(device_id) {
            return Err(PairingError::DeviceUnknown);
        }
        for device in self.devices.values_mut() {
            device.controller = device.device_id == device_id;
        }
        Ok(())
    }
}

fn hash_secret(secret: &[u8]) -> [u8; 32] {
    Sha256::digest(secret).into()
}

fn valid_sha256_fingerprint(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn validate_public_identifier(value: &str, maximum: usize) -> Result<(), PairingError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(PairingError::InvalidIdentifier);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PairingError {
    #[error("pairing invitation is invalid")]
    InvalidInvitation,
    #[error("pairing invitation is unknown or already consumed")]
    InvitationUnknown,
    #[error("pairing invitation has expired")]
    InvitationExpired,
    #[error("pairing invitation still requires explicit Mac approval")]
    ApprovalRequired,
    #[error("pairing short code does not match")]
    ShortCodeMismatch,
    #[error("pairing invitation secret does not match")]
    InvitationSecretMismatch,
    #[error("pairing device identifier or name is invalid")]
    InvalidIdentifier,
    #[error("pairing device credential is invalid")]
    InvalidCredential,
    #[error("persisted paired-device data is invalid")]
    InvalidPersistedDevice,
    #[error("paired device limit reached")]
    DeviceLimitReached,
    #[error("paired device is unknown or revoked")]
    DeviceUnknown,
    #[error("paired device credential does not match")]
    CredentialMismatch,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use lumi_remote_protocol::{
        IntegrationHealth, OperationState, REMOTE_PROTOCOL_VERSION, RemoteCommand,
        RemoteCommandKind, RemoteFrame, RemoteFrameKind, RemoteIntegrationStatus,
        RemoteLiveProjection,
    };

    use super::{
        AttemptRateLimiter, CommandAdmission, CommandGuardError, DEFAULT_COMMAND_LEDGER_CAPACITY,
        GatewayCommandGuard, GatewayCommandPolicy, HubError, PairingError,
        PairingInvitationRequest, PairingRegistry, ProjectionHub, RateLimitError, ReleaseChannel,
    };

    fn projection(revision: u64) -> RemoteLiveProjection {
        RemoteLiveProjection {
            projection_revision: revision,
            state_revision: revision,
            engine_version: "0.6.0-dev-4".to_owned(),
            operation_state: OperationState::Off,
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
            phrase_role_options: Vec::new(),
        }
    }

    #[test]
    fn discovery_metadata_matches_the_scoped_client_contract() {
        let metadata = ReleaseChannel::Dev
            .discovery_metadata("installation-123", 61_234)
            .unwrap_or_default();
        assert_eq!(
            metadata.get("id").map(String::as_str),
            Some("installation-123")
        );
        assert_eq!(metadata.get("pv").map(String::as_str), Some("1"));
        assert_eq!(metadata.get("channel").map(String::as_str), Some("dev"));
        assert_eq!(metadata.get("port").map(String::as_str), Some("61234"));
        assert_eq!(
            ReleaseChannel::Dev.bonjour_service_type(),
            "_lumi-remote-dev._tcp"
        );
    }

    #[test]
    fn coalesces_visual_anchors_without_dropping_projection_revisions() -> Result<(), HubError> {
        let mut hub = ProjectionHub::new(1, 8).map_err(|_| HubError::ClientLimitReached)?;
        let client = hub.connect()?;
        hub.publish_projection(projection(1))?;
        for sequence in 1..=100 {
            hub.publish_transport_anchor(
                1,
                RemoteFrame {
                    protocol_version: REMOTE_PROTOCOL_VERSION,
                    frame_kind: RemoteFrameKind::TransportAnchor,
                    sequence,
                    correlation_id: None,
                    payload: json!({ "playerNumber": 1, "beat": sequence }),
                },
            )?;
        }
        let metrics = hub.metrics(client).ok_or(HubError::UnknownClient)?;
        assert_eq!(metrics.critical_depth, 1);
        assert_eq!(metrics.latest_anchor_count, 1);
        assert_eq!(metrics.coalesced_anchor_count, 99);
        assert_eq!(
            hub.next_frame(client)?.map(|frame| frame.frame_kind),
            Some(RemoteFrameKind::Projection)
        );
        assert_eq!(hub.next_frame(client)?.map(|frame| frame.sequence), Some(2));
        Ok(())
    }

    #[test]
    fn four_clients_remain_bounded_during_a_two_player_anchor_storm() -> Result<(), HubError> {
        let mut hub = ProjectionHub::new(4, 8).map_err(|_| HubError::ClientLimitReached)?;
        let clients = (0..4)
            .map(|_| hub.connect())
            .collect::<Result<Vec<_>, _>>()?;
        hub.publish_projection(projection(1))?;

        for sequence in 1..=20_000 {
            let player_number = if sequence % 2 == 0 { 2 } else { 1 };
            hub.publish_transport_anchor(
                player_number,
                RemoteFrame {
                    protocol_version: REMOTE_PROTOCOL_VERSION,
                    frame_kind: RemoteFrameKind::TransportAnchor,
                    sequence,
                    correlation_id: None,
                    payload: json!({ "playerNumber": player_number, "beat": sequence }),
                },
            )?;
        }

        for client in clients {
            let metrics = hub.metrics(client).ok_or(HubError::UnknownClient)?;
            assert_eq!(metrics.critical_depth, 1);
            assert_eq!(metrics.latest_anchor_count, 2);
            assert_eq!(metrics.coalesced_anchor_count, 19_998);
            let delivered = (0..3)
                .map(|_| hub.next_frame(client))
                .collect::<Result<Vec<_>, _>>()?;
            let sequences = delivered
                .into_iter()
                .flatten()
                .map(|frame| frame.sequence)
                .collect::<Vec<_>>();
            assert_eq!(sequences, vec![1, 2, 3]);
        }
        Ok(())
    }

    #[test]
    fn a_new_client_receives_a_complete_snapshot_with_its_own_sequence() -> Result<(), HubError> {
        let mut hub = ProjectionHub::new(2, 8).map_err(|_| HubError::ClientLimitReached)?;
        hub.publish_projection(projection(9))?;
        let client = hub.connect()?;
        let frame = hub.next_frame(client)?.ok_or(HubError::UnknownClient)?;
        assert_eq!(frame.frame_kind, RemoteFrameKind::Snapshot);
        assert_eq!(frame.sequence, 1);
        assert_eq!(frame.payload["projectionRevision"], 9);
        Ok(())
    }

    #[test]
    fn disconnects_a_client_before_its_bounded_critical_queue_can_grow() -> Result<(), HubError> {
        let mut hub = ProjectionHub::new(1, 8).map_err(|_| HubError::ClientLimitReached)?;
        let client = hub.connect()?;
        for revision in 1..=8 {
            assert!(hub.publish_projection(projection(revision))?.is_empty());
        }
        assert_eq!(
            hub.metrics(client).map(|metrics| metrics.critical_depth),
            Some(8)
        );
        assert_eq!(hub.publish_projection(projection(9))?, vec![client]);
        assert!(hub.metrics(client).is_none());
        Ok(())
    }

    #[test]
    fn rejects_mutation_without_matching_controller_lease() {
        let mut policy = GatewayCommandPolicy::default();
        policy.transfer_control("phone-a".to_owned(), "lease-a".to_owned());
        let command = RemoteCommand {
            command_id: "arm".to_owned(),
            controller_lease_id: "lease-b".to_owned(),
            issued_at_unix_millis: 1,
            command: RemoteCommandKind::SetAbletonLinkEnabled {
                enabled: true,
                expected_state_revision: 4,
            },
        };
        assert!(policy.authorize("phone-a", &command).is_err());
    }

    #[test]
    fn repeated_mutating_command_is_admitted_exactly_once() -> Result<(), CommandGuardError> {
        let mut guard = GatewayCommandGuard::new(DEFAULT_COMMAND_LEDGER_CAPACITY)?;
        guard.transfer_control("phone-a".to_owned(), "lease-a".to_owned());
        let command = RemoteCommand {
            command_id: "arm-once".to_owned(),
            controller_lease_id: "lease-a".to_owned(),
            issued_at_unix_millis: 1,
            command: RemoteCommandKind::SetOperationState {
                operation_state: lumi_remote_protocol::OperationTarget::Armed,
                expected_state_revision: 4,
            },
        };
        assert_eq!(
            guard.admit("phone-a", &command)?,
            CommandAdmission::Accepted
        );
        assert_eq!(guard.admit("phone-a", &command)?, CommandAdmission::Pending);
        let forwarded = guard.forwarded_command("phone-a", &command);
        assert_ne!(forwarded.command_id, command.command_id);
        assert!(guard.authorize_forwarded(&forwarded).is_ok());
        guard.record_result(lumi_remote_protocol::RemoteCommandResult {
            command_id: forwarded.command_id.clone(),
            status: lumi_remote_protocol::RemoteCommandResultStatus::Conflict,
            state_revision: Some(8),
            plan_revision: None,
            reason_code: Some("stateRevisionConflict".to_owned()),
        });
        let replay = guard
            .result_for_device("phone-a", &forwarded.command_id)
            .ok_or(CommandGuardError::UnknownCommand)?;
        assert_eq!(replay.command_id, command.command_id);
        assert_eq!(
            replay.status,
            lumi_remote_protocol::RemoteCommandResultStatus::Conflict
        );
        assert_eq!(
            guard.admit("phone-a", &command)?,
            CommandAdmission::Completed(replay)
        );
        assert!(
            guard
                .result_for_device("phone-b", &forwarded.command_id)
                .is_none()
        );
        let mut changed = command.clone();
        changed.issued_at_unix_millis = 2;
        assert!(matches!(
            guard.admit("phone-a", &changed),
            Err(CommandGuardError::CommandIdReused)
        ));
        guard.revoke_control();
        assert!(guard.authorize_forwarded(&forwarded).is_err());
        guard.transfer_control("phone-a".to_owned(), "lease-new".to_owned());
        changed.controller_lease_id = "lease-new".to_owned();
        assert_eq!(
            guard.admit("phone-a", &changed)?,
            CommandAdmission::Accepted
        );
        assert_ne!(
            guard.forwarded_command("phone-a", &changed).command_id,
            forwarded.command_id
        );
        Ok(())
    }

    #[test]
    fn pending_commands_cannot_be_evicted_and_executed_twice() -> Result<(), CommandGuardError> {
        let mut guard = GatewayCommandGuard::new(64)?;
        guard.transfer_control("phone".into(), "lease".into());
        let mut command = RemoteCommand {
            command_id: "0".into(),
            controller_lease_id: "lease".into(),
            issued_at_unix_millis: 1,
            command: RemoteCommandKind::SetOperationState {
                operation_state: lumi_remote_protocol::OperationTarget::Armed,
                expected_state_revision: 1,
            },
        };
        for index in 0..64 {
            command.command_id = index.to_string();
            assert_eq!(guard.admit("phone", &command)?, CommandAdmission::Accepted);
        }
        command.command_id = "overflow".into();
        assert!(matches!(
            guard.admit("phone", &command),
            Err(CommandGuardError::LedgerFull)
        ));
        command.command_id = "0".into();
        assert_eq!(guard.admit("phone", &command)?, CommandAdmission::Pending);
        let forwarded = guard.forwarded_command("phone", &command);
        guard.record_result(lumi_remote_protocol::RemoteCommandResult {
            command_id: forwarded.command_id,
            status: lumi_remote_protocol::RemoteCommandResultStatus::Accepted,
            state_revision: Some(2),
            plan_revision: None,
            reason_code: None,
        });
        assert!(
            matches!(guard.admit("phone", &command)?, CommandAdmission::Completed(result)
            if result.status == lumi_remote_protocol::RemoteCommandResultStatus::Accepted && result.state_revision == Some(2))
        );
        command.command_id = "overflow".into();
        assert_eq!(guard.admit("phone", &command)?, CommandAdmission::Accepted);
        Ok(())
    }

    #[test]
    fn pairing_attempt_limiter_recovers_after_its_window() -> Result<(), RateLimitError> {
        let mut limiter = AttemptRateLimiter::new(2, 1_000)?;
        assert_eq!(limiter.record("peer-a", 1_000), Ok(()));
        assert_eq!(limiter.record("peer-a", 1_100), Ok(()));
        assert_eq!(
            limiter.record("peer-a", 1_200),
            Err(RateLimitError::Limited)
        );
        assert_eq!(limiter.record("peer-a", 2_100), Ok(()));
        Ok(())
    }

    #[test]
    fn limiter_checks_before_work_and_bounds_distinct_identities() -> Result<(), RateLimitError> {
        let mut limiter = AttemptRateLimiter::new(1, 1_000)?;
        limiter.record("peer", 0)?;
        assert_eq!(limiter.check("peer", 1), Err(RateLimitError::Limited));
        for index in 1..1_024 {
            limiter.record(&format!("peer-{index}"), 1)?;
        }
        assert_eq!(limiter.record("overflow", 2), Err(RateLimitError::Limited));
        assert_eq!(limiter.attempts.len(), 1_024);
        limiter.record("fresh", 1_001)?;
        assert_eq!(limiter.attempts.len(), 1);
        Ok(())
    }

    #[test]
    fn pairing_requires_explicit_approval_and_is_single_use() -> Result<(), PairingError> {
        let mut registry = PairingRegistry::default();
        registry.create_invitation(PairingInvitationRequest {
            invitation_id: "invitation-0001".to_owned(),
            invitation_secret: "0123456789abcdef0123456789abcdef".to_owned(),
            short_code: "214730".to_owned(),
            certificate_fingerprint_sha256: "a".repeat(64),
            created_at_unix_millis: 1_000,
            expires_at_unix_millis: 61_000,
        })?;
        let details = registry
            .invitation_details("invitation-0001")
            .ok_or(PairingError::InvitationUnknown)?;
        assert!(!details.approved);
        assert_eq!(
            registry.exchange(
                "invitation-0001",
                "0123456789abcdef0123456789abcdef",
                "phone-1".to_owned(),
                "Booth iPhone".to_owned(),
                "abcdef0123456789abcdef0123456789",
                2_000,
            ),
            Err(PairingError::ApprovalRequired)
        );

        // A phone may arrive before the Mac user has compared and approved
        // the code. The invitation remains pending, but it becomes one-use
        // immediately after one approved successful exchange.
        registry.approve("invitation-0001", "214730")?;
        registry.exchange(
            "invitation-0001",
            "0123456789abcdef0123456789abcdef",
            "phone-1".to_owned(),
            "Booth iPhone".to_owned(),
            "abcdef0123456789abcdef0123456789",
            2_000,
        )?;
        assert_eq!(
            registry.exchange(
                "invitation-0001",
                "0123456789abcdef0123456789abcdef",
                "phone-2".to_owned(),
                "Other iPhone".to_owned(),
                "abcdef0123456789abcdef0123456789",
                2_001,
            ),
            Err(PairingError::InvitationUnknown)
        );
        Ok(())
    }

    #[test]
    fn revoked_device_can_no_longer_authenticate() -> Result<(), PairingError> {
        let mut registry = PairingRegistry::default();
        registry.create_invitation(PairingInvitationRequest {
            invitation_id: "invitation-0002".to_owned(),
            invitation_secret: "0123456789abcdef0123456789abcdef".to_owned(),
            short_code: "214730".to_owned(),
            certificate_fingerprint_sha256: "b".repeat(64),
            created_at_unix_millis: 1_000,
            expires_at_unix_millis: 61_000,
        })?;
        registry.approve("invitation-0002", "214730")?;
        registry.exchange(
            "invitation-0002",
            "0123456789abcdef0123456789abcdef",
            "phone-1".to_owned(),
            "Booth iPhone".to_owned(),
            "abcdef0123456789abcdef0123456789",
            2_000,
        )?;
        assert_eq!(
            registry.authenticate("phone-1", "abcdef0123456789abcdef0123456789", 3_000),
            Ok(())
        );
        assert!(registry.revoke("phone-1"));
        assert_eq!(
            registry.authenticate("phone-1", "abcdef0123456789abcdef0123456789", 4_000),
            Err(PairingError::DeviceUnknown)
        );
        Ok(())
    }
}
