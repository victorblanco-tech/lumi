use coremidi::{
    EventListBuffer, MidiClient, MidiError, MidiObject, MidiProperty, MidiProtocol,
    MidiVirtualDestinationStream, VirtualSource,
};
use lumi_midi_output::{MIDI_CLOCK_SOURCE_NAME, MidiMessage, MidiSourceProvider};

use crate::{MidiChannelVoiceMessage, MidiDestinationState, MidiDestinationStatus};

const LUMI_COREMIDI_UNIQUE_ID: i32 = 0x4c55_4d49;
const LUMI_CLOCK_COREMIDI_UNIQUE_ID: i32 = 0x4c55_4d4a;
const DESTINATION_BUFFER_CAPACITY: usize = 256;

#[derive(Default)]
pub struct CoreMidiSourceProvider {
    // The endpoint must be disposed before the client that owns it.
    source: Option<VirtualSource>,
    client: Option<MidiClient>,
}

#[derive(Default)]
pub struct CoreMidiDestinationProvider {
    // The endpoint must be disposed before the client that owns it.
    destination: Option<MidiVirtualDestinationStream>,
    client: Option<MidiClient>,
    destination_name: Option<String>,
    received_message_count: u64,
    invalid_word_count: u64,
    last_message: Option<MidiChannelVoiceMessage>,
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

impl CoreMidiDestinationProvider {
    pub const fn new() -> Self {
        Self {
            destination: None,
            client: None,
            destination_name: None,
            received_message_count: 0,
            invalid_word_count: 0,
            last_message: None,
        }
    }

    pub fn publish(&mut self, destination_name: &str) -> Result<(), CoreMidiError> {
        if self.destination.is_some() {
            return Ok(());
        }
        let client = MidiClient::new("Lumi Deck Input Engine")?;
        let destination = MidiVirtualDestinationStream::create(
            client.raw(),
            destination_name,
            MidiProtocol::Midi1,
            DESTINATION_BUFFER_CAPACITY,
        )?;
        self.destination = Some(destination);
        self.client = Some(client);
        self.destination_name = Some(destination_name.to_owned());
        Ok(())
    }

    pub fn stop(&mut self) {
        self.destination = None;
        self.client = None;
        self.destination_name = None;
    }

    pub fn drain_messages(&mut self) -> Vec<MidiChannelVoiceMessage> {
        let mut messages = Vec::new();
        let Some(destination) = self.destination.as_ref() else {
            return messages;
        };
        while let Some(event_list) = destination.try_next() {
            for packet in event_list.packets {
                let word_count = usize::try_from(packet.wordCount).unwrap_or(0).min(64);
                for word in packet.words.into_iter().take(word_count) {
                    if let Some(message) = decode_midi_one_ump_word(word) {
                        self.received_message_count = self.received_message_count.saturating_add(1);
                        self.last_message = Some(message);
                        messages.push(message);
                    } else {
                        self.invalid_word_count = self.invalid_word_count.saturating_add(1);
                    }
                }
            }
        }
        messages
    }

    pub fn status(&self) -> MidiDestinationStatus {
        MidiDestinationStatus {
            state: if self.destination.is_some() {
                MidiDestinationState::Ready
            } else {
                MidiDestinationState::Stopped
            },
            destination_name: self.destination_name.clone(),
            received_message_count: self.received_message_count,
            invalid_word_count: self.invalid_word_count,
            last_message: self.last_message,
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
        let unique_id = if source_name == MIDI_CLOCK_SOURCE_NAME {
            LUMI_CLOCK_COREMIDI_UNIQUE_ID
        } else {
            LUMI_COREMIDI_UNIQUE_ID
        };
        source.set_integer_property(MidiProperty::unique_id(), unique_id)?;
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
    let message_type = if bytes[0] >= 0xf0 { 0x1_u32 } else { 0x2_u32 };
    (message_type << 28)
        | (u32::from(bytes[0]) << 16)
        | (u32::from(bytes[1]) << 8)
        | u32::from(bytes[2])
}

fn decode_midi_one_ump_word(word: u32) -> Option<MidiChannelVoiceMessage> {
    if word >> 28 != 0x2 {
        return None;
    }
    let status_byte = u8::try_from((word >> 16) & 0xff).ok()?;
    let status = status_byte >> 4;
    if !(0x8..=0xe).contains(&status) {
        return None;
    }
    Some(MidiChannelVoiceMessage {
        status,
        channel: (status_byte & 0x0f).saturating_add(1),
        data_one: u8::try_from((word >> 8) & 0x7f).ok()?,
        data_two: u8::try_from(word & 0x7f).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_one_channel_voice_bytes_encode_as_ump() {
        assert_eq!(midi_one_ump_word([0x9f, 60, 100]), 0x209f_3c64);
        assert_eq!(midi_one_ump_word([0x8f, 60, 0]), 0x208f_3c00);
    }

    #[test]
    fn midi_one_system_realtime_bytes_encode_as_system_ump() {
        assert_eq!(midi_one_ump_word(MidiMessage::clock().bytes()), 0x10f8_0000);
        assert_eq!(midi_one_ump_word(MidiMessage::start().bytes()), 0x10fa_0000);
    }

    #[test]
    fn midi_one_ump_word_decodes_channel_voice_message() {
        assert_eq!(
            decode_midi_one_ump_word(0x20b1_107f),
            Some(MidiChannelVoiceMessage {
                status: 0xb,
                channel: 2,
                data_one: 16,
                data_two: 127,
            })
        );
        assert_eq!(decode_midi_one_ump_word(0x1090_3c64), None);
    }
}
