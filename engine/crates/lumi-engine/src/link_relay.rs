//! Isolated Ableton Link relay.
//!
//! The relay consumes only the selected musical clock. It deliberately has no
//! operation-state, phrase, lighting-plan, AutoLoop or MIDI API: those domains
//! cannot hold, restart or re-anchor SoundSwitch's Link timeline.

use std::time::{Duration, Instant};

use lumi_timing_output::{
    CarabinerTimingOutput, LinkClockObservation, TimingOutputProvider, TimingOutputStatus,
    TimingSourceKind,
};

pub(crate) const MINIMUM_PROLINK_TIMING_STALE_AFTER: Duration = Duration::from_secs(3);
pub(crate) const MAXIMUM_PROLINK_TIMING_STALE_AFTER: Duration = Duration::from_secs(8);
const PROLINK_TIMING_STALE_BEATS: u64 = 8;

pub(crate) struct LinkRelay<P = CarabinerTimingOutput>
where
    P: TimingOutputProvider,
{
    provider: P,
    enabled: bool,
    last_prolink_timing_at: Option<Instant>,
    last_prolink_bpm_milli: Option<u32>,
    last_prolink_playing: Option<bool>,
    last_prolink_deck_number: Option<u8>,
    last_prolink_observed_at_micros: Option<u64>,
    prolink_timing_stale: bool,
}

impl<P> LinkRelay<P>
where
    P: TimingOutputProvider,
{
    pub(crate) fn new(provider: P) -> Self {
        Self {
            provider,
            enabled: false,
            last_prolink_timing_at: None,
            last_prolink_bpm_milli: None,
            last_prolink_playing: None,
            last_prolink_deck_number: None,
            last_prolink_observed_at_micros: None,
            prolink_timing_stale: false,
        }
    }

    pub(crate) const fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn status(&self) -> TimingOutputStatus {
        self.provider.status()
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        if enabled == self.enabled {
            return Ok(());
        }
        if enabled {
            self.provider.publish().map_err(|error| error.to_string())?;
            self.enabled = true;
        } else {
            let stop_result = self.provider.stop().map_err(|error| error.to_string());
            // Stop is a user-owned lifecycle decision. Even when the bounded
            // helper cleanup reports an error, a later Enable must perform a
            // fresh explicit publish instead of no-oping against stale state.
            self.enabled = false;
            self.reset_prolink_freshness();
            stop_result?;
        }
        Ok(())
    }

    /// Accepts one immutable observation from the selected deck clock.
    ///
    /// Continuous tempo changes are intentionally forwarded immediately. The
    /// provider preserves its established Link phase while changing BPM, so a
    /// master-CDJ pitch-slider movement reaches SoundSwitch without becoming a
    /// phrase, lighting or transport-correction command.
    pub(crate) fn synchronize(&mut self, observation: LinkClockObservation) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if observation.source == TimingSourceKind::ProDjLink {
            if let (Some(previous), Some(current)) = (
                self.last_prolink_observed_at_micros,
                observation.observed_at_micros,
            ) && current < previous
            {
                return Ok(());
            }
            let materially_changed = self.last_prolink_bpm_milli != Some(observation.bpm_milli)
                || self.last_prolink_playing != Some(observation.playing)
                || self.last_prolink_deck_number != observation.deck_number;
            self.last_prolink_timing_at = Some(Instant::now());
            self.last_prolink_bpm_milli = Some(observation.bpm_milli);
            self.last_prolink_playing = Some(observation.playing);
            self.last_prolink_deck_number = observation.deck_number;
            self.last_prolink_observed_at_micros = observation.observed_at_micros;
            self.prolink_timing_stale = false;
            if !materially_changed {
                return Ok(());
            }
        } else {
            self.reset_prolink_freshness();
        }
        self.provider
            .synchronize(observation)
            .map_err(|error| error.to_string())
    }

    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn reconcile_prolink_freshness(
        &mut self,
        source_ready: bool,
        source_status: &str,
    ) -> Result<(), String> {
        if !self.enabled || self.prolink_timing_stale {
            return Ok(());
        }
        let Some(last_timing_at) = self.last_prolink_timing_at else {
            return Ok(());
        };
        // A stopped master intentionally emits no beat packets. Its held clock
        // remains valid while the source itself is healthy.
        if source_ready && self.last_prolink_playing == Some(false) {
            return Ok(());
        }
        let age = last_timing_at.elapsed();
        if source_ready && age <= prolink_timing_stale_after(self.last_prolink_bpm_milli) {
            return Ok(());
        }
        let reason = if source_ready {
            format!(
                "Pro DJ Link timing is stale ({} ms without a clock observation); Link transport was held fail-closed",
                age.as_millis()
            )
        } else {
            format!("Pro DJ Link source is {source_status}; Link transport was held fail-closed")
        };
        self.fail_closed(reason)
    }

    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn fail_closed(&mut self, reason: impl Into<String>) -> Result<(), String> {
        if !self.enabled || self.prolink_timing_stale {
            return Ok(());
        }
        self.provider
            .fail_closed(reason.into())
            .map_err(|error| error.to_string())?;
        self.prolink_timing_stale = true;
        Ok(())
    }

    fn reset_prolink_freshness(&mut self) {
        self.last_prolink_timing_at = None;
        self.last_prolink_bpm_milli = None;
        self.last_prolink_playing = None;
        self.last_prolink_deck_number = None;
        self.last_prolink_observed_at_micros = None;
        self.prolink_timing_stale = false;
    }
}

impl LinkRelay<CarabinerTimingOutput> {
    pub(crate) fn test_helper(&mut self) -> Result<(), String> {
        self.provider
            .self_test_helper()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn prolink_timing_stale_after(bpm_milli: Option<u32>) -> Duration {
    let beat_window = bpm_milli
        .filter(|bpm| *bpm > 0)
        .map(|bpm| {
            Duration::from_micros(
                60_000_000_000_u64.saturating_mul(PROLINK_TIMING_STALE_BEATS) / u64::from(bpm),
            )
        })
        .unwrap_or(MINIMUM_PROLINK_TIMING_STALE_AFTER);
    beat_window.clamp(
        MINIMUM_PROLINK_TIMING_STALE_AFTER,
        MAXIMUM_PROLINK_TIMING_STALE_AFTER,
    )
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use lumi_timing_output::{TimingOutputState, TimingSourceKind};

    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Call {
        Publish,
        Synchronize(LinkClockObservation),
        Hold,
        Stop,
    }

    #[derive(Default)]
    struct RecordingProvider {
        calls: Vec<Call>,
        status: TimingOutputStatus,
    }

    impl TimingOutputProvider for RecordingProvider {
        type Error = Infallible;

        fn provider_kind(&self) -> &'static str {
            "recording"
        }

        fn publish(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Publish);
            self.status.state = TimingOutputState::Ready;
            Ok(())
        }

        fn synchronize(&mut self, observation: LinkClockObservation) -> Result<(), Self::Error> {
            self.calls.push(Call::Synchronize(observation));
            self.status.bpm_milli = Some(observation.bpm_milli);
            Ok(())
        }

        fn hold(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Hold);
            Ok(())
        }

        fn stop(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Stop);
            Ok(())
        }

        fn status(&self) -> TimingOutputStatus {
            self.status.clone()
        }
    }

    fn prolink_clock(bpm_milli: u32) -> LinkClockObservation {
        LinkClockObservation {
            source: TimingSourceKind::ProDjLink,
            deck_number: Some(1),
            bpm_milli,
            beat_within_bar: 1,
            playing: true,
            observed_at_micros: Some(1_000_000),
        }
    }

    #[test]
    fn realtime_master_tempo_changes_are_forwarded_without_hold_or_restart() {
        let mut relay = LinkRelay::new(RecordingProvider::default());
        assert!(relay.set_enabled(true).is_ok());
        assert!(relay.synchronize(prolink_clock(130_000)).is_ok());
        assert!(relay.synchronize(prolink_clock(136_500)).is_ok());

        assert_eq!(relay.status().bpm_milli, Some(136_500));
        assert_eq!(
            relay.provider.calls,
            vec![
                Call::Publish,
                Call::Synchronize(prolink_clock(130_000)),
                Call::Synchronize(prolink_clock(136_500)),
            ]
        );
    }

    #[test]
    fn repeated_master_beats_refresh_health_without_recorrecting_link() {
        let mut relay = LinkRelay::new(RecordingProvider::default());
        assert!(relay.set_enabled(true).is_ok());
        assert!(relay.synchronize(prolink_clock(140_000)).is_ok());
        let mut next_beat = prolink_clock(140_000);
        next_beat.beat_within_bar = 2;
        next_beat.observed_at_micros = Some(1_500_000);
        assert!(relay.synchronize(next_beat).is_ok());

        assert_eq!(
            relay.provider.calls,
            vec![Call::Publish, Call::Synchronize(prolink_clock(140_000))]
        );
        assert!(!relay.prolink_timing_stale);
    }

    #[test]
    fn disabled_relay_ignores_clock_observations() {
        let mut relay = LinkRelay::new(RecordingProvider::default());
        assert!(relay.synchronize(prolink_clock(140_000)).is_ok());
        assert!(relay.provider.calls.is_empty());
    }
}
