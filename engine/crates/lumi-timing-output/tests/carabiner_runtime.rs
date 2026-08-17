use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use lumi_timing_output::{
    CarabinerConfiguration, CarabinerTimingOutput, LinkClockObservation, TimingOutputProvider as _,
    TimingOutputState, TimingSourceKind,
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
        .synchronize(LinkClockObservation {
            source: TimingSourceKind::LocalPlayback,
            deck_number: Some(1),
            bpm_milli: 130_000,
            beat_within_bar: 1,
            playing: false,
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
        .synchronize(LinkClockObservation {
            source: TimingSourceKind::LocalPlayback,
            deck_number: Some(1),
            bpm_milli: 130_000,
            beat_within_bar: 1,
            playing: true,
            observed_at_micros: None,
        })
        .unwrap_or_else(|error| panic!("transport should start: {error}"));
    thread::sleep(Duration::from_millis(100));
    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Running);
    assert!(status.playing);

    output
        .synchronize(LinkClockObservation {
            source: TimingSourceKind::LocalPlayback,
            deck_number: Some(1),
            bpm_milli: 140_000,
            beat_within_bar: 2,
            playing: true,
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

#[test]
#[ignore = "requires LUMI_CARABINER_TEST_EXECUTABLE"]
fn dropping_the_managed_output_terminates_its_owned_link_peer() {
    let executable = std::env::var("LUMI_CARABINER_TEST_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| panic!("LUMI_CARABINER_TEST_EXECUTABLE is required"));
    let port = 17_092;
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: Some(executable),
        port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("helper should publish: {error}"));
    assert!(control_port_is_open(port));

    // App shutdown drops the engine runtime; it does not need a separate user
    // command to stop Link first. The owned foreground helper must disappear
    // as part of that drop so SoundSwitch cannot retain a ghost peer.
    drop(output);
    let deadline = Instant::now() + Duration::from_secs(2);
    while control_port_is_open(port) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !control_port_is_open(port),
        "the app-owned Link helper must close its control port on drop"
    );
}

#[test]
#[ignore = "set LUMI_LINK_SOAK_SECONDS and LUMI_CARABINER_TEST_EXECUTABLE"]
fn link_only_configurable_soak_keeps_one_bounded_latest_clock() {
    let executable = std::env::var("LUMI_CARABINER_TEST_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| panic!("LUMI_CARABINER_TEST_EXECUTABLE is required"));
    let duration_seconds: u64 = std::env::var("LUMI_LINK_SOAK_SECONDS")
        .unwrap_or_else(|_| panic!("LUMI_LINK_SOAK_SECONDS is required"))
        .parse()
        .unwrap_or_else(|error| panic!("LUMI_LINK_SOAK_SECONDS must be an integer: {error}"));
    assert!(duration_seconds > 0, "Link soak duration must be positive");
    let port = std::env::var("LUMI_LINK_SOAK_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(17_093);
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: Some(executable),
        port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("Link helper should publish: {error}"));

    let started = Instant::now();
    let finish = started + Duration::from_secs(duration_seconds);
    let mut observations = 0_u64;
    let mut last_bpm_milli = 155_000_u32;
    while Instant::now() < finish {
        let step = observations % 12;
        let bpm_milli = match step {
            0..=3 => 155_000,
            4..=7 => 161_510,
            _ => 151_900,
        };
        last_bpm_milli = bpm_milli;
        output
            .synchronize(LinkClockObservation {
                source: TimingSourceKind::ProDjLink,
                deck_number: Some(1),
                bpm_milli,
                beat_within_bar: u8::try_from(observations % 4 + 1)
                    .unwrap_or_else(|_| unreachable!()),
                playing: true,
                observed_at_micros: None,
            })
            .unwrap_or_else(|error| panic!("Link clock should remain writable: {error}"));
        observations = observations.saturating_add(1);
        let status = output.status();
        assert_eq!(status.failure_count, 0, "Link helper degraded: {status:?}");
        assert_ne!(status.state, TimingOutputState::Degraded);
        thread::sleep(Duration::from_millis(25));
    }

    let drain_deadline = Instant::now() + Duration::from_secs(2);
    while output.status().bpm_milli != Some(last_bpm_milli) && Instant::now() < drain_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let status = output.status();
    println!(
        "Link soak: duration={}s received={} applied={} coalesced={} hard={} failures={} peers={}",
        duration_seconds,
        status.received_anchor_count,
        status.applied_anchor_count,
        status.coalesced_anchor_count,
        status.hard_reanchor_count,
        status.failure_count,
        status.peers,
    );
    assert_eq!(status.received_anchor_count, observations);
    assert_eq!(status.bpm_milli, Some(last_bpm_milli));
    assert!(
        status
            .applied_anchor_count
            .saturating_add(status.coalesced_anchor_count)
            >= observations,
        "each source observation must be either applied or explicitly coalesced"
    );
    assert_eq!(status.failure_count, 0);
    if std::env::var("LUMI_EXPECT_LINK_PEER").as_deref() == Ok("1") {
        assert_eq!(status.peers, 1, "exactly one SoundSwitch peer is expected");
    }
    output
        .stop()
        .unwrap_or_else(|error| panic!("Link helper should stop cleanly: {error}"));
    assert_eq!(output.status().peers, 0);
}

fn control_port_is_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
        Duration::from_millis(50),
    )
    .is_ok()
}
