//! Provider-neutral MIDI source control and fail-silent POC sequencing.

#![forbid(unsafe_code)]

pub const POC_SOURCE_NAME: &str = "Lumi Virtual MIDI";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiMessage([u8; 3]);

impl MidiMessage {
    pub const fn note_on(channel: u8, note: u8, velocity: u8) -> Option<Self> {
        if channel < 16 && note < 128 && velocity < 128 {
            Some(Self([0x90 | channel, note, velocity]))
        } else {
            None
        }
    }

    pub const fn note_off(channel: u8, note: u8) -> Option<Self> {
        if channel < 16 && note < 128 {
            Some(Self([0x80 | channel, note, 0]))
        } else {
            None
        }
    }

    pub const fn bytes(self) -> [u8; 3] {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiSourceStatus {
    pub state: MidiSourceState,
    pub source_name: &'static str,
    pub sent_pulse_count: u64,
    pub last_event: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiSourceState {
    Stopped,
    Ready,
}

pub trait MidiSourceProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn publish(&mut self, source_name: &str) -> Result<(), Self::Error>;
    fn stop(&mut self);
    fn send(&mut self, messages: &[MidiMessage]) -> Result<(), Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum MidiPocError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("the virtual MIDI source is not published")]
    SourceNotPublished,
    #[error("the MIDI pulse counter overflowed")]
    PulseCounterOverflow,
    #[error("the MIDI provider failed: {0}")]
    Provider(E),
}

pub struct MidiPocController<P>
where
    P: MidiSourceProvider,
{
    provider: P,
    state: MidiSourceState,
    sent_pulse_count: u64,
    last_event: Option<String>,
}

impl<P> MidiPocController<P>
where
    P: MidiSourceProvider,
{
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            state: MidiSourceState::Stopped,
            sent_pulse_count: 0,
            last_event: None,
        }
    }

    pub fn publish(&mut self) -> Result<(), MidiPocError<P::Error>> {
        self.provider
            .publish(POC_SOURCE_NAME)
            .map_err(MidiPocError::Provider)?;
        self.state = MidiSourceState::Ready;
        self.last_event = Some("Virtual MIDI source published; no MIDI sent".to_owned());
        Ok(())
    }

    pub fn stop(&mut self) {
        self.provider.stop();
        self.state = MidiSourceState::Stopped;
        self.last_event = Some("Virtual MIDI source stopped".to_owned());
    }

    pub fn send_learn_pulse(&mut self) -> Result<(), MidiPocError<P::Error>> {
        if self.state != MidiSourceState::Ready {
            return Err(MidiPocError::SourceNotPublished);
        }
        let note_on = MidiMessage::note_on(15, 60, 100).ok_or(MidiPocError::SourceNotPublished)?;
        let note_off = MidiMessage::note_off(15, 60).ok_or(MidiPocError::SourceNotPublished)?;
        let next_count = self
            .sent_pulse_count
            .checked_add(1)
            .ok_or(MidiPocError::PulseCounterOverflow)?;
        self.provider
            .send(&[note_on, note_off])
            .map_err(MidiPocError::Provider)?;
        self.sent_pulse_count = next_count;
        self.last_event = Some("Learn pulse sent · Ch 16 · Note 60 · Note Off included".to_owned());
        Ok(())
    }

    pub fn status(&self) -> MidiSourceStatus {
        MidiSourceStatus {
            state: self.state,
            source_name: POC_SOURCE_NAME,
            sent_pulse_count: self.sent_pulse_count,
            last_event: self.last_event.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingProvider {
        published: bool,
        messages: Vec<MidiMessage>,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("recording provider failed")]
    struct RecordingError;

    impl MidiSourceProvider for RecordingProvider {
        type Error = RecordingError;

        fn publish(&mut self, source_name: &str) -> Result<(), Self::Error> {
            self.published = source_name == POC_SOURCE_NAME;
            Ok(())
        }

        fn stop(&mut self) {
            self.published = false;
        }

        fn send(&mut self, messages: &[MidiMessage]) -> Result<(), Self::Error> {
            self.messages.extend_from_slice(messages);
            Ok(())
        }
    }

    #[test]
    fn learn_pulse_is_fail_silent_until_published() {
        let mut controller = MidiPocController::new(RecordingProvider::default());

        assert!(matches!(
            controller.send_learn_pulse(),
            Err(MidiPocError::SourceNotPublished)
        ));
        assert_eq!(controller.status().sent_pulse_count, 0);
    }

    #[test]
    fn learn_pulse_always_contains_note_on_and_note_off() {
        let mut controller = MidiPocController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        assert!(controller.send_learn_pulse().is_ok());

        assert_eq!(controller.provider.messages.len(), 2);
        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 60, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 60, 0]);
        assert_eq!(controller.status().sent_pulse_count, 1);
    }
}
