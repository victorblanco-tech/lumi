//! Public, LAN-safe contract for Lumi Remote clients.
//!
//! This crate intentionally does not reuse the internal engine envelope. Only
//! presentation-safe Live state and explicitly booth-safe mutations belong in
//! this boundary.

#![forbid(unsafe_code)]

mod auth;
mod command;
mod frame;
mod projection;

pub use command::{
    OperationTarget, RemoteCommand, RemoteCommandError, RemoteCommandKind, RemoteCommandResult,
    RemoteCommandResultStatus,
};
pub use frame::{
    MAX_REMOTE_FRAME_BYTES, REMOTE_PROTOCOL_VERSION, RemoteFrame, RemoteFrameError, RemoteFrameKind,
};
pub use projection::{
    IntegrationHealth, OperationState, ProjectionError, RemoteAutoloopChoice, RemoteBeatGrid,
    RemoteHotCue, RemoteIntegrationStatus, RemoteLightPlan, RemoteLiveProjection, RemotePhrase,
    RemotePhraseRoleOption, RemotePlanCue, RemotePlayer, RemoteThemeOption, RemoteTrack,
    RemoteTransportAnchor, RemoteWaveformPoint,
};

#[cfg(test)]
mod contract_tests;
pub use auth::{RemoteAuthenticationError, RemoteClientHello, RemoteServerHello};
