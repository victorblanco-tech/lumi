use lumi_midi_output::{MidiMessage, MidiSourceProvider};

#[derive(Default)]
pub struct CoreMidiSourceProvider;

#[derive(Debug, thiserror::Error)]
#[error("CoreMIDI is only available on macOS")]
pub struct CoreMidiError;

impl CoreMidiSourceProvider {
    pub const fn new() -> Self {
        Self
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
