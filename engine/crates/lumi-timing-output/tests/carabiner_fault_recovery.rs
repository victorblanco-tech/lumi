use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use lumi_timing_output::{
    CarabinerConfiguration, CarabinerTimingOutput, LinkClockObservation, TimingOutputProvider as _,
    TimingOutputState, TimingSourceKind,
};

#[test]
fn stale_hold_recovers_on_the_next_authoritative_anchor_without_restarting() {
    let server = FakeCarabiner::start();
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: None,
        port: server.port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("fake helper should publish: {error}"));
    output
        .synchronize(anchor(140_000, true))
        .unwrap_or_else(|error| panic!("first anchor should synchronize: {error}"));
    wait_until(Duration::from_secs(1), || {
        output.status().state == TimingOutputState::Running
    });

    output
        .fail_closed("source timing stale")
        .unwrap_or_else(|error| panic!("stale source should hold: {error}"));
    wait_until(Duration::from_secs(1), || {
        output.status().state == TimingOutputState::Degraded
    });
    let held = output.status();
    assert!(!held.playing);
    assert_eq!(held.fail_closed_count, 1);
    assert_eq!(held.last_error.as_deref(), Some("source timing stale"));

    output
        .synchronize(anchor(142_500, true))
        .unwrap_or_else(|error| panic!("fresh anchor should recover: {error}"));
    wait_until(Duration::from_secs(1), || {
        let status = output.status();
        status.state == TimingOutputState::Running && status.bpm_milli == Some(142_500)
    });
    let recovered = output.status();
    assert!(recovered.playing);
    assert_eq!(recovered.last_error, None);
    assert_eq!(recovered.hard_reanchor_count, 2);
    assert_eq!(recovered.failure_count, 0);
    assert_eq!(
        server.connection_count(),
        1,
        "recovery must reuse the session"
    );
    assert!(
        server
            .commands()
            .iter()
            .any(|command| command == "stop-playing 1000000"),
        "fail-closed must stop shared Link transport"
    );
}

#[test]
fn a_fresh_anchor_queued_during_fail_closed_recovery_is_not_lost() {
    let server = FakeCarabiner::start();
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: None,
        port: server.port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("fake helper should publish: {error}"));
    output
        .synchronize(anchor(140_000, true))
        .unwrap_or_else(|error| panic!("first anchor should synchronize: {error}"));
    wait_until(Duration::from_secs(1), || {
        output.status().state == TimingOutputState::Running
    });

    output
        .fail_closed("source timing stale")
        .unwrap_or_else(|error| panic!("stale source should hold: {error}"));
    output
        .synchronize(anchor(141_000, true))
        .unwrap_or_else(|error| panic!("racing recovery anchor should queue: {error}"));

    wait_until(Duration::from_secs(1), || {
        let status = output.status();
        status.state == TimingOutputState::Running
            && status.bpm_milli == Some(141_000)
            && status.fail_closed_count == 1
    });
    assert_eq!(server.connection_count(), 1);
}

#[test]
fn continuous_udp_beat_jitter_never_rewinds_the_link_timeline() {
    let server = FakeCarabiner::start();
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: None,
        port: server.port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("fake helper should publish: {error}"));
    output
        .synchronize(anchor(155_000, true))
        .unwrap_or_else(|error| panic!("initial anchor should synchronize: {error}"));
    wait_until(Duration::from_secs(1), || {
        output.status().applied_anchor_count == 1
    });

    for beat_within_bar in [2, 4, 1, 3, 1, 4] {
        let mut continuous = anchor(155_000, true);
        continuous.beat_within_bar = beat_within_bar;
        let expected = output.status().applied_anchor_count.saturating_add(1);
        output
            .synchronize(continuous)
            .unwrap_or_else(|error| panic!("continuous anchor should synchronize: {error}"));
        wait_until(Duration::from_secs(1), || {
            output.status().applied_anchor_count >= expected
        });
    }

    let status = output.status();
    assert_eq!(status.hard_reanchor_count, 1);
    assert_eq!(status.soft_correction_count, 0);
    let phase_commands = server
        .commands()
        .into_iter()
        .filter(|command| {
            command.starts_with("force-beat-at-time ")
                || command.starts_with("request-beat-at-time ")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        phase_commands.len(),
        1,
        "only the initial discontinuity may move Link phase"
    );
}

#[test]
fn a_short_scheduler_stall_does_not_restart_the_link_peer() {
    let server = FakeCarabiner::start_with_response_delay(Duration::from_millis(350));
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: None,
        port: server.port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("a delayed helper should still publish: {error}"));

    let status = output.status();
    assert_eq!(status.state, TimingOutputState::Ready);
    assert_eq!(status.failure_count, 0);
    assert_eq!(
        server.connection_count(),
        1,
        "a transient userspace delay may not create a second Link peer"
    );
}

#[test]
fn a_failed_active_session_never_opens_a_replacement_peer_implicitly() {
    let server = FakeCarabiner::start_failing_on_bpm();
    let mut output = CarabinerTimingOutput::new(CarabinerConfiguration {
        executable: None,
        port: server.port,
        expected_version: "1.2.0".to_owned(),
    });
    output
        .publish()
        .unwrap_or_else(|error| panic!("fake helper should publish: {error}"));
    output
        .synchronize(anchor(140_000, true))
        .unwrap_or_else(|error| panic!("anchor should enter the worker: {error}"));
    wait_until(Duration::from_secs(2), || {
        output.status().state == TimingOutputState::Degraded
    });

    for bpm in [141_000, 142_000, 143_000] {
        output
            .synchronize(anchor(bpm, true))
            .unwrap_or_else(|error| panic!("latest clock may still be accepted: {error}"));
    }
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        server.connection_count(),
        1,
        "a failed session requires an explicit Link disable/enable; clock updates may not create another peer"
    );
}

fn anchor(bpm_milli: u32, playing: bool) -> LinkClockObservation {
    LinkClockObservation {
        source: TimingSourceKind::ProDjLink,
        deck_number: Some(1),
        bpm_milli,
        beat_within_bar: 1,
        playing,
        observed_at_micros: None,
    }
}

fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for timing worker"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

struct FakeCarabiner {
    port: u16,
    commands: Arc<Mutex<Vec<String>>>,
    connections: Arc<Mutex<u64>>,
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}

impl FakeCarabiner {
    fn start() -> Self {
        Self::start_with_options(Duration::ZERO, false)
    }

    fn start_with_response_delay(response_delay: Duration) -> Self {
        Self::start_with_options(response_delay, false)
    }

    fn start_failing_on_bpm() -> Self {
        Self::start_with_options(Duration::ZERO, true)
    }

    fn start_with_options(response_delay: Duration, fail_on_bpm: bool) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap_or_else(|error| panic!("fake helper should bind: {error}"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|error| panic!("fake helper should be nonblocking: {error}"));
        let port = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("fake helper address should exist: {error}"))
            .port();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(Mutex::new(0_u64));
        let worker_commands = Arc::clone(&commands);
        let worker_connections = Arc::clone(&connections);
        let (stop, stop_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            loop {
                if stop_receiver.try_recv().is_ok() {
                    return;
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Ok(mut count) = worker_connections.lock() {
                            *count = (*count).saturating_add(1);
                        }
                        serve_connection(stream, &worker_commands, response_delay, fail_on_bpm);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("fake helper accept failed: {error}"),
                }
            }
        });
        Self {
            port,
            commands,
            connections,
            stop,
            worker: Some(worker),
        }
    }

    fn commands(&self) -> Vec<String> {
        self.commands
            .lock()
            .map_or_else(|_| Vec::new(), |value| value.clone())
    }

    fn connection_count(&self) -> u64 {
        self.connections.lock().map_or(0, |value| *value)
    }
}

impl Drop for FakeCarabiner {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        // Wake a worker that is waiting in accept after the test connection
        // has already closed.
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve_connection(
    stream: TcpStream,
    commands: &Arc<Mutex<Vec<String>>>,
    response_delay: Duration,
    fail_on_bpm: bool,
) {
    stream
        .set_nonblocking(false)
        .unwrap_or_else(|error| panic!("fake helper connection should block: {error}"));
    let mut writer = stream
        .try_clone()
        .unwrap_or_else(|error| panic!("fake helper stream should clone: {error}"));
    write_status(&mut writer, 120.0, false);
    let mut bpm = 120.0;
    let mut playing = false;
    for line in BufReader::new(stream).lines() {
        let Ok(command) = line else { return };
        if let Ok(mut recorded) = commands.lock() {
            recorded.push(command.clone());
        }
        thread::sleep(response_delay);
        if command == "version" {
            writeln!(writer, "version \"1.2.0\"")
                .unwrap_or_else(|error| panic!("fake version should write: {error}"));
            writer
                .flush()
                .unwrap_or_else(|error| panic!("fake version should flush: {error}"));
            continue;
        }
        if let Some(value) = command.strip_prefix("bpm ") {
            if fail_on_bpm {
                return;
            }
            bpm = value
                .parse()
                .unwrap_or_else(|_| panic!("invalid BPM command: {command}"));
        } else if command.starts_with("start-playing ") {
            playing = true;
        } else if command.starts_with("stop-playing ") {
            playing = false;
        }
        write_status(&mut writer, bpm, playing);
    }
}

fn write_status(stream: &mut TcpStream, bpm: f64, playing: bool) {
    writeln!(
        stream,
        "status {{ :peers 1 :bpm {bpm:.6} :start 1000000 :beat 0.000000 :playing {playing} }}"
    )
    .unwrap_or_else(|error| panic!("fake status should write: {error}"));
    stream
        .flush()
        .unwrap_or_else(|error| panic!("fake status should flush: {error}"));
}
