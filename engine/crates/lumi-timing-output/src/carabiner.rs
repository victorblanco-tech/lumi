use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    TimingAnchor, TimingDiscontinuity, TimingOutputProvider, TimingOutputState, TimingOutputStatus,
};

pub const CARABINER_DEFAULT_PORT: u16 = 17_001;
pub const CARABINER_EXPECTED_VERSION: &str = "1.2.0";
const COMMAND_TIMEOUT: Duration = Duration::from_millis(300);
const START_RETRIES: usize = 30;
const START_RETRY_DELAY: Duration = Duration::from_millis(50);
const SOFT_PHASE_ERROR_MICROS: i64 = 8_000;
const HARD_PHASE_ERROR_MICROS: i64 = 25_000;
const MAX_SHARED_EPOCH_SKEW_MICROS: u64 = 5_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarabinerConfiguration {
    pub executable: Option<PathBuf>,
    pub port: u16,
    pub expected_version: String,
}

impl Default for CarabinerConfiguration {
    fn default() -> Self {
        Self {
            executable: None,
            port: CARABINER_DEFAULT_PORT,
            expected_version: CARABINER_EXPECTED_VERSION.to_owned(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CarabinerError {
    #[error("the Ableton Link timing worker is unavailable")]
    WorkerUnavailable,
    #[error("invalid timing anchor: {0}")]
    InvalidAnchor(String),
    #[error("Carabiner failed: {0}")]
    Helper(String),
}

enum WorkerCommand {
    Publish(Option<mpsc::Sender<Result<(), String>>>),
    Synchronize,
    Hold,
    FailClosed(String),
    Stop(mpsc::Sender<Result<(), String>>),
    Shutdown,
}

pub struct CarabinerTimingOutput {
    configuration: CarabinerConfiguration,
    commands: mpsc::Sender<WorkerCommand>,
    latest_anchor: Arc<Mutex<Option<TimingAnchor>>>,
    worker: Option<JoinHandle<()>>,
    status: Arc<Mutex<TimingOutputStatus>>,
    shutting_down: Arc<AtomicBool>,
}

impl CarabinerTimingOutput {
    #[must_use]
    pub fn new(configuration: CarabinerConfiguration) -> Self {
        let (commands, receiver) = mpsc::channel();
        let status = Arc::new(Mutex::new(TimingOutputStatus::default()));
        let latest_anchor = Arc::new(Mutex::new(None));
        let worker_status = Arc::clone(&status);
        let worker_anchor = Arc::clone(&latest_anchor);
        let worker_configuration = configuration.clone();
        let shutting_down = Arc::new(AtomicBool::new(false));
        let worker_shutting_down = Arc::clone(&shutting_down);
        let worker = thread::Builder::new()
            .name("lumi-ableton-link".to_owned())
            .spawn(move || {
                run_worker(
                    worker_configuration,
                    receiver,
                    &worker_status,
                    &worker_anchor,
                    &worker_shutting_down,
                );
            })
            .ok();
        Self {
            configuration,
            commands,
            latest_anchor,
            worker,
            status,
            shutting_down,
        }
    }

    /// Starts the managed helper without delaying engine or UI startup.
    pub fn publish_async(&self) -> Result<(), CarabinerError> {
        update_status(&self.status, |status| {
            status.state = TimingOutputState::Starting;
            status.last_event = Some("Starting managed Ableton Link output".to_owned());
            status.last_error = None;
        });
        self.commands
            .send(WorkerCommand::Publish(None))
            .map_err(|_| CarabinerError::WorkerUnavailable)
    }

    /// Verifies the bundled helper without creating an Ableton Link peer.
    ///
    /// Launching the normal Carabiner server joins the shared Link session and
    /// can therefore influence consensus tempo even while Lumi is Off. The
    /// diagnostics self-test deliberately uses the process' terminating
    /// `--version` mode instead.
    pub fn self_test_helper(&self) -> Result<String, CarabinerError> {
        let executable = self.configuration.executable.as_ref().ok_or_else(|| {
            CarabinerError::Helper("managed executable is unavailable".to_owned())
        })?;
        let output = Command::new(executable)
            .arg("--version")
            .env_remove("LUMI_SESSION_TOKEN")
            .stdin(Stdio::null())
            .output()
            .map_err(|error| CarabinerError::Helper(error.to_string()))?;
        if !output.status.success() {
            return Err(CarabinerError::Helper(format!(
                "version self-test exited with {}",
                output.status
            )));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| CarabinerError::Helper("version output was not UTF-8".to_owned()))?;
        let expected = format!("Carabiner version {}", self.configuration.expected_version);
        if !stdout.lines().any(|line| line.trim() == expected) {
            return Err(CarabinerError::Helper(format!(
                "expected {}, found {}",
                self.configuration.expected_version,
                stdout.lines().next().unwrap_or("no version")
            )));
        }
        update_status(&self.status, |status| {
            status.state = TimingOutputState::Stopped;
            status.helper_version = Some(self.configuration.expected_version.clone());
            status.peers = 0;
            status.last_event = Some("Ableton Link helper self-test passed; idle".to_owned());
            status.last_error = None;
        });
        Ok(self.configuration.expected_version.clone())
    }

    /// Stops Link transport immediately while retaining the current session.
    ///
    /// A fresh authoritative anchor can recover the same worker without an app
    /// or helper restart. The reason remains visible until that recovery has
    /// been applied successfully.
    pub fn fail_closed(&self, reason: impl Into<String>) -> Result<(), CarabinerError> {
        self.commands
            .send(WorkerCommand::FailClosed(reason.into()))
            .map_err(|_| CarabinerError::WorkerUnavailable)
    }
}

impl TimingOutputProvider for CarabinerTimingOutput {
    type Error = CarabinerError;

    fn provider_kind(&self) -> &'static str {
        "abletonLink"
    }

    fn publish(&mut self) -> Result<(), Self::Error> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::Publish(Some(reply)))
            .map_err(|_| CarabinerError::WorkerUnavailable)?;
        response
            .recv()
            .map_err(|_| CarabinerError::WorkerUnavailable)?
            .map_err(CarabinerError::Helper)
    }

    fn synchronize(&mut self, anchor: TimingAnchor) -> Result<(), Self::Error> {
        let anchor = anchor
            .validate()
            .map_err(|error| CarabinerError::InvalidAnchor(error.to_string()))?;
        let previous_pending = self
            .latest_anchor
            .lock()
            .map_err(|_| CarabinerError::WorkerUnavailable)?
            .replace(anchor);
        update_status(&self.status, |status| {
            status.received_anchor_count = status.received_anchor_count.saturating_add(1);
            if previous_pending.is_some() {
                status.coalesced_anchor_count = status.coalesced_anchor_count.saturating_add(1);
            }
        });
        let should_wake = previous_pending.is_none();
        if should_wake {
            self.commands
                .send(WorkerCommand::Synchronize)
                .map_err(|_| CarabinerError::WorkerUnavailable)?;
        }
        Ok(())
    }

    fn hold(&mut self) -> Result<(), Self::Error> {
        self.commands
            .send(WorkerCommand::Hold)
            .map_err(|_| CarabinerError::WorkerUnavailable)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::Stop(reply))
            .map_err(|_| CarabinerError::WorkerUnavailable)?;
        response
            .recv()
            .map_err(|_| CarabinerError::WorkerUnavailable)?
            .map_err(CarabinerError::Helper)
    }

    fn status(&self) -> TimingOutputStatus {
        self.status.lock().map_or_else(
            |_| TimingOutputStatus::default(),
            |status| {
                let mut snapshot = status.clone();
                snapshot.last_anchor_age_millis = snapshot.last_anchor_at.map(|observed| {
                    u64::try_from(observed.elapsed().as_millis()).unwrap_or(u64::MAX)
                });
                snapshot
            },
        )
    }
}

impl Drop for CarabinerTimingOutput {
    fn drop(&mut self) {
        self.shutting_down.store(true, Ordering::Release);
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(
    configuration: CarabinerConfiguration,
    receiver: mpsc::Receiver<WorkerCommand>,
    shared_status: &Arc<Mutex<TimingOutputStatus>>,
    latest_anchor: &Arc<Mutex<Option<TimingAnchor>>>,
    shutting_down: &Arc<AtomicBool>,
) {
    let mut session: Option<CarabinerSession> = None;
    let mut owned_child: Option<Child> = None;
    let mut last_anchor: Option<TimingAnchor> = None;

    while let Ok(command) = receiver.recv() {
        if shutting_down.load(Ordering::Acquire) && !matches!(&command, WorkerCommand::Shutdown) {
            continue;
        }
        match command {
            WorkerCommand::Publish(reply) => {
                update_status(shared_status, |status| {
                    status.state = TimingOutputState::Starting;
                    status.last_event = Some("Starting managed Ableton Link output".to_owned());
                    status.last_error = None;
                });
                let result = open_session(
                    &configuration,
                    &mut owned_child,
                    &mut session,
                    shared_status,
                );
                if let Err(error) = &result {
                    set_degraded(shared_status, error);
                }
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                }
            }
            WorkerCommand::Synchronize => {
                let anchor = latest_anchor.lock().ok().and_then(|mut value| value.take());
                let Some(anchor) = anchor else { continue };
                if session.is_none()
                    && let Err(error) = open_session(
                        &configuration,
                        &mut owned_child,
                        &mut session,
                        shared_status,
                    )
                {
                    set_degraded(shared_status, &error);
                    continue;
                }
                let Some(connected) = session.as_mut() else {
                    continue;
                };
                match apply_anchor(connected, last_anchor, anchor) {
                    Ok(outcome) => {
                        last_anchor = Some(anchor);
                        update_status(shared_status, |status| {
                            status.state = if anchor.playing {
                                TimingOutputState::Running
                            } else {
                                TimingOutputState::Ready
                            };
                            status.peers = outcome.peers;
                            status.source = Some(anchor.source);
                            status.deck_number = anchor.deck_number;
                            status.bpm_milli = Some(anchor.bpm_milli);
                            status.beat_within_bar = Some(anchor.beat_within_bar);
                            status.playing = anchor.playing;
                            status.generation = Some(anchor.generation);
                            status.last_anchor_at = Some(Instant::now());
                            status.last_anchor_age_millis = Some(0);
                            status.phase_error_micros = outcome.phase_error_micros;
                            status.applied_anchor_count =
                                status.applied_anchor_count.saturating_add(1);
                            if outcome.reanchored {
                                status.hard_reanchor_count =
                                    status.hard_reanchor_count.saturating_add(1);
                            } else if outcome.corrected {
                                status.soft_correction_count =
                                    status.soft_correction_count.saturating_add(1);
                            }
                            status.max_abs_phase_error_micros =
                                status.max_abs_phase_error_micros.max(
                                    outcome
                                        .phase_error_micros
                                        .unwrap_or_default()
                                        .unsigned_abs(),
                                );
                            if outcome.reanchored {
                                status.last_reanchor = Some(anchor.discontinuity);
                            }
                            status.last_event = Some(outcome.event);
                            status.last_error = None;
                        });
                    }
                    Err(error) => {
                        set_degraded(shared_status, &error);
                        session = None;
                    }
                }
            }
            WorkerCommand::Hold => {
                if let Ok(mut value) = latest_anchor.lock() {
                    *value = None;
                }
                let result = session
                    .as_mut()
                    .map_or(Ok(()), CarabinerSession::stop_playing_now);
                if let Err(error) = result {
                    set_degraded(shared_status, &error);
                } else {
                    last_anchor = None;
                    update_status(shared_status, |status| {
                        status.state = TimingOutputState::Ready;
                        status.playing = false;
                        status.last_event = Some("Ableton Link timing held safely".to_owned());
                    });
                }
            }
            WorkerCommand::FailClosed(reason) => {
                let result = session
                    .as_mut()
                    .map_or(Ok(()), CarabinerSession::stop_playing_now);
                last_anchor = None;
                match result {
                    Ok(()) => update_status(shared_status, |status| {
                        status.state = TimingOutputState::Degraded;
                        status.playing = false;
                        status.fail_closed_count = status.fail_closed_count.saturating_add(1);
                        status.last_event = Some(
                            "Ableton Link held because source timing became unsafe".to_owned(),
                        );
                        status.last_error = Some(reason);
                    }),
                    Err(error) => {
                        set_degraded(shared_status, &error);
                        session = None;
                    }
                }
            }
            WorkerCommand::Stop(reply) => {
                if let Ok(mut value) = latest_anchor.lock() {
                    *value = None;
                }
                let result = session
                    .as_mut()
                    .map_or(Ok(()), CarabinerSession::stop_playing_now);
                session = None;
                if let Some(mut child) = owned_child.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                last_anchor = None;
                update_status(shared_status, |status| {
                    let helper_version = status.helper_version.clone();
                    *status = TimingOutputStatus::default();
                    status.helper_version = helper_version;
                    status.last_event = Some("Ableton Link stopped safely".to_owned());
                });
                let _ = reply.send(result);
            }
            WorkerCommand::Shutdown => break,
        }
    }

    if let Some(mut child) = owned_child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn open_session(
    configuration: &CarabinerConfiguration,
    owned_child: &mut Option<Child>,
    session: &mut Option<CarabinerSession>,
    shared_status: &Arc<Mutex<TimingOutputStatus>>,
) -> Result<(), String> {
    let mut connected = connect_or_launch(configuration, owned_child)?;
    let version = connected.version()?;
    if version != configuration.expected_version {
        return Err(format!(
            "expected Carabiner {}, found {version}",
            configuration.expected_version
        ));
    }
    connected.enable_start_stop_sync()?;
    let snapshot = connected.status()?;
    update_status(shared_status, |status| {
        status.state = TimingOutputState::Ready;
        status.helper_version = Some(version);
        status.peers = snapshot.peers;
        status.last_event = Some("Ableton Link ready; waiting for timing source".to_owned());
        status.last_error = None;
    });
    *session = Some(connected);
    Ok(())
}

fn connect_or_launch(
    configuration: &CarabinerConfiguration,
    owned_child: &mut Option<Child>,
) -> Result<CarabinerSession, String> {
    if let Ok(session) = CarabinerSession::connect(configuration.port) {
        return Ok(session);
    }
    let executable = configuration
        .executable
        .as_ref()
        .ok_or_else(|| "managed Carabiner executable is unavailable".to_owned())?;
    if let Some(child) = owned_child.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => *owned_child = None,
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                *owned_child = None;
            }
            Err(_) => *owned_child = None,
        }
    }
    let child = Command::new(executable)
        .arg(format!("--port={}", configuration.port))
        .arg("--poll=10")
        .arg("--daemon")
        .env_remove("LUMI_SESSION_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not launch {}: {error}", executable.display()))?;
    *owned_child = Some(child);
    for _ in 0..START_RETRIES {
        if let Ok(session) = CarabinerSession::connect(configuration.port) {
            return Ok(session);
        }
        thread::sleep(START_RETRY_DELAY);
    }
    Err("managed Carabiner did not open its loopback port".to_owned())
}

struct AnchorOutcome {
    peers: u32,
    phase_error_micros: Option<i64>,
    reanchored: bool,
    corrected: bool,
    event: String,
}

fn apply_anchor(
    session: &mut CarabinerSession,
    previous: Option<TimingAnchor>,
    anchor: TimingAnchor,
) -> Result<AnchorOutcome, String> {
    let tempo_changed = previous.is_none_or(|value| value.bpm_milli != anchor.bpm_milli);
    if tempo_changed {
        session.set_bpm(anchor.bpm_milli)?;
    }

    let transport_changed = previous.is_none_or(|value| value.playing != anchor.playing);
    let generation_changed = previous.is_none_or(|value| value.generation != anchor.generation);
    let explicit_discontinuity = anchor.discontinuity != TimingDiscontinuity::Continuous;
    let snapshot = session.status()?;
    let target_phase = f64::from(anchor.phase_beat());
    let snapshot_time = snapshot.timeline_micros();
    let shared_epoch_time = anchor
        .observed_at_micros
        .filter(|observed| observed.abs_diff(snapshot_time) <= MAX_SHARED_EPOCH_SKEW_MICROS);
    let anchor_time = shared_epoch_time.unwrap_or(snapshot_time);
    let micros_per_beat = 60_000_000.0 / (f64::from(anchor.bpm_milli) / 1_000.0);
    let expected_phase = projected_phase(target_phase, micros_per_beat, anchor_time, snapshot_time);
    let current_phase = positive_modulo(snapshot.beat, 4.0);
    let phase_beats = shortest_phase_delta(current_phase, expected_phase, 4.0);
    let phase_error_micros = (phase_beats * micros_per_beat).round() as i64;
    let should_reanchor = generation_changed
        || explicit_discontinuity
        || phase_error_micros.abs() >= HARD_PHASE_ERROR_MICROS;

    if should_reanchor {
        session.force_beat_at_time(target_phase, anchor_time)?;
    } else if phase_error_micros.abs() >= SOFT_PHASE_ERROR_MICROS {
        session.request_beat_at_time(target_phase, anchor_time)?;
    }

    if transport_changed || generation_changed {
        if anchor.playing {
            session.start_playing_now()?;
        } else {
            session.stop_playing_now()?;
        }
    }

    Ok(AnchorOutcome {
        peers: snapshot.peers,
        phase_error_micros: Some(phase_error_micros),
        reanchored: should_reanchor,
        corrected: !should_reanchor && phase_error_micros.abs() >= SOFT_PHASE_ERROR_MICROS,
        event: if anchor.observed_at_micros.is_some() && shared_epoch_time.is_none() {
            "Ableton Link synchronized with receive-time fallback".to_owned()
        } else if should_reanchor {
            "Ableton Link timeline re-anchored".to_owned()
        } else if tempo_changed {
            "Ableton Link tempo updated with phase preserved".to_owned()
        } else {
            "Ableton Link timing locked".to_owned()
        },
    })
}

fn positive_modulo(value: f64, quantum: f64) -> f64 {
    ((value % quantum) + quantum) % quantum
}

fn projected_phase(
    anchor_phase: f64,
    micros_per_beat: f64,
    anchor_time_micros: u64,
    target_time_micros: u64,
) -> f64 {
    let elapsed_micros = target_time_micros as i128 - anchor_time_micros as i128;
    positive_modulo(anchor_phase + elapsed_micros as f64 / micros_per_beat, 4.0)
}

fn shortest_phase_delta(current: f64, target: f64, quantum: f64) -> f64 {
    let mut delta = target - current;
    if delta > quantum / 2.0 {
        delta -= quantum;
    } else if delta < -(quantum / 2.0) {
        delta += quantum;
    }
    delta
}

struct CarabinerSession {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

#[derive(Clone, Copy, Debug)]
struct LinkSnapshot {
    peers: u32,
    bpm: f64,
    start_micros: u64,
    beat: f64,
}

impl LinkSnapshot {
    fn timeline_micros(self) -> u64 {
        let elapsed = (self.beat * 60_000_000.0 / self.bpm).max(0.0);
        self.start_micros.saturating_add(elapsed.round() as u64)
    }
}

impl CarabinerSession {
    fn connect(port: u16) -> Result<Self, String> {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let stream = TcpStream::connect_timeout(&address, COMMAND_TIMEOUT)
            .map_err(|error| error.to_string())?;
        stream
            .set_read_timeout(Some(COMMAND_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(COMMAND_TIMEOUT))
            .map_err(|error| error.to_string())?;
        let writer = stream.try_clone().map_err(|error| error.to_string())?;
        let mut session = Self {
            reader: BufReader::new(stream),
            writer,
        };
        let _ = session.read_until("status ")?;
        Ok(session)
    }

    fn version(&mut self) -> Result<String, String> {
        let response = self.command("version", "version ")?;
        response
            .strip_prefix("version ")
            .map(|value| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("invalid version response: {response}"))
    }

    fn enable_start_stop_sync(&mut self) -> Result<(), String> {
        self.command("enable-start-stop-sync", "status ")?;
        Ok(())
    }

    fn status(&mut self) -> Result<LinkSnapshot, String> {
        let response = self.command("status", "status ")?;
        parse_status(&response)
    }

    fn set_bpm(&mut self, bpm_milli: u32) -> Result<(), String> {
        self.command(
            &format!("bpm {:.3}", f64::from(bpm_milli) / 1_000.0),
            "status ",
        )?;
        Ok(())
    }

    fn force_beat_at_time(&mut self, beat: f64, when_micros: u64) -> Result<(), String> {
        self.command(
            &format!("force-beat-at-time {beat:.6} {when_micros} 4"),
            "status ",
        )?;
        Ok(())
    }

    fn request_beat_at_time(&mut self, beat: f64, when_micros: u64) -> Result<(), String> {
        self.command(
            &format!("request-beat-at-time {beat:.6} {when_micros} 4"),
            "status ",
        )?;
        Ok(())
    }

    fn start_playing_now(&mut self) -> Result<(), String> {
        let snapshot = self.status()?;
        self.command(
            &format!("start-playing {}", snapshot.timeline_micros()),
            "status ",
        )?;
        Ok(())
    }

    fn stop_playing_now(&mut self) -> Result<(), String> {
        let snapshot = self.status()?;
        self.command(
            &format!("stop-playing {}", snapshot.timeline_micros()),
            "status ",
        )?;
        Ok(())
    }

    fn command(&mut self, command: &str, response_prefix: &str) -> Result<String, String> {
        self.writer
            .write_all(command.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush())
            .map_err(|error| error.to_string())?;
        self.read_until(response_prefix)
    }

    fn read_until(&mut self, response_prefix: &str) -> Result<String, String> {
        loop {
            let mut line = String::new();
            let length = self
                .reader
                .read_line(&mut line)
                .map_err(|error| error.to_string())?;
            if length == 0 {
                return Err("Carabiner closed the timing connection".to_owned());
            }
            let line = line.trim().to_owned();
            if line.starts_with(response_prefix) {
                return Ok(line);
            }
            if line.starts_with("bad-") || line.starts_with("unsupported") {
                return Err(line);
            }
        }
    }
}

fn parse_status(line: &str) -> Result<LinkSnapshot, String> {
    Ok(LinkSnapshot {
        peers: parse_field(line, ":peers")?,
        bpm: parse_field(line, ":bpm")?,
        start_micros: parse_field(line, ":start")?,
        beat: parse_field(line, ":beat")?,
    })
}

fn parse_field<T>(line: &str, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    let value = line
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|pair| (pair[0] == name).then_some(pair[1]))
        .map(|value| value.trim_end_matches('}'))
        .ok_or_else(|| format!("missing {name} in Carabiner status"))?;
    value
        .parse::<T>()
        .map_err(|_| format!("invalid {name} in Carabiner status"))
}

fn update_status(
    shared_status: &Arc<Mutex<TimingOutputStatus>>,
    update: impl FnOnce(&mut TimingOutputStatus),
) {
    if let Ok(mut status) = shared_status.lock() {
        update(&mut status);
    }
}

fn set_degraded(shared_status: &Arc<Mutex<TimingOutputStatus>>, error: &str) {
    update_status(shared_status, |status| {
        status.state = TimingOutputState::Degraded;
        status.failure_count = status.failure_count.saturating_add(1);
        status.last_error = Some(error.to_owned());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_carabiner_status() {
        let Ok(parsed) = parse_status(
            "status { :peers 1 :bpm 130.500000 :start 73743731220 :beat 12.250000 :playing true }",
        ) else {
            panic!("status should parse");
        };
        assert_eq!(parsed.peers, 1);
        assert_eq!(parsed.start_micros, 73_743_731_220);
        assert!((parsed.bpm - 130.5).abs() < f64::EPSILON);
        assert!((parsed.beat - 12.25).abs() < f64::EPSILON);
    }

    #[test]
    fn phase_delta_uses_shortest_direction() {
        assert!((shortest_phase_delta(3.9, 0.0, 4.0) - 0.1).abs() < 0.000_001);
        assert!((shortest_phase_delta(0.1, 3.9, 4.0) + 0.2).abs() < 0.000_001);
    }

    #[test]
    fn source_phase_is_projected_to_the_helper_snapshot_time() {
        let micros_per_beat = 60_000_000.0 / 130.0;
        let phase = projected_phase(2.0, micros_per_beat, 1_000_000, 1_010_000);
        assert!((phase - 2.021_666_666).abs() < 0.000_001);
    }
}
