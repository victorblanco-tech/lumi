//! CoreMIDI adapter for Lumi's provider-neutral MIDI source port.

#![forbid(unsafe_code)]

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub use macos::CoreMidiSourceProvider;
#[cfg(not(target_os = "macos"))]
pub use unsupported::CoreMidiSourceProvider;
