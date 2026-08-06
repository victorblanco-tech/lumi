//! CoreMIDI adapter for Lumi's provider-neutral MIDI source port.

#![forbid(unsafe_code)]

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub use macos::{CoreMidiDestinationProvider, CoreMidiSourceProvider};
#[cfg(not(target_os = "macos"))]
pub use unsupported::{CoreMidiDestinationProvider, CoreMidiSourceProvider};

pub const DECK_INPUT_DESTINATION_NAME: &str = "Lumi Deck Input";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiChannelVoiceMessage {
    pub status: u8,
    pub channel: u8,
    pub data_one: u8,
    pub data_two: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiDestinationStatus {
    pub state: MidiDestinationState,
    pub destination_name: Option<String>,
    pub received_message_count: u64,
    pub invalid_word_count: u64,
    pub last_message: Option<MidiChannelVoiceMessage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiDestinationState {
    Stopped,
    Ready,
}
