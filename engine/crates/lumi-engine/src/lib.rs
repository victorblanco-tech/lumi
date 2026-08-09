//! Lumi's local autonomous engine process.

#![forbid(unsafe_code)]

mod autoloop_defaults;
mod commands;
mod library;
mod phrase_role_defaults;
mod session;
mod startup;

pub use session::{EngineError, run};
pub use startup::StartupReady;
