use coremidi::{
    EventListBuffer, MidiClient, MidiError, MidiObject, MidiProperty, MidiProtocol, VirtualSource,
};
use lumi_midi_output::{MidiMessage, MidiSourceProvider};

const LUMI_COREMIDI_UNIQUE_ID: i32 = 0x4c55_4d49;

#[derive(Default)]
pub struct CoreMidiSourceProvider {
    // The endpoint must be disposed before the client that owns it.
    source: Option<VirtualSource>,
    client: Option<MidiClient>,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreMidiError {
    #[error("the virtual MIDI source is not published")]
    SourceNotPublished,
    #[error("CoreMIDI failed: {0}")]
    CoreMidi(#[from] MidiError),
}

impl CoreMidiSourceProvider {
    pub const fn new() -> Self {
        Self {
            source: None,
            client: None,
        }
    }
}

impl MidiSourceProvider for CoreMidiSourceProvider {
    type Error = CoreMidiError;

    fn publish(&mut self, source_name: &str) -> Result<(), Self::Error> {
        if self.source.is_some() {
            return Ok(());
        }
        let client = MidiClient::new("Lumi MIDI Engine")?;
        let source = client.virtual_source_with_protocol(source_name, MidiProtocol::Midi1)?;
        source.set_integer_property(MidiProperty::unique_id(), LUMI_COREMIDI_UNIQUE_ID)?;
        self.source = Some(source);
        self.client = Some(client);
        Ok(())
    }

    fn stop(&mut self) {
        self.source = None;
        self.client = None;
    }

    fn send(&mut self, messages: &[MidiMessage]) -> Result<(), Self::Error> {
        let source = self
            .source
            .as_ref()
            .ok_or(CoreMidiError::SourceNotPublished)?;
        let mut events = EventListBuffer::with_capacity(MidiProtocol::Midi1, 128);
        for message in messages {
            events.add_packet_words(0, &[midi_one_ump_word(message.bytes())])?;
        }
        source.received_event_list(&events)?;
        Ok(())
    }
}

fn midi_one_ump_word(bytes: [u8; 3]) -> u32 {
    (0x2_u32 << 28) | (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_one_channel_voice_bytes_encode_as_ump() {
        assert_eq!(midi_one_ump_word([0x9f, 60, 100]), 0x209f_3c64);
        assert_eq!(midi_one_ump_word([0x8f, 60, 0]), 0x208f_3c00);
    }
}
