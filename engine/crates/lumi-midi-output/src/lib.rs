//! Provider-neutral MIDI source control and fail-silent SoundSwitch sequencing.

#![forbid(unsafe_code)]

use std::time::Duration;

pub const MIDI_SOURCE_NAME: &str = "Lumi Virtual MIDI";
pub const MIDI_CHANNEL: u8 = 16;
const MIDI_CHANNEL_ZERO_BASED: u8 = MIDI_CHANNEL - 1;
const BANK_NOTE_BASE: u8 = 60;
const AUTOLOOP_NOTE_BASE: u8 = 64;
pub const BANK_SETTLE_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiAddressKind {
    Bank,
    Autoloop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiAddress {
    kind: MidiAddressKind,
    number: u8,
    note: u8,
}

impl MidiAddress {
    pub const BANK_ONE: Self = Self {
        kind: MidiAddressKind::Bank,
        number: 1,
        note: BANK_NOTE_BASE,
    };

    pub const fn bank(number: u8) -> Option<Self> {
        if number >= 1 && number <= 4 {
            Some(Self {
                kind: MidiAddressKind::Bank,
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
                kind: MidiAddressKind::Autoloop,
                number,
                note: AUTOLOOP_NOTE_BASE + number - 1,
            })
        } else {
            None
        }
    }

    pub const fn kind(self) -> MidiAddressKind {
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
pub enum MidiOutputError<E>
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

pub struct MidiOutputController<P>
where
    P: MidiSourceProvider,
{
    provider: P,
    state: MidiSourceState,
    sent_pulse_count: u64,
    last_event: Option<String>,
}

impl<P> MidiOutputController<P>
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

    pub fn publish(&mut self) -> Result<(), MidiOutputError<P::Error>> {
        self.provider
            .publish(MIDI_SOURCE_NAME)
            .map_err(MidiOutputError::Provider)?;
        self.state = MidiSourceState::Ready;
        self.last_event = Some("Virtual MIDI source published; no MIDI sent".to_owned());
        Ok(())
    }

    pub fn stop(&mut self) {
        self.provider.stop();
        self.state = MidiSourceState::Stopped;
        self.last_event = Some("Virtual MIDI source stopped".to_owned());
    }

    pub fn send_learn_pulse(&mut self) -> Result<(), MidiOutputError<P::Error>> {
        self.send_address_learn_pulse(MidiAddress::BANK_ONE)
    }

    pub fn send_address_learn_pulse(
        &mut self,
        address: MidiAddress,
    ) -> Result<(), MidiOutputError<P::Error>> {
        self.send_address_pulse(address)?;
        let target = match address.kind() {
            MidiAddressKind::Bank => "Bank",
            MidiAddressKind::Autoloop => "AutoLoop",
        };
        self.last_event = Some(format!(
            "{target} {} learn pulse sent · Ch {} · Note {} · Note Off included",
            address.number(),
            MIDI_CHANNEL,
            address.note()
        ));
        Ok(())
    }

    pub fn trigger_autoloop(
        &mut self,
        bank_number: u8,
        autoloop_number: u8,
    ) -> Result<(), MidiOutputError<P::Error>> {
        self.trigger_autoloop_with_wait(bank_number, autoloop_number, std::thread::sleep)
    }

    fn trigger_autoloop_with_wait<F>(
        &mut self,
        bank_number: u8,
        autoloop_number: u8,
        wait: F,
    ) -> Result<(), MidiOutputError<P::Error>>
    where
        F: FnOnce(Duration),
    {
        let bank = MidiAddress::bank(bank_number).ok_or(MidiOutputError::InvalidAddress)?;
        let autoloop =
            MidiAddress::autoloop(autoloop_number).ok_or(MidiOutputError::InvalidAddress)?;
        self.send_address_pulse(bank)?;
        self.last_event = Some(format!(
            "Bank {bank_number} selected · waiting {} ms for SoundSwitch",
            BANK_SETTLE_DELAY.as_millis()
        ));
        wait(BANK_SETTLE_DELAY);
        self.send_address_pulse(autoloop)?;
        self.last_event = Some(format!(
            "Triggered Bank {bank_number} → AutoLoop {autoloop_number} · Ch {MIDI_CHANNEL} · Notes {} → {} · {} ms gap",
            bank.note(),
            autoloop.note(),
            BANK_SETTLE_DELAY.as_millis()
        ));
        Ok(())
    }

    fn send_address_pulse(
        &mut self,
        address: MidiAddress,
    ) -> Result<(), MidiOutputError<P::Error>> {
        if self.state != MidiSourceState::Ready {
            return Err(MidiOutputError::SourceNotPublished);
        }
        let note_on = MidiMessage::note_on(MIDI_CHANNEL_ZERO_BASED, address.note(), 100)
            .ok_or(MidiOutputError::SourceNotPublished)?;
        let note_off = MidiMessage::note_off(MIDI_CHANNEL_ZERO_BASED, address.note())
            .ok_or(MidiOutputError::SourceNotPublished)?;
        let next_count = self
            .sent_pulse_count
            .checked_add(1)
            .ok_or(MidiOutputError::PulseCounterOverflow)?;
        self.provider
            .send(&[note_on, note_off])
            .map_err(MidiOutputError::Provider)?;
        self.sent_pulse_count = next_count;
        Ok(())
    }

    pub fn status(&self) -> MidiSourceStatus {
        MidiSourceStatus {
            state: self.state,
            source_name: MIDI_SOURCE_NAME,
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
            self.published = source_name == MIDI_SOURCE_NAME;
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
        let mut controller = MidiOutputController::new(RecordingProvider::default());

        assert!(matches!(
            controller.send_learn_pulse(),
            Err(MidiOutputError::SourceNotPublished)
        ));
        assert_eq!(controller.status().sent_pulse_count, 0);
    }

    #[test]
    fn learn_pulse_always_contains_note_on_and_note_off() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
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
            .filter_map(MidiAddress::bank)
            .map(MidiAddress::note)
            .collect::<Vec<_>>();
        let autoloop_notes = (1..=32)
            .filter_map(MidiAddress::autoloop)
            .map(MidiAddress::note)
            .collect::<Vec<_>>();

        assert_eq!(bank_notes, [60, 61, 62, 63]);
        assert_eq!(autoloop_notes.first(), Some(&64));
        assert_eq!(autoloop_notes.last(), Some(&95));
        assert!(bank_notes.iter().all(|note| !autoloop_notes.contains(note)));
    }

    #[test]
    fn address_learn_pulse_uses_the_requested_autoloop_note() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        let Some(address) = MidiAddress::autoloop(32) else {
            panic!("AutoLoop 32 must have a MIDI address");
        };
        assert!(controller.send_address_learn_pulse(address).is_ok());

        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 95, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 95, 0]);
    }

    #[test]
    fn runtime_trigger_selects_bank_then_autoloop_after_settle_delay() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        let mut observed_delay = None;

        assert!(
            controller
                .trigger_autoloop_with_wait(1, 1, |delay| observed_delay = Some(delay))
                .is_ok()
        );

        assert_eq!(observed_delay, Some(BANK_SETTLE_DELAY));
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
        let mut controller = MidiOutputController::new(RecordingProvider::default());

        assert!(matches!(
            controller.trigger_autoloop_with_wait(1, 1, |_| {}),
            Err(MidiOutputError::SourceNotPublished)
        ));
        assert!(controller.provider.messages.is_empty());
        assert_eq!(controller.status().sent_pulse_count, 0);
    }
}
