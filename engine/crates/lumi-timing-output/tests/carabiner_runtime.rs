use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use lumi_timing_output::{
    CarabinerConfiguration, CarabinerTimingOutput, TimingAnchor, TimingDiscontinuity,
    TimingOutputProvider as _, TimingOutputState, TimingSourceKind,
};

#[test]
#[ignore = "requires LUMI_CARABINER_TEST_EXECUTABLE"]
fn launches_real_helper_and_publishes_a_link_timeline() {
    let executable = std::env::var("LUMI_CARABINER_TEST_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| panic!("LUMI_CARABINER_TEST_EXECUTABLE is required"));
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: Some(executable),
        port: 17_091,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("helper should publish: {error}"));
    output
        .synchronize(TimingAnchor {
            source: TimingSourceKind::LocalPlayback,
            deck_number: Some(1),
            bpm_milli: 130_000,
            beat_within_bar: 1,
            playing: true,
            generation: 1,
            discontinuity: TimingDiscontinuity::Started,
            observed_at_micros: None,
        })
        .unwrap_or_else(|error| panic!("timeline should synchronize: {error}"));
    thread::sleep(Duration::from_millis(100));

    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Running);
    assert_eq!(status.helper_version.as_deref(), Some("1.2.0"));
    assert_eq!(status.bpm_milli, Some(130_000));
    assert!(status.playing);

    output
        .hold()
        .unwrap_or_else(|error| panic!("timeline should hold: {error}"));
    output
        .stop()
        .unwrap_or_else(|error| panic!("helper should stop: {error}"));
}
