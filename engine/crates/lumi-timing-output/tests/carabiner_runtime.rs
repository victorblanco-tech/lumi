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
    let version = output
        .self_test_helper()
        .unwrap_or_else(|error| panic!("helper self-test should pass: {error}"));
    assert_eq!(version, "1.2.0");
    let idle_status = output.status();
    assert_eq!(idle_status.state, TimingOutputState::Stopped);
    assert_eq!(idle_status.peers, 0);
    output
        .publish()
        .unwrap_or_else(|error| panic!("helper should publish: {error}"));
    output
        .synchronize(TimingAnchor {
            source: TimingSourceKind::LocalPlayback,
            deck_number: Some(1),
            bpm_milli: 130_000,
            beat_within_bar: 1,
            playing: false,
            generation: 1,
            discontinuity: TimingDiscontinuity::Started,
            observed_at_micros: None,
        })
        .unwrap_or_else(|error| panic!("timeline should synchronize: {error}"));
    thread::sleep(Duration::from_millis(100));

    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Ready);
    assert_eq!(status.helper_version.as_deref(), Some("1.2.0"));
    assert_eq!(status.bpm_milli, Some(130_000));
    assert!(!status.playing);

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
        .unwrap_or_else(|error| panic!("transport should start: {error}"));
    thread::sleep(Duration::from_millis(100));
    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Running);
    assert!(status.playing);

    output
        .synchronize(TimingAnchor {
            source: TimingSourceKind::LocalPlayback,
            deck_number: Some(1),
            bpm_milli: 140_000,
            beat_within_bar: 2,
            playing: true,
            generation: 1,
            discontinuity: TimingDiscontinuity::Continuous,
            observed_at_micros: None,
        })
        .unwrap_or_else(|error| panic!("tempo change should synchronize: {error}"));
    thread::sleep(Duration::from_millis(250));

    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Running);
    assert_eq!(status.bpm_milli, Some(140_000));
    assert!(status.playing);
    if std::env::var("LUMI_EXPECT_LINK_PEER").as_deref() == Ok("1") {
        assert!(
            status.peers >= 1,
            "SoundSwitch should be visible as a Link peer"
        );
    }

    output
        .hold()
        .unwrap_or_else(|error| panic!("timeline should hold: {error}"));
    thread::sleep(Duration::from_millis(50));
    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Ready);
    assert!(!status.playing);
    output
        .stop()
        .unwrap_or_else(|error| panic!("helper should stop: {error}"));
    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Stopped);
    assert_eq!(status.helper_version.as_deref(), Some("1.2.0"));
    assert_eq!(status.peers, 0);
}
