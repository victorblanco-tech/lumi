use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};

use lumi_midi_output::{MIDI_SOURCE_NAME, MidiMessage, MidiSourceProvider, RealtimeMidiController};

#[derive(Default)]
struct CountingProvider {
    published: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("counting provider failed")]
struct CountingError;

impl MidiSourceProvider for CountingProvider {
    type Error = CountingError;

    fn publish(&mut self, source_name: &str) -> Result<(), Self::Error> {
        self.published = source_name == MIDI_SOURCE_NAME;
        Ok(())
    }

    fn stop(&mut self) {
        self.published = false;
    }

    fn send(&mut self, _messages: &[MidiMessage]) -> Result<(), Self::Error> {
        if self.published {
            Ok(())
        } else {
            Err(CountingError)
        }
    }
}

#[test]
#[ignore = "set LUMI_AUTOLOOP_SOAK_SECONDS; RC evidence requires at least 3600"]
fn realtime_lane_configurable_soak_retains_correctness_and_latency() -> Result<(), Box<dyn Error>> {
    let duration_seconds: u64 = std::env::var("LUMI_AUTOLOOP_SOAK_SECONDS")
        .map_err(|_| "LUMI_AUTOLOOP_SOAK_SECONDS is required")?
        .parse()?;
    if duration_seconds == 0 {
        return Err("soak duration must be positive".into());
    }

    let lane = RealtimeMidiController::new(CountingProvider::default);
    lane.publish()?;
    let started = Instant::now();
    let finish = started + Duration::from_secs(duration_seconds);
    let mut generation = 0_u64;
    let mut expected_emitted = 0_u64;
    let mut expected_cancelled = 0_u64;

    while Instant::now() < finish {
        generation = generation.saturating_add(1);
        lane.set_generation(generation)?;
        let now = Instant::now();
        lane.schedule_bank(generation, ((generation - 1) % 4 + 1) as u8, now)?;
        lane.schedule_autoloop(
            generation,
            ((generation - 1) % 32 + 1) as u8,
            now + Duration::from_millis(52),
        )?;
        expected_emitted = expected_emitted.saturating_add(2);

        let iteration_deadline = Instant::now() + Duration::from_millis(300);
        while lane.status().emitted_count < expected_emitted && Instant::now() < iteration_deadline
        {
            thread::sleep(Duration::from_millis(1));
        }
        if lane.status().emitted_count != expected_emitted {
            return Err(format!("missed or duplicate output: {:?}", lane.status()).into());
        }
        if generation.is_multiple_of(10) {
            lane.schedule_autoloop(generation, 32, Instant::now() + Duration::from_millis(400))?;
            generation = generation.saturating_add(1);
            lane.set_generation(generation)?;
            expected_cancelled = expected_cancelled.saturating_add(1);
        }
    }

    // Generation changes are deliberately fire-and-forget on the caller lane;
    // the single realtime worker still processes them in FIFO order. Give the
    // final invalidation command the same bounded drain opportunity as emitted
    // actions before evaluating the counters.
    let cancellation_deadline = Instant::now() + Duration::from_millis(300);
    while lane.status().cancelled_count < expected_cancelled
        && Instant::now() < cancellation_deadline
    {
        thread::sleep(Duration::from_millis(1));
    }
    let status = lane.status();
    println!(
        "AutoLoop soak: duration={}s scheduled={} emitted={} cancelled={} saturation={} p50={}us p95={}us p99={}us max={}us highWater={}",
        duration_seconds,
        status.scheduled_count,
        status.emitted_count,
        status.cancelled_count,
        status.saturation_count,
        status.latency_p50_micros,
        status.latency_p95_micros,
        status.latency_p99_micros,
        status.latency_max_micros,
        status.queue_high_water,
    );
    assert_eq!(status.emitted_count, expected_emitted);
    assert!(status.cancelled_count >= expected_cancelled);
    assert_eq!(status.saturation_count, 0);
    assert!(status.queue_high_water <= status.queue_capacity);
    assert!(status.latency_p95_micros <= 20_000, "{status:?}");
    Ok(())
}
