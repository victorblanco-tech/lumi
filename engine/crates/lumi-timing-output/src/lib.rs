//! Provider-neutral musical timing output and managed Ableton Link adapter.

#![forbid(unsafe_code)]

use std::error::Error;
use std::time::Instant;

mod carabiner;

pub use carabiner::{
    CARABINER_DEFAULT_PORT, CARABINER_EXPECTED_VERSION, CarabinerConfiguration,
    CarabinerTimingOutput,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingSourceKind {
    LocalPlayback,
    ProDjLink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingDiscontinuity {
    Continuous,
    Started,
    Resumed,
    Seeked,
    TrackChanged,
    MasterChanged,
}

/// One immutable clock observation from the selected musical timing source.
///
/// `observed_at_micros` is populated when the source and Link helper share a
/// verified monotonic epoch, as they do for the Java Pro DJ Link bridge on
/// macOS. The contract deliberately contains no track position, phrase,
/// AutoLoop, Hot Cue, seek or show-generation state. Those belong to Lumi's
/// independent show executor and can never request a Link timeline change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkClockObservation {
    pub source: TimingSourceKind,
    pub deck_number: Option<u8>,
    pub bpm_milli: u32,
    pub beat_within_bar: u8,
    pub playing: bool,
    pub observed_at_micros: Option<u64>,
}

impl LinkClockObservation {
    pub fn validate(self) -> Result<Self, TimingOutputValidationError> {
        if !(20_000..=300_000).contains(&self.bpm_milli) {
            return Err(TimingOutputValidationError::Tempo);
        }
        if !(1..=4).contains(&self.beat_within_bar) {
            return Err(TimingOutputValidationError::BeatWithinBar);
        }
        if self.source == TimingSourceKind::ProDjLink && self.deck_number.is_none() {
            return Err(TimingOutputValidationError::DeckRequired);
        }
        Ok(self)
    }

    #[must_use]
    pub const fn phase_beat(self) -> u8 {
        self.beat_within_bar - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingOutputState {
    Stopped,
    Starting,
    Ready,
    Running,
    Degraded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimingOutputStatus {
    pub state: TimingOutputState,
    pub provider: &'static str,
    pub helper_version: Option<String>,
    pub peers: u32,
    pub source: Option<TimingSourceKind>,
    pub deck_number: Option<u8>,
    pub bpm_milli: Option<u32>,
    pub beat_within_bar: Option<u8>,
    pub playing: bool,
    pub generation: Option<u64>,
    pub last_anchor_at: Option<Instant>,
    pub last_anchor_age_millis: Option<u64>,
    pub phase_error_micros: Option<i64>,
    pub received_anchor_count: u64,
    pub applied_anchor_count: u64,
    pub coalesced_anchor_count: u64,
    pub hard_reanchor_count: u64,
    pub soft_correction_count: u64,
    pub fail_closed_count: u64,
    pub failure_count: u64,
    pub max_abs_phase_error_micros: u64,
    pub last_reanchor: Option<TimingDiscontinuity>,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
}

impl Default for TimingOutputStatus {
    fn default() -> Self {
        Self {
            state: TimingOutputState::Stopped,
            provider: "Carabiner",
            helper_version: None,
            peers: 0,
            source: None,
            deck_number: None,
            bpm_milli: None,
            beat_within_bar: None,
            playing: false,
            generation: None,
            last_anchor_at: None,
            last_anchor_age_millis: None,
            phase_error_micros: None,
            received_anchor_count: 0,
            applied_anchor_count: 0,
            coalesced_anchor_count: 0,
            hard_reanchor_count: 0,
            soft_correction_count: 0,
            fail_closed_count: 0,
            failure_count: 0,
            max_abs_phase_error_micros: 0,
            last_reanchor: None,
            last_event: None,
            last_error: None,
        }
    }
}

pub trait TimingOutputProvider {
    type Error: Error + Send + Sync + 'static;

    fn provider_kind(&self) -> &'static str;
    fn publish(&mut self) -> Result<(), Self::Error>;
    fn synchronize(&mut self, observation: LinkClockObservation) -> Result<(), Self::Error>;
    fn hold(&mut self) -> Result<(), Self::Error>;
    fn fail_closed(&mut self, reason: String) -> Result<(), Self::Error> {
        let _ = reason;
        self.hold()
    }
    fn stop(&mut self) -> Result<(), Self::Error>;
    fn status(&self) -> TimingOutputStatus;
}

#[derive(Clone, Copy, Debug, thiserror::Error, Eq, PartialEq)]
pub enum TimingOutputValidationError {
    #[error("timing BPM is outside the supported range")]
    Tempo,
    #[error("timing beat-within-bar must be in the range 1 through 4")]
    BeatWithinBar,
    #[error("Pro DJ Link timing requires a deck number")]
    DeckRequired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pro_link_anchor_requires_a_deck() {
        let anchor = LinkClockObservation {
            source: TimingSourceKind::ProDjLink,
            deck_number: None,
            bpm_milli: 130_000,
            beat_within_bar: 1,
            playing: true,
            observed_at_micros: Some(10),
        };
        assert_eq!(
            anchor.validate(),
            Err(TimingOutputValidationError::DeckRequired)
        );
    }

    #[test]
    fn phase_is_zero_based_for_link_quantum() {
        let anchor = LinkClockObservation {
            source: TimingSourceKind::LocalPlayback,
            deck_number: None,
            bpm_milli: 130_000,
            beat_within_bar: 4,
            playing: true,
            observed_at_micros: None,
        };
        assert_eq!(anchor.phase_beat(), 3);
    }
}
