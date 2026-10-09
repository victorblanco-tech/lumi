//! Lumi's local autonomous engine process.

#![forbid(unsafe_code)]

mod autoloop_defaults;
mod autoloop_executor;
mod commands;
mod library;
pub mod launch_policy;
mod link_relay;
mod live_library_resolver;
mod media_resolver;
mod phrase_role_defaults;
mod remote_ipc;
mod service;
mod session;
mod startup;
mod timing_preferences;
mod usb_media_identity;
mod usb_worker;

pub use session::{EngineError, run};
pub use startup::StartupReady;
pub use usb_worker::{UsbWorkerError, run_usb_worker};
