use lumi_midi_output::{MidiMessage, MidiSourceProvider};

use crate::{MidiChannelVoiceMessage, MidiDestinationState, MidiDestinationStatus};

#[derive(Default)]
pub struct CoreMidiSourceProvider;

#[derive(Default)]
pub struct CoreMidiDestinationProvider;

#[derive(Debug, thiserror::Error)]
#[error("CoreMIDI is only available on macOS")]
pub struct CoreMidiError;

impl CoreMidiSourceProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl CoreMidiDestinationProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn publish(&mut self, _destination_name: &str) -> Result<(), CoreMidiError> {
        Err(CoreMidiError)
    }

    pub fn stop(&mut self) {}

    pub fn drain_messages(&mut self) -> Vec<MidiChannelVoiceMessage> {
        Vec::new()
    }

    pub fn status(&self) -> MidiDestinationStatus {
        MidiDestinationStatus {
            state: MidiDestinationState::Stopped,
            destination_name: None,
            received_message_count: 0,
            invalid_word_count: 0,
            last_message: None,
        }
    }
}

impl MidiSourceProvider for CoreMidiSourceProvider {
    type Error = CoreMidiError;

    fn publish(&mut self, _source_name: &str) -> Result<(), Self::Error> {
        Err(CoreMidiError)
    }

    fn stop(&mut self) {}

    fn send(&mut self, _messages: &[MidiMessage]) -> Result<(), Self::Error> {
        Err(CoreMidiError)
    }
}
