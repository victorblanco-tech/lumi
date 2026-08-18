use coremidi::{
    MidiClient, MidiError, MidiObject, MidiProperty, MidiProtocol, MidiVirtualDestinationStream,
    PacketListBuffer, VirtualSource,
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
        // SoundSwitch still consumes CoreMIDI through the legacy MIDI 1.0
        // packet API. A UMP-backed source is discoverable and can initially
        // deliver events, but SoundSwitch may silently stop consuming it while
        // the endpoint itself remains healthy. Publish a classic MIDI 1.0
        // source so the endpoint and delivery path match physical controllers,
        // IAC buses and Beat Link Trigger.
        let source = client.virtual_source(source_name)?;
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
        let packets = midi_one_packet_list(messages)?;
        source.received(&packets)?;
        Ok(())
    }
}

fn midi_one_packet_list(messages: &[MidiMessage]) -> Result<PacketListBuffer, MidiError> {
    let mut packets = PacketListBuffer::with_capacity(128);
    for (index, message) in messages.iter().enumerate() {
        // Distinct timestamps prevent CoreMIDI from coalescing Note On and
        // Note Off into one packet. These tiny absolute values are in the past
        // and therefore both packets remain immediate.
        let timestamp = u64::try_from(index).unwrap_or(u64::MAX);
        packets.add_packet(timestamp, &message.bytes())?;
    }
    Ok(packets)
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
    fn source_messages_are_separate_legacy_midi_one_packets() {
        let messages = [
            MidiMessage::note_on(15, 60, 100).unwrap_or_else(|| panic!("valid note on")),
            MidiMessage::note_off(15, 60).unwrap_or_else(|| panic!("valid note off")),
        ];
        let packets = midi_one_packet_list(&messages)
            .unwrap_or_else(|error| panic!("packet list must encode: {error}"));
        let bytes = packets
            .as_packet_list()
            .iter()
            .map(|packet| packet.bytes().to_vec())
            .collect::<Vec<_>>();
        assert_eq!(bytes, vec![vec![0x9f, 60, 100], vec![0x8f, 60, 0]]);
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
