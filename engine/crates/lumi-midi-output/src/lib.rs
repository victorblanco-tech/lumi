//! Provider-neutral MIDI source control and fail-silent POC sequencing.

#![forbid(unsafe_code)]

use std::time::Duration;

pub const POC_SOURCE_NAME: &str = "Lumi Virtual MIDI";
pub const POC_MIDI_CHANNEL: u8 = 16;
const POC_MIDI_CHANNEL_ZERO_BASED: u8 = POC_MIDI_CHANNEL - 1;
const BANK_NOTE_BASE: u8 = 60;
const AUTOLOOP_NOTE_BASE: u8 = 64;
pub const POC_BANK_SETTLE_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiPocAddressKind {
    Bank,
    Autoloop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiPocAddress {
    kind: MidiPocAddressKind,
    number: u8,
    note: u8,
}

impl MidiPocAddress {
    pub const BANK_ONE: Self = Self {
        kind: MidiPocAddressKind::Bank,
        number: 1,
        note: BANK_NOTE_BASE,
    };

    pub const fn bank(number: u8) -> Option<Self> {
        if number >= 1 && number <= 4 {
            Some(Self {
                kind: MidiPocAddressKind::Bank,
                number,
                note: BANK_NOTE_BASE + number - 1,
            })
        } else {
            None
        }
    }

    pub const fn autoloop(number: u8) -> Option<Self> {
        if number >= 1 && number <= 32 {
            Some(Self {
                kind: MidiPocAddressKind::Autoloop,
                number,
                note: AUTOLOOP_NOTE_BASE + number - 1,
            })
        } else {
            None
        }
    }

    pub const fn kind(self) -> MidiPocAddressKind {
        self.kind
    }

    pub const fn number(self) -> u8 {
        self.number
    }

    pub const fn note(self) -> u8 {
        self.note
    }
}

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
    #[error("the requested bank or AutoLoop address is invalid")]
    InvalidAddress,
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
        self.send_address_learn_pulse(MidiPocAddress::BANK_ONE)
    }

    pub fn send_address_learn_pulse(
        &mut self,
        address: MidiPocAddress,
    ) -> Result<(), MidiPocError<P::Error>> {
        self.send_address_pulse(address)?;
        let target = match address.kind() {
            MidiPocAddressKind::Bank => "Bank",
            MidiPocAddressKind::Autoloop => "AutoLoop",
        };
        self.last_event = Some(format!(
            "{target} {} learn pulse sent · Ch {} · Note {} · Note Off included",
            address.number(),
            POC_MIDI_CHANNEL,
            address.note()
        ));
        Ok(())
    }

    pub fn trigger_autoloop(
        &mut self,
        bank_number: u8,
        autoloop_number: u8,
    ) -> Result<(), MidiPocError<P::Error>> {
        self.trigger_autoloop_with_wait(bank_number, autoloop_number, std::thread::sleep)
    }

    fn trigger_autoloop_with_wait<F>(
        &mut self,
        bank_number: u8,
        autoloop_number: u8,
        wait: F,
    ) -> Result<(), MidiPocError<P::Error>>
    where
        F: FnOnce(Duration),
    {
        let bank = MidiPocAddress::bank(bank_number).ok_or(MidiPocError::InvalidAddress)?;
        let autoloop =
            MidiPocAddress::autoloop(autoloop_number).ok_or(MidiPocError::InvalidAddress)?;
        self.send_address_pulse(bank)?;
        self.last_event = Some(format!(
            "Bank {bank_number} selected · waiting {} ms for SoundSwitch",
            POC_BANK_SETTLE_DELAY.as_millis()
        ));
        wait(POC_BANK_SETTLE_DELAY);
        self.send_address_pulse(autoloop)?;
        self.last_event = Some(format!(
            "Triggered Bank {bank_number} → AutoLoop {autoloop_number} · Ch {POC_MIDI_CHANNEL} · Notes {} → {} · {} ms gap",
            bank.note(),
            autoloop.note(),
            POC_BANK_SETTLE_DELAY.as_millis()
        ));
        Ok(())
    }

    fn send_address_pulse(
        &mut self,
        address: MidiPocAddress,
    ) -> Result<(), MidiPocError<P::Error>> {
        if self.state != MidiSourceState::Ready {
            return Err(MidiPocError::SourceNotPublished);
        }
        let note_on = MidiMessage::note_on(POC_MIDI_CHANNEL_ZERO_BASED, address.note(), 100)
            .ok_or(MidiPocError::SourceNotPublished)?;
        let note_off = MidiMessage::note_off(POC_MIDI_CHANNEL_ZERO_BASED, address.note())
            .ok_or(MidiPocError::SourceNotPublished)?;
        let next_count = self
            .sent_pulse_count
            .checked_add(1)
            .ok_or(MidiPocError::PulseCounterOverflow)?;
        self.provider
            .send(&[note_on, note_off])
            .map_err(MidiPocError::Provider)?;
        self.sent_pulse_count = next_count;
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

    #[test]
    fn four_banks_and_thirty_two_autoloops_have_stable_unique_notes() {
        let bank_notes = (1..=4)
            .filter_map(MidiPocAddress::bank)
            .map(MidiPocAddress::note)
            .collect::<Vec<_>>();
        let autoloop_notes = (1..=32)
            .filter_map(MidiPocAddress::autoloop)
            .map(MidiPocAddress::note)
            .collect::<Vec<_>>();

        assert_eq!(bank_notes, [60, 61, 62, 63]);
        assert_eq!(autoloop_notes.first(), Some(&64));
        assert_eq!(autoloop_notes.last(), Some(&95));
        assert!(bank_notes.iter().all(|note| !autoloop_notes.contains(note)));
    }

    #[test]
    fn address_learn_pulse_uses_the_requested_autoloop_note() {
        let mut controller = MidiPocController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        let Some(address) = MidiPocAddress::autoloop(32) else {
            panic!("AutoLoop 32 must have a POC address");
        };
        assert!(controller.send_address_learn_pulse(address).is_ok());

        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 95, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 95, 0]);
    }

    #[test]
    fn runtime_trigger_selects_bank_then_autoloop_after_settle_delay() {
        let mut controller = MidiPocController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        let mut observed_delay = None;

        assert!(
            controller
                .trigger_autoloop_with_wait(1, 1, |delay| observed_delay = Some(delay))
                .is_ok()
        );

        assert_eq!(observed_delay, Some(POC_BANK_SETTLE_DELAY));
        assert_eq!(controller.provider.messages.len(), 4);
        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 60, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 60, 0]);
        assert_eq!(controller.provider.messages[2].bytes(), [0x9f, 64, 100]);
        assert_eq!(controller.provider.messages[3].bytes(), [0x8f, 64, 0]);
        assert_eq!(controller.status().sent_pulse_count, 2);
        assert!(
            controller
                .status()
                .last_event
                .as_deref()
                .is_some_and(|event| event.contains("Bank 1 → AutoLoop 1"))
        );
    }

    #[test]
    fn runtime_trigger_is_fail_silent_until_source_is_published() {
        let mut controller = MidiPocController::new(RecordingProvider::default());

        assert!(matches!(
            controller.trigger_autoloop_with_wait(1, 1, |_| {}),
            Err(MidiPocError::SourceNotPublished)
        ));
        assert!(controller.provider.messages.is_empty());
        assert_eq!(controller.status().sent_pulse_count, 0);
    }
}
