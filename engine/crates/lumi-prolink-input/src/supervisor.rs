use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use thiserror::Error;

use crate::{BridgeDecodeError, BridgeDecoder, BridgeEvent, BridgeMessage, BridgeTrafficClass};

const STDERR_TAIL_CAPACITY: usize = 40;
const PROCESS_OUTPUT_CAPACITY: usize = 512;
const INGRESS_LATENCY_BOUNDS_MICROS: [u64; 13] = [
    100, 250, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 40_000, 100_000, 250_000, 500_000,
    1_000_000,
];
pub const PRO_DJ_LINK_UDP_PORTS: [u16; 3] = [50_000, 50_001, 50_002];

/// Refuse to start a second Pro DJ Link application on the same network host.
///
/// Rekordbox Export Mode and Beat Link based applications need the same fixed
/// UDP ports. Starting both on one interface can interrupt a track being read
/// from rekordbox by a player, so this check deliberately fails closed before
/// the Java bridge is launched or sends any network traffic.
pub fn ensure_prolink_network_available() -> Result<(), BridgeSupervisorError> {
    ensure_udp_ports_available(&PRO_DJ_LINK_UDP_PORTS)
}

fn ensure_udp_ports_available(ports: &[u16]) -> Result<(), BridgeSupervisorError> {
    ensure_udp_ports_available_with(ports, |port| {
        UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
    })
}

fn ensure_udp_ports_available_with(
    ports: &[u16],
    bind: impl FnMut(u16) -> std::io::Result<UdpSocket>,
) -> Result<(), BridgeSupervisorError> {
    let (occupied_ports, owners) = occupied_udp_ports(ports, bind);
    if !occupied_ports.is_empty() {
        return Err(BridgeSupervisorError::NetworkConflict(
            ProLinkNetworkConflict {
                ports: occupied_ports,
                owners,
            },
        ));
    }
    Ok(())
}

fn occupied_udp_ports(
    ports: &[u16],
    mut bind: impl FnMut(u16) -> std::io::Result<UdpSocket>,
) -> (Vec<u16>, Vec<String>) {
    let mut occupied_ports = Vec::new();
    let mut owners = Vec::new();

    #[cfg(target_os = "macos")]
    for &port in ports {
        if let Some(owner) = macos_udp_port_owner(port) {
            occupied_ports.push(port);
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
    }

    // Keep successful reservations alive until every port has been checked so
    // another local process cannot claim one halfway through this preflight.
    let mut reservations = Vec::new();
    for &port in ports {
        if occupied_ports.contains(&port) {
            continue;
        }
        match bind(port) {
            Ok(socket) => reservations.push(socket),
            Err(_) => occupied_ports.push(port),
        }
    }
    occupied_ports.sort_unstable();
    occupied_ports.dedup();
    (occupied_ports, owners)
}

#[cfg(target_os = "macos")]
fn macos_udp_port_owner(port: u16) -> Option<String> {
    let output = Command::new("/usr/sbin/lsof")
        .args(["-nP", &format!("-iUDP:{port}")])
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .skip(1)
        .find_map(|line| line.split_whitespace().next().map(str::to_owned))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProLinkNetworkConflict {
    pub ports: Vec<u16>,
    pub owners: Vec<String>,
}

impl std::fmt::Display for ProLinkNetworkConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Pro DJ Link cannot start because UDP ports {:?} are already in use",
            self.ports
        )?;
        if !self.owners.is_empty() {
            write!(formatter, " by {}", self.owners.join(", "))?;
        }
        write!(
            formatter,
            ". Close rekordbox or other Pro DJ Link software on this Mac, or run it on a different computer."
        )
    }
}

impl std::error::Error for ProLinkNetworkConflict {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeLaunchConfiguration {
    executable: PathBuf,
    arguments: Vec<String>,
}

impl BridgeLaunchConfiguration {
    #[must_use]
    pub fn java_jar(java_executable: impl Into<PathBuf>, bridge_jar: impl AsRef<Path>) -> Self {
        Self {
            executable: java_executable.into(),
            arguments: vec![
                "-Djava.awt.headless=true".to_owned(),
                "-jar".to_owned(),
                bridge_jar.as_ref().to_string_lossy().into_owned(),
            ],
        }
    }

    #[must_use]
    pub fn command(executable: impl Into<PathBuf>, arguments: Vec<String>) -> Self {
        Self {
            executable: executable.into(),
            arguments,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeProcessDiagnostics {
    pub running: bool,
    pub last_sequence: Option<u64>,
    pub queue_capacity: usize,
    pub queue_depth: usize,
    pub queue_high_water: usize,
    pub coalesced_message_count: u64,
    pub critical_saturation_count: u64,
    pub source_age_sample_count: u64,
    pub source_age_p50_micros: u64,
    pub source_age_p95_micros: u64,
    pub source_age_p99_micros: u64,
    pub source_age_max_micros: u64,
    pub stderr_tail: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct IngressLatencyHistogram {
    buckets: [u64; INGRESS_LATENCY_BOUNDS_MICROS.len() + 1],
    sample_count: u64,
    maximum_micros: u64,
}

impl IngressLatencyHistogram {
    fn record(&mut self, micros: u64) {
        let bucket = INGRESS_LATENCY_BOUNDS_MICROS
            .iter()
            .position(|bound| micros <= *bound)
            .unwrap_or(INGRESS_LATENCY_BOUNDS_MICROS.len());
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        self.sample_count = self.sample_count.saturating_add(1);
        self.maximum_micros = self.maximum_micros.max(micros);
    }

    fn percentile(&self, percentile: u64) -> u64 {
        if self.sample_count == 0 {
            return 0;
        }
        let target = self
            .sample_count
            .saturating_mul(percentile)
            .saturating_add(99)
            / 100;
        let mut cumulative = 0_u64;
        for (index, count) in self.buckets.iter().enumerate() {
            cumulative = cumulative.saturating_add(*count);
            if cumulative >= target {
                return INGRESS_LATENCY_BOUNDS_MICROS
                    .get(index)
                    .copied()
                    .unwrap_or(self.maximum_micros);
            }
        }
        self.maximum_micros
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoalescingKey {
    DeckStatus(u8),
    TempoStatus(u8),
    PrecisePosition(u8),
    TrackMetadata(u8),
    TrackSignature(u8),
}

fn coalescing_key(message: &BridgeMessage) -> Option<CoalescingKey> {
    if message.traffic_class == BridgeTrafficClass::Critical {
        return None;
    }
    match &message.event {
        BridgeEvent::DeckStatus(status) => Some(CoalescingKey::DeckStatus(status.device_number)),
        BridgeEvent::TempoStatus(status) => Some(CoalescingKey::TempoStatus(status.device_number)),
        BridgeEvent::PrecisePosition(position) => {
            Some(CoalescingKey::PrecisePosition(position.device_number))
        }
        BridgeEvent::TrackMetadata(metadata) => {
            Some(CoalescingKey::TrackMetadata(metadata.deck_number))
        }
        BridgeEvent::TrackSignature(signature) => {
            Some(CoalescingKey::TrackSignature(signature.deck_number))
        }
        BridgeEvent::Hello(_)
        | BridgeEvent::SourceStatus(_)
        | BridgeEvent::DeviceFound(_)
        | BridgeEvent::DeviceLost(_)
        | BridgeEvent::Beat(_)
        | BridgeEvent::Error(_) => None,
    }
}

struct ProcessOutputQueue {
    messages: VecDeque<BridgeMessage>,
    received_at: BTreeMap<u64, Instant>,
    capacity: usize,
    high_water: usize,
    coalesced_message_count: u64,
    critical_saturation_count: u64,
    last_sequence: Option<u64>,
    source_age: IngressLatencyHistogram,
    terminal_error: Option<String>,
    stdout_closed: bool,
}

impl ProcessOutputQueue {
    fn new(capacity: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(capacity),
            received_at: BTreeMap::new(),
            capacity,
            high_water: 0,
            coalesced_message_count: 0,
            critical_saturation_count: 0,
            last_sequence: None,
            source_age: IngressLatencyHistogram::default(),
            terminal_error: None,
            stdout_closed: false,
        }
    }

    fn push(&mut self, message: BridgeMessage) -> bool {
        self.last_sequence = Some(message.sequence);
        if let Some(key) = coalescing_key(&message)
            && let Some(index) = self
                .messages
                .iter()
                .position(|queued| coalescing_key(queued) == Some(key))
        {
            if let Some(replaced) = self.messages.remove(index) {
                self.received_at.remove(&replaced.sequence);
            }
            self.coalesced_message_count = self.coalesced_message_count.saturating_add(1);
        }

        if self.messages.len() == self.capacity {
            if let Some(index) = self
                .messages
                .iter()
                .position(|queued| coalescing_key(queued).is_some())
            {
                if let Some(replaced) = self.messages.remove(index) {
                    self.received_at.remove(&replaced.sequence);
                }
                self.coalesced_message_count = self.coalesced_message_count.saturating_add(1);
            } else {
                self.critical_saturation_count = self.critical_saturation_count.saturating_add(1);
                self.terminal_error = Some(format!(
                    "bounded Pro DJ Link ingress saturated at {} critical messages",
                    self.capacity
                ));
                return false;
            }
        }

        self.received_at.insert(message.sequence, Instant::now());
        self.messages.push_back(message);
        self.high_water = self.high_water.max(self.messages.len());
        true
    }

    fn fail(&mut self, message: String) {
        self.terminal_error = Some(message);
    }
}

pub struct BridgeProcessSupervisor {
    child: Child,
    stdin: Option<ChildStdin>,
    output: Arc<Mutex<ProcessOutputQueue>>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

impl BridgeProcessSupervisor {
    pub fn spawn(configuration: &BridgeLaunchConfiguration) -> Result<Self, BridgeSupervisorError> {
        let mut child = Command::new(&configuration.executable)
            .args(&configuration.arguments)
            .env_remove("LUMI_SESSION_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(BridgeSupervisorError::Launch)?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or(BridgeSupervisorError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(BridgeSupervisorError::MissingPipe("stderr"))?;

        let output = Arc::new(Mutex::new(ProcessOutputQueue::new(PROCESS_OUTPUT_CAPACITY)));
        let output_lines = Arc::clone(&output);
        thread::Builder::new()
            .name("lumi-prolink-stdout".to_owned())
            .spawn(move || {
                let mut decoder = BridgeDecoder::new();
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) => {
                            let message = match decoder.decode_line(&line) {
                                Ok(message) => message,
                                Err(error) => {
                                    if let Ok(mut queue) = output_lines.lock() {
                                        queue.fail(format!(
                                            "invalid Pro DJ Link bridge message: {error}"
                                        ));
                                    }
                                    return;
                                }
                            };
                            let Ok(mut queue) = output_lines.lock() else {
                                return;
                            };
                            if !queue.push(message) {
                                return;
                            }
                        }
                        Err(error) => {
                            if let Ok(mut queue) = output_lines.lock() {
                                queue.fail(error.to_string());
                            }
                            return;
                        }
                    }
                }
                if let Ok(mut queue) = output_lines.lock() {
                    queue.stdout_closed = true;
                }
            })
            .map_err(BridgeSupervisorError::ReaderThread)?;

        let stderr_tail = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_CAPACITY)));
        let stderr_lines = Arc::clone(&stderr_tail);
        thread::Builder::new()
            .name("lumi-prolink-stderr".to_owned())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let Ok(mut lines) = stderr_lines.lock() else {
                        return;
                    };
                    if lines.len() == STDERR_TAIL_CAPACITY {
                        lines.pop_front();
                    }
                    lines.push_back(line);
                }
            })
            .map_err(BridgeSupervisorError::ReaderThread)?;

        Ok(Self {
            child,
            stdin,
            output,
            stderr_tail,
        })
    }

    pub fn drain_messages(&mut self) -> Result<Vec<BridgeMessage>, BridgeSupervisorError> {
        let mut output = self
            .output
            .lock()
            .map_err(|_| BridgeSupervisorError::OutputLock)?;
        if let Some(message) = output.terminal_error.take() {
            return Err(BridgeSupervisorError::Read(message));
        }
        let mut messages: Vec<_> = output.messages.drain(..).collect();
        for message in &messages {
            let supervisor_age = output
                .received_at
                .remove(&message.sequence)
                .map_or(0, |received| {
                    u64::try_from(received.elapsed().as_micros()).unwrap_or(u64::MAX)
                });
            output.source_age.record(
                message
                    .bridge_queue_age_micros
                    .saturating_add(supervisor_age),
            );
        }
        messages.sort_by_key(|message| {
            let priority = match message.traffic_class {
                BridgeTrafficClass::Critical => 0_u8,
                BridgeTrafficClass::Tempo => 1,
                BridgeTrafficClass::Transport => 2,
                BridgeTrafficClass::Display => 3,
            };
            (priority, message.sequence)
        });
        Ok(messages)
    }

    pub fn diagnostics(&mut self) -> Result<BridgeProcessDiagnostics, BridgeSupervisorError> {
        let process_running = self
            .child
            .try_wait()
            .map_err(BridgeSupervisorError::Status)?
            .is_none();
        let output = self
            .output
            .lock()
            .map_err(|_| BridgeSupervisorError::OutputLock)?;
        let stderr_tail = self
            .stderr_tail
            .lock()
            .map_err(|_| BridgeSupervisorError::StderrLock)?
            .iter()
            .cloned()
            .collect();
        Ok(BridgeProcessDiagnostics {
            running: process_running && !output.stdout_closed && output.terminal_error.is_none(),
            last_sequence: output.last_sequence,
            queue_capacity: output.capacity,
            queue_depth: output.messages.len(),
            queue_high_water: output.high_water,
            coalesced_message_count: output.coalesced_message_count,
            critical_saturation_count: output.critical_saturation_count,
            source_age_sample_count: output.source_age.sample_count,
            source_age_p50_micros: output.source_age.percentile(50),
            source_age_p95_micros: output.source_age.percentile(95),
            source_age_p99_micros: output.source_age.percentile(99),
            source_age_max_micros: output.source_age.maximum_micros,
            stderr_tail,
        })
    }

    pub fn stop(&mut self) -> Result<(), BridgeSupervisorError> {
        self.stdin.take();
        if self
            .child
            .try_wait()
            .map_err(BridgeSupervisorError::Status)?
            .is_none()
        {
            self.child.kill().map_err(BridgeSupervisorError::Stop)?;
        }
        self.child.wait().map_err(BridgeSupervisorError::Stop)?;
        Ok(())
    }
}

impl Drop for BridgeProcessSupervisor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Debug, Error)]
pub enum BridgeSupervisorError {
    #[error("{0}")]
    NetworkConflict(#[from] ProLinkNetworkConflict),
    #[error("failed to launch Pro DJ Link bridge: {0}")]
    Launch(std::io::Error),
    #[error("the Pro DJ Link bridge did not expose its {0} pipe")]
    MissingPipe(&'static str),
    #[error("failed to start Pro DJ Link bridge reader: {0}")]
    ReaderThread(std::io::Error),
    #[error("failed to read Pro DJ Link bridge output: {0}")]
    Read(String),
    #[error("invalid Pro DJ Link bridge message: {0}")]
    Decode(#[from] BridgeDecodeError),
    #[error("failed to inspect Pro DJ Link bridge process: {0}")]
    Status(std::io::Error),
    #[error("failed to stop Pro DJ Link bridge process: {0}")]
    Stop(std::io::Error),
    #[error("the Pro DJ Link bridge diagnostics lock is poisoned")]
    StderrLock,
    #[error("the Pro DJ Link bridge output lock is poisoned")]
    OutputLock,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn occupied_udp_port_fails_the_same_preflight_used_by_pro_dj_link() {
        let port = 65_535;
        let result = ensure_udp_ports_available_with(&[port], |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "test reservation",
            ))
        });
        let Err(error) = result else {
            panic!("an occupied UDP port must fail closed");
        };
        assert!(matches!(error, BridgeSupervisorError::NetworkConflict(_)));
        assert!(error.to_string().contains(&port.to_string()));
    }

    #[test]
    fn an_exited_bridge_is_detected_without_blocking_the_supervisor() {
        let configuration = BridgeLaunchConfiguration::command("/usr/bin/true", Vec::new());
        let mut supervisor = BridgeProcessSupervisor::spawn(&configuration)
            .unwrap_or_else(|error| panic!("test process should launch: {error}"));
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let diagnostics = supervisor
                .diagnostics()
                .unwrap_or_else(|error| panic!("process should be inspectable: {error}"));
            if !diagnostics.running {
                break;
            }
            assert!(Instant::now() < deadline, "exited process stayed healthy");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn deck_status_message(sequence: u64, device_number: u8, beat_number: i64) -> BridgeMessage {
        BridgeMessage {
            sequence,
            observed_at_nanos: sequence,
            traffic_class: crate::BridgeTrafficClass::Transport,
            bridge_queue_age_micros: 0,
            event: BridgeEvent::DeckStatus(crate::DeckStatus {
                device_number,
                device_name: format!("Player {device_number}"),
                playing: true,
                paused: false,
                cued: false,
                tempo_master: device_number == 1,
                on_air: true,
                source_player: device_number,
                source_slot: "usb".to_owned(),
                track_type: "rekordbox".to_owned(),
                rekordbox_id: 42,
                track_bpm: 140.0,
                effective_bpm: 140.0,
                beat_number,
                beat_within_bar: 1,
                raw_pitch: 0,
            }),
        }
    }

    fn beat_message(sequence: u64) -> BridgeMessage {
        BridgeMessage {
            sequence,
            observed_at_nanos: sequence,
            traffic_class: crate::BridgeTrafficClass::Critical,
            bridge_queue_age_micros: 0,
            event: BridgeEvent::Beat(crate::Beat {
                device_number: 1,
                device_name: "Player 1".to_owned(),
                effective_bpm: 140.0,
                beat_within_bar: 1,
                tempo_master: true,
            }),
        }
    }

    #[test]
    fn continuous_status_is_coalesced_but_exact_beats_keep_order() {
        let mut queue = ProcessOutputQueue::new(4);
        assert!(queue.push(deck_status_message(1, 1, 1)));
        assert!(queue.push(beat_message(2)));
        assert!(queue.push(deck_status_message(3, 1, 2)));
        assert!(queue.push(beat_message(4)));

        assert_eq!(queue.messages.len(), 3);
        assert_eq!(queue.coalesced_message_count, 1);
        assert_eq!(queue.last_sequence, Some(4));
        assert!(matches!(queue.messages[0].event, BridgeEvent::Beat(_)));
        assert!(matches!(
            queue.messages[1].event,
            BridgeEvent::DeckStatus(_)
        ));
        assert!(matches!(queue.messages[2].event, BridgeEvent::Beat(_)));
    }

    #[test]
    fn all_critical_saturation_fails_closed_without_growing() {
        let mut queue = ProcessOutputQueue::new(2);
        assert!(queue.push(beat_message(1)));
        assert!(queue.push(beat_message(2)));
        assert!(!queue.push(beat_message(3)));

        assert_eq!(queue.messages.len(), 2);
        assert_eq!(queue.high_water, 2);
        assert_eq!(queue.critical_saturation_count, 1);
        assert!(queue.terminal_error.is_some());
    }

    #[test]
    fn fifty_thousand_status_updates_remain_constant_space_and_within_budget() {
        let mut queue = ProcessOutputQueue::new(PROCESS_OUTPUT_CAPACITY);
        let started = Instant::now();
        for sequence in 1..=50_000_u64 {
            assert!(queue.push(deck_status_message(
                sequence,
                1,
                i64::try_from(sequence).unwrap_or(i64::MAX),
            )));
        }
        let elapsed = started.elapsed();

        eprintln!(
            "Pro DJ Link bounded ingress benchmark: updates=50000 elapsed={elapsed:?} depth={} high_water={} coalesced={}",
            queue.messages.len(),
            queue.high_water,
            queue.coalesced_message_count,
        );
        assert_eq!(queue.messages.len(), 1);
        assert_eq!(queue.high_water, 1);
        assert_eq!(queue.coalesced_message_count, 49_999);
        assert_eq!(queue.critical_saturation_count, 0);
        assert!(
            elapsed < Duration::from_secs(1),
            "status coalescing exceeded its one-second debug-build budget"
        );
    }

    #[test]
    fn ingress_latency_histogram_is_bounded_and_reports_release_percentiles() {
        let mut histogram = IngressLatencyHistogram::default();
        for micros in [50, 200, 750, 1_500, 4_000, 9_000, 19_000, 39_000, 80_000] {
            histogram.record(micros);
        }

        assert_eq!(histogram.sample_count, 9);
        assert_eq!(histogram.percentile(50), 5_000);
        assert_eq!(histogram.percentile(95), 100_000);
        assert_eq!(histogram.percentile(99), 100_000);
        assert_eq!(histogram.maximum_micros, 80_000);
        assert_eq!(histogram.buckets.len(), 14);
    }
}
