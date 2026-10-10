//! USB identity discovery only. Network and SQLite reads run on a separate
//! bounded worker; this module never changes transport, plans or MIDI output.
use std::{
    collections::BTreeMap,
    io::Read,
    net::Ipv4Addr,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use lumi_library_sqlite::{SqliteLibraryRepository, UsbMediaTrust};
use lumi_prolink_input::{BridgeEvent, SourceCondition, USBMountState};
use serde::{Deserialize, Serialize};

const PROCESS_LIMIT: Duration = Duration::from_secs(8);
const MAX_OUTPUT: u64 = 16_384;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaStatus {
    pub state: &'static str,
    pub source_id: Option<String>,
    pub source_name: Option<String>,
    pub color_id: Option<u8>,
    pub generation: u64,
    pub last_verified_unix_millis: Option<u64>,
    pub elapsed_millis: Option<u64>,
    pub detail: String,
}

struct Slot {
    address: String,
    generation: u64,
    next_attempt: Instant,
    failures: u32,
    cancel: Arc<AtomicBool>,
    status: MediaStatus,
    media_id: Option<String>,
    mount_state: Option<USBMountState>,
    read_needed: bool,
}

struct Job {
    player: u8,
    address: String,
    generation: u64,
    cancel: Arc<AtomicBool>,
}

struct Completion {
    player: u8,
    generation: u64,
    outcome: Outcome,
}

#[derive(Debug)]
enum Outcome {
    Trusted {
        media_id: String,
        source_id: String,
        name: String,
        elapsed: u64,
    },
    Unknown,
    Conflict,
    Failed(String),
}

pub(crate) struct MediaResolver {
    slots: BTreeMap<u8, Slot>,
    next_generation: u64,
    jobs: Option<SyncSender<Job>>,
    results: Receiver<Completion>,
    inflight: Option<(u8, u64)>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl MediaResolver {
    #[cfg(test)]
    pub(crate) fn verified_fixture(player: u8, source: &str, name: &str, color: u8) -> Self {
        let mut resolver = Self::disabled();
        resolver.observe(
            &BridgeEvent::DeviceFound(lumi_prolink_input::Device {
                device_number: player,
                device_name: "CDJ-1500X".into(),
                address: "192.168.1.1".into(),
            }),
            Instant::now(),
        );
        if let Some(slot) = resolver.slots.get_mut(&player) {
            slot.status.state = "trusted";
            slot.status.source_id = Some(source.into());
            slot.status.source_name = Some(name.into());
            slot.status.color_id = Some(color);
            slot.status.last_verified_unix_millis = Some(1);
            slot.read_needed = false;
        }
        resolver
    }

    #[cfg(test)]
    pub(crate) fn with_fixture_generation(mut self, generation: u64) -> Self {
        self.next_generation = generation;
        for slot in self.slots.values_mut() {
            slot.generation = generation;
            slot.status.generation = generation;
        }
        self
    }

    pub fn disabled() -> Self {
        let (_, results) = mpsc::sync_channel(4);
        Self {
            slots: BTreeMap::new(),
            next_generation: 0,
            jobs: None,
            results,
            inflight: None,
            shutdown: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    #[cfg_attr(test, allow(dead_code))]
    pub fn new(java: PathBuf, jar: PathBuf, database: PathBuf) -> Self {
        Self::spawn(move |job, shutdown| {
            let command = ReaderCommand {
                executable: java.clone(),
                arguments: vec![
                    "-Xmx64m".to_owned(),
                    "-Djava.awt.headless=true".to_owned(),
                    "-cp".to_owned(),
                    jar.to_string_lossy().into_owned(),
                    "co.victorblan.tech.lumi.prolink.MediaIdentityProbeMain".to_owned(),
                    "--worker-marker".to_owned(),
                    job.address.clone(),
                ],
            };
            let bytes = match read_process(&command, job, shutdown, PROCESS_LIMIT) {
                Ok(bytes) => bytes,
                Err(detail) => return Outcome::Failed(detail),
            };
            resolve_reply(&bytes, &job.address, &database)
        })
    }

    fn spawn<F>(read: F) -> Self
    where
        F: Fn(&Job, &AtomicBool) -> Outcome + Send + 'static,
    {
        let (jobs, requests) = mpsc::sync_channel::<Job>(1);
        let (sender, results) = mpsc::sync_channel(4);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stopped = shutdown.clone();
        let worker = thread::Builder::new()
            .name("lumi-usb-media".to_owned())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let job = match requests.recv_timeout(Duration::from_millis(50)) {
                        Ok(job) => job,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let outcome = read(&job, &stopped);
                    let _ = sender.try_send(Completion {
                        player: job.player,
                        generation: job.generation,
                        outcome,
                    });
                }
            })
            .ok();
        Self {
            slots: BTreeMap::new(),
            next_generation: 0,
            jobs: worker.as_ref().map(|_| jobs),
            results,
            inflight: None,
            shutdown,
            worker,
        }
    }

    /// Observe individual discovery events, including loss/found in the same
    /// bridge batch. Looking only at a final device map would miss that reset.
    pub fn observe(&mut self, event: &BridgeEvent, now: Instant) {
        match event {
            BridgeEvent::USBMount(mount) => {
                self.observe_mount(mount.device_number, mount.state, now);
            }
            BridgeEvent::USBMedia(media) => {
                if let Some(slot) = self.slots.get_mut(&media.device_number) {
                    slot.status.color_id = media.color_id;
                }
            }
            BridgeEvent::DeviceLost(device) => self.remove(device.device_number),
            BridgeEvent::SourceStatus(status)
                if matches!(
                    status.status,
                    SourceCondition::Stopped | SourceCondition::Degraded
                ) =>
            {
                self.clear()
            }
            BridgeEvent::DeviceFound(device) if (1..=4).contains(&device.device_number) => {
                if self
                    .slots
                    .get(&device.device_number)
                    .is_some_and(|slot| slot.address == device.address)
                {
                    return;
                }
                self.remove(device.device_number);
                self.next_generation = self.next_generation.saturating_add(1);
                let generation = self.next_generation;
                self.slots.insert(
                    device.device_number,
                    Slot {
                        address: device.address.clone(),
                        generation,
                        next_attempt: now,
                        failures: 0,
                        cancel: Arc::new(AtomicBool::new(false)),
                        status: MediaStatus {
                            state: "resolving",
                            source_id: None,
                            source_name: None,
                            color_id: None,
                            generation,
                            last_verified_unix_millis: None,
                            elapsed_millis: None,
                            detail: "Reading USB identity independently of show output".to_owned(),
                        },
                        media_id: None,
                        mount_state: None,
                        read_needed: true,
                    },
                );
            }
            _ => {}
        }
    }

    fn remove(&mut self, player: u8) {
        if let Some(slot) = self.slots.remove(&player) {
            slot.cancel.store(true, Ordering::Release);
        }
    }

    pub fn clear(&mut self) {
        for slot in self.slots.values() {
            slot.cancel.store(true, Ordering::Release);
        }
        self.slots.clear();
    }

    pub fn status(&self, player: u8) -> Option<&MediaStatus> {
        self.slots.get(&player).map(|slot| &slot.status)
    }

    /// A healthy, unchanged mount can authorize subsequent loads without
    /// another network read. Only an unresolved mount needs work; track loads
    /// never invalidate identity belonging to other cached tracks.
    pub fn request_verification(&mut self, player: u8, now: Instant) {
        let Some(slot) = self.slots.get_mut(&player) else {
            return;
        };
        if slot.status.state != "trusted"
            && !matches!(
                slot.mount_state,
                Some(USBMountState::Empty | USBMountState::Unloading)
            )
            && !slot.read_needed
            && !matches!(slot.status.state, "unknown" | "conflict")
        {
            slot.read_needed = true;
            slot.next_attempt = now;
        }
    }

    fn observe_mount(&mut self, player: u8, state: USBMountState, now: Instant) {
        let Some(slot) = self.slots.get_mut(&player) else {
            return;
        };
        if slot.mount_state == Some(state) {
            return;
        }
        let first_loaded = slot.mount_state.is_none() && state == USBMountState::Loaded;
        slot.mount_state = Some(state);
        if first_loaded {
            // Device discovery may already have started the first bounded
            // read. The first Loaded status confirms that same mount.
            return;
        }
        slot.cancel.store(true, Ordering::Release);
        slot.cancel = Arc::new(AtomicBool::new(false));
        self.next_generation = self.next_generation.saturating_add(1);
        slot.generation = self.next_generation;
        slot.status.generation = slot.generation;
        slot.status.source_id = None;
        slot.status.source_name = None;
        slot.status.color_id = None;
        slot.status.last_verified_unix_millis = None;
        slot.status.elapsed_millis = None;
        slot.media_id = None;
        slot.failures = 0;
        slot.next_attempt = now;
        slot.read_needed = matches!(state, USBMountState::Loaded | USBMountState::Unknown);
        (slot.status.state, slot.status.detail) = match state {
            USBMountState::Empty => ("empty", "No USB is mounted in this Player".into()),
            USBMountState::Unloading => (
                "unloading",
                "USB is being ejected; loaded tracks retain their origin".into(),
            ),
            USBMountState::Loaded => ("resolving", "Identifying the newly mounted USB".into()),
            USBMountState::Unknown => (
                "resolving",
                "USB mount state is unknown; verifying identity once".into(),
            ),
        };
    }

    /// Bounded, non-blocking pump access: at most four replies and one send.
    /// No file reads, process launches, parsing or SQLite on this path.
    pub fn poll(&mut self, now: Instant) {
        for _ in 0..4 {
            let result = match self.results.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.jobs.take();
                    self.inflight = None;
                    break;
                }
            };
            if self.inflight == Some((result.player, result.generation)) {
                self.inflight = None;
            }
            let duplicate = match &result.outcome {
                Outcome::Trusted { media_id, .. } => {
                    self.slots.iter().find_map(|(player, other)| {
                        (*player != result.player
                            && other.media_id.as_ref() == Some(media_id)
                            && matches!(other.status.state, "trusted" | "conflict"))
                        .then_some(*player)
                    })
                }
                _ => None,
            };
            let Some(slot) = self
                .slots
                .get_mut(&result.player)
                .filter(|slot| slot.generation == result.generation)
            else {
                continue;
            };
            // A failed read is not evidence of a different medium. Retain the
            // established epoch for already-bound loads; a new load still
            // waits because request_verification clears its verification time.
            // Explicit Unknown/Conflict, device loss and a different marker
            // below continue to revoke trust rather than guessing.
            if let Outcome::Failed(detail) = &result.outcome
                && slot.status.state == "trusted"
                && slot.status.source_id.is_some()
            {
                slot.failures = slot.failures.saturating_add(1);
                slot.status.detail =
                    format!("USB recheck delayed; last verified identity retained: {detail}");
                slot.next_attempt = now + retry_backoff(slot.failures);
                continue;
            }
            slot.status.source_id = None;
            slot.status.source_name = None;
            slot.status.elapsed_millis = None;
            let (state, detail) = match result.outcome {
                Outcome::Trusted {
                    media_id,
                    source_id,
                    name,
                    elapsed,
                } => {
                    if slot.media_id.as_ref().is_some_and(|old| old != &media_id) {
                        slot.status.color_id = None;
                        self.next_generation = self.next_generation.saturating_add(1);
                        slot.generation = self.next_generation;
                        slot.status.generation = slot.generation;
                    }
                    slot.media_id = Some(media_id);
                    slot.status.source_id = Some(source_id);
                    slot.status.source_name = Some(name);
                    slot.status.elapsed_millis = Some(elapsed);
                    slot.status.last_verified_unix_millis = Some(unix_millis());
                    slot.failures = 0;
                    slot.read_needed = false;
                    ("trusted", "Trusted USB identified".to_owned())
                }
                Outcome::Unknown => (
                    "unknown",
                    "Connect this USB to the Mac and scan it in Import & Sources".to_owned(),
                ),
                Outcome::Conflict => (
                    "conflict",
                    "USB identity conflicts with its local registration; review the source on the Mac".to_owned(),
                ),
                Outcome::Failed(detail) => ("unavailable", detail),
            };
            slot.status.state = state;
            slot.status.detail = detail.to_owned();
            if state != "trusted" {
                slot.status.color_id = None;
                slot.status.last_verified_unix_millis = None;
                slot.failures = slot.failures.saturating_add(1);
            }
            if matches!(state, "unknown" | "conflict") {
                slot.read_needed = false;
            }
            slot.next_attempt = now + retry_backoff(slot.failures);
            if let Some(other) = duplicate {
                for player in [result.player, other] {
                    if let Some(slot) = self.slots.get_mut(&player) {
                        slot.status.state = "conflict";
                        slot.status.source_id = None;
                        slot.status.source_name = None;
                        slot.status.color_id = None;
                        slot.status.last_verified_unix_millis = None;
                        slot.status.detail = "The same USB marker was found on two Players; review the local sources".to_owned();
                    }
                }
            }
        }
        if self.inflight.is_some() {
            return;
        }
        let Some((&player, slot)) = self
            .slots
            .iter_mut()
            .filter(|(_, slot)| slot.read_needed && slot.next_attempt <= now)
            .min_by_key(|(_, slot)| slot.next_attempt)
        else {
            return;
        };
        let Some(jobs) = self.jobs.as_ref() else {
            slot.status.state = "unavailable";
            slot.status.detail = "USB identity reader is unavailable in this build".to_owned();
            slot.next_attempt = now + Duration::from_secs(30);
            return;
        };
        let job = Job {
            player,
            address: slot.address.clone(),
            generation: slot.generation,
            cancel: slot.cancel.clone(),
        };
        if jobs.try_send(job).is_ok() {
            self.inflight = Some((player, slot.generation));
        }
    }
}

fn retry_backoff(failures: u32) -> Duration {
    Duration::from_secs(2_u64.saturating_pow(failures.min(5)).min(30))
}

impl Drop for MediaResolver {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.clear();
        self.jobs.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct ReaderCommand {
    executable: PathBuf,
    arguments: Vec<String>,
}

fn read_process(
    command: &ReaderCommand,
    job: &Job,
    shutdown: &AtomicBool,
    limit: Duration,
) -> Result<Vec<u8>, String> {
    if !private_player_address(&job.address) {
        return Err("Only a discovered private IPv4 Player address is allowed".into());
    }
    if job.cancel.load(Ordering::Acquire) || shutdown.load(Ordering::Acquire) {
        return Err("USB identity request cancelled".into());
    }
    let mut child = Command::new(&command.executable)
        .args(&command.arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Could not start the isolated USB identity reader")?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("USB identity reader has no response channel".into());
    };
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(MAX_OUTPUT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + limit;
    let outcome = loop {
        if job.cancel.load(Ordering::Acquire) || shutdown.load(Ordering::Acquire) {
            break Err("USB identity request cancelled".to_owned());
        }
        if Instant::now() >= deadline {
            break Err("USB identity reader timed out; show output continues".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break Ok(status);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => break Err("USB identity reader stopped unexpectedly".to_owned()),
        }
    };
    // Direct --worker-marker starts no children. Closing/reaping it also closes
    // stdout, so the bounded response reader cannot outlive engine shutdown.
    if outcome.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let bytes = reader
        .join()
        .map_err(|_| "USB identity response reader failed")?
        .map_err(|_| "USB identity response could not be read")?;
    let status = outcome?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("USB identity response exceeded its size limit".into());
    }
    if !status.success() {
        return Err(format!("{} ({status})", reader_failure_detail(&bytes)));
    }
    Ok(bytes)
}

fn reader_failure_detail(bytes: &[u8]) -> String {
    let reply = serde_json::from_slice::<serde_json::Value>(bytes).ok();
    let detail = reply
        .as_ref()
        .and_then(|value| value.get("detail"))
        .and_then(serde_json::Value::as_str);
    let known_failure = reply
        .as_ref()
        .and_then(|value| value.get("outcome"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|code| {
            matches!(
                code,
                "rpc_timeout"
                    | "rpc_or_io_failure"
                    | "file_unavailable"
                    | "service_unavailable"
                    | "size_rejected"
                    | "not_regular_file"
                    | "truncated_read"
                    | "response_size_rejected"
                    | "deadline"
                    | "media_changed"
                    | "invalid_marker"
                    | "invalid_arguments"
                    | "process_deadline"
                    | "marker_missing"
                    | "access_denied"
                    | "nfs_error"
                    | "cleanup_failed"
            )
        });
    if known_failure && let Some(detail) = detail {
        let bounded: String = detail
            .chars()
            .filter(|character| !character.is_control())
            .take(256)
            .collect();
        if !bounded.is_empty() {
            return format!("USB identity reader: {bounded}");
        }
    }
    "USB identity unavailable; check the Player's USB and local registration".into()
}

fn private_player_address(value: &str) -> bool {
    value
        .parse::<Ipv4Addr>()
        .is_ok_and(|ip| (ip.is_private() || ip.is_link_local()) && ip.octets()[3] != 255)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkerReply {
    outcome: String,
    player_address: String,
    mount: String,
    path: String,
    #[serde(rename = "bytes")]
    byte_count: usize,
    sha256: String,
    elapsed_millis: f64,
    media_id: String,
    source_id: String,
}

fn resolve_reply(bytes: &[u8], address: &str, database: &std::path::Path) -> Outcome {
    let Ok(reply) = serde_json::from_slice::<MarkerReply>(bytes) else {
        return Outcome::Failed("Invalid USB identity response".into());
    };
    let marker = crate::usb_media_identity::MediaIdentity {
        schema_version: 1,
        media_id: reply.media_id,
        source_id: reply.source_id,
    };
    if reply.outcome != "marker_read"
        || reply.player_address != address
        || reply.mount != "/C/"
        || reply.path != "/.lumi-media.json"
        || !(1..=4096).contains(&reply.byte_count)
        || !marker.valid()
        || reply.sha256.len() != 64
        || !reply.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        || !reply.elapsed_millis.is_finite()
        || !(0.0..=8000.0).contains(&reply.elapsed_millis)
    {
        return Outcome::Failed("Invalid USB identity response".into());
    }
    let trust = SqliteLibraryRepository::open_read_only(database)
        .and_then(|reader| reader.trusted_usb_media(&marker.media_id, &marker.source_id));
    match trust {
        Ok(UsbMediaTrust::Trusted(media)) => Outcome::Trusted {
            media_id: marker.media_id,
            source_id: media.source_id,
            name: media.display_name,
            elapsed: reply.elapsed_millis as u64,
        },
        Ok(UsbMediaTrust::Unknown) => Outcome::Unknown,
        Ok(UsbMediaTrust::Conflict) => Outcome::Conflict,
        Err(_) => Outcome::Failed(
            "Local USB registration could not be read; retrying independently".into(),
        ),
    }
}

fn unix_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn child_failure_diagnostic_is_bounded_and_never_accepts_marker_success() {
        let detail = super::reader_failure_detail(
            br#"{"outcome":"rpc_or_io_failure","detail":"Network\n denied"}"#,
        );
        assert_eq!(detail, "USB identity reader: Network denied");
        let oversized =
            serde_json::json!({"outcome":"rpc_timeout", "detail":"x".repeat(4096)}).to_string();
        assert!(super::reader_failure_detail(oversized.as_bytes()).len() < 300);
        assert!(
            !super::reader_failure_detail(br#"{"outcome":"marker_read","detail":"UNTRUSTED"}"#)
                .contains("UNTRUSTED")
        );
    }

    #[test]
    fn actual_nfs_failure_categories_preserve_the_bounded_diagnostic() {
        for outcome in [
            "marker_missing",
            "access_denied",
            "nfs_error",
            "cleanup_failed",
        ] {
            let reply = serde_json::json!({"outcome": outcome, "detail": "USB\n read failed"});
            assert_eq!(
                super::reader_failure_detail(reply.to_string().as_bytes()),
                "USB identity reader: USB read failed"
            );
        }
    }
    use super::*;
    use lumi_prolink_input::Device;
    fn found(number: u8, address: &str) -> BridgeEvent {
        BridgeEvent::DeviceFound(Device {
            device_number: number,
            device_name: "CDJ-1500X".to_owned(),
            address: address.to_owned(),
        })
    }
    fn poll_until(resolver: &mut MediaResolver, condition: impl Fn(&MediaResolver) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !condition(resolver) {
            assert!(Instant::now() < deadline, "resolver test deadline");
            resolver.poll(Instant::now());
            thread::sleep(Duration::from_millis(2));
        }
    }
    fn verified_reply(name: &str) -> Outcome {
        Outcome::Trusted {
            media_id: name.into(),
            source_id: format!("usb-fs:{name}"),
            name: name.into(),
            elapsed: 1,
        }
    }

    fn force_read(resolver: &mut MediaResolver) {
        if let Some(slot) = resolver.slots.get_mut(&1) {
            slot.next_attempt = Instant::now();
            slot.read_needed = true;
        }
    }

    fn mount(player: u8, state: USBMountState) -> BridgeEvent {
        BridgeEvent::USBMount(lumi_prolink_input::USBMount {
            device_number: player,
            state,
        })
    }

    #[test]
    fn healthy_mount_and_track_changes_do_not_repeat_network_reads()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let reads = calls.clone();
        let mut resolver = MediaResolver::spawn(move |_, _| {
            reads.fetch_add(1, Ordering::Relaxed);
            verified_reply("CHRM")
        });
        let now = Instant::now();
        resolver.observe(&found(1, "192.168.1.1"), now);
        resolver.observe(&mount(1, USBMountState::Loaded), now);
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        let initial = resolver.status(1).ok_or("missing status")?.clone();
        for second in 1..=10_000 {
            let later = now + Duration::from_secs(second);
            resolver.observe(&mount(1, USBMountState::Loaded), later);
            resolver.request_verification(1, later);
            resolver.poll(later);
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            resolver.status(1).ok_or("missing status")?.generation,
            initial.generation
        );
        assert_eq!(
            resolver
                .status(1)
                .ok_or("missing status")?
                .last_verified_unix_millis,
            initial.last_verified_unix_millis
        );
        Ok(())
    }

    #[test]
    fn empty_mount_stops_reads_and_insertion_resolves_only_the_new_epoch()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let reads = calls.clone();
        let mut resolver = MediaResolver::spawn(move |_, _| {
            let call = reads.fetch_add(1, Ordering::Relaxed);
            verified_reply(if call == 0 { "CHRM" } else { "GRAY" })
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        resolver.observe(&mount(1, USBMountState::Loaded), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        let old = resolver.status(1).ok_or("missing status")?.generation;
        resolver.observe(&mount(1, USBMountState::Unloading), Instant::now());
        resolver.observe(&mount(1, USBMountState::Empty), Instant::now());
        for _ in 0..100 {
            resolver.request_verification(1, Instant::now());
            resolver.poll(Instant::now() + Duration::from_secs(3600));
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(resolver.status(1).ok_or("missing status")?.state, "empty");
        assert!(
            resolver
                .status(1)
                .ok_or("missing status")?
                .source_id
                .is_none()
        );
        resolver.observe(&mount(1, USBMountState::Loaded), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        let current = resolver.status(1).ok_or("missing status")?;
        assert!(current.generation > old);
        assert_eq!(current.source_name.as_deref(), Some("GRAY"));
        Ok(())
    }

    #[test]
    fn explicitly_requested_failed_reads_retain_identity_and_back_off_until_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut resolver = MediaResolver::spawn(move |_, _| {
            let call = calls.fetch_add(1, Ordering::Relaxed) + 1;
            if (2..=9).contains(&call) {
                Outcome::Failed("ONC/RPC call timed out".into())
            } else {
                verified_reply("CHRM")
            }
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .status
            .color_id = Some(1);
        let verified = resolver.status(1).ok_or("missing status")?.clone();
        for failure in 1..=8 {
            let now = Instant::now();
            force_read(&mut resolver);
            resolver
                .slots
                .get_mut(&1)
                .ok_or("missing slot")?
                .next_attempt = now;
            poll_until(&mut resolver, |r| {
                r.slots.get(&1).is_some_and(|s| s.failures == failure)
            });
            let slot = resolver.slots.get(&1).ok_or("missing slot")?;
            assert_eq!(slot.status.state, "trusted");
            assert_eq!(slot.status.source_id, verified.source_id);
            assert_eq!(slot.status.source_name, verified.source_name);
            assert_eq!(slot.status.color_id, verified.color_id);
            assert_eq!(slot.status.generation, verified.generation);
            assert_eq!(
                slot.status.last_verified_unix_millis,
                verified.last_verified_unix_millis
            );
            assert!(slot.next_attempt >= now + retry_backoff(failure));
            assert!(
                slot.status
                    .detail
                    .contains("last verified identity retained")
            );
        }
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .next_attempt = Instant::now();
        force_read(&mut resolver);
        poll_until(&mut resolver, |r| {
            r.slots.get(&1).is_some_and(|s| s.failures == 0)
        });
        let recovered = resolver.status(1).ok_or("missing status")?;
        assert_eq!(recovered.generation, verified.generation);
        assert_eq!(recovered.source_id, verified.source_id);
        assert_eq!(recovered.detail, "Trusted USB identified");
        Ok(())
    }

    #[test]
    fn failed_new_mount_verification_does_not_authorize_new_loads()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut resolver = MediaResolver::spawn(move |_, _| {
            let call = calls.fetch_add(1, Ordering::Relaxed) + 1;
            if call == 2 {
                Outcome::Failed("timeout".into())
            } else {
                verified_reply("CHRM")
            }
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        let epoch = resolver.status(1).ok_or("missing status")?.generation;
        resolver.observe(&mount(1, USBMountState::Empty), Instant::now());
        resolver.observe(&mount(1, USBMountState::Loaded), Instant::now());
        poll_until(&mut resolver, |r| {
            r.slots.get(&1).is_some_and(|s| s.failures == 1)
        });
        let status = resolver.status(1).ok_or("missing status")?;
        assert_eq!(status.state, "unavailable");
        assert!(status.generation > epoch);
        assert!(status.source_id.is_none());
        // The old load's frozen binding is separate; this mount cannot
        // authorize new loads until its own successful read.
        assert!(status.last_verified_unix_millis.is_none());
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .next_attempt = Instant::now();
        poll_until(&mut resolver, |r| {
            r.status(1)
                .is_some_and(|s| s.last_verified_unix_millis.is_some())
        });
        assert!(resolver.status(1).ok_or("missing status")?.generation > epoch);
        Ok(())
    }

    #[test]
    fn explicit_marker_change_or_conflict_revokes_identity_after_a_timeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut resolver = MediaResolver::spawn(move |_, _| {
            let call = calls.fetch_add(1, Ordering::Relaxed) + 1;
            match call {
                1 => verified_reply("CHRM"),
                2 => Outcome::Failed("timeout".into()),
                3 => Outcome::Unknown,
                4 => verified_reply("GRAY"),
                _ => Outcome::Conflict,
            }
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        let epoch = resolver.status(1).ok_or("missing status")?.generation;
        force_read(&mut resolver);
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .next_attempt = Instant::now();
        poll_until(&mut resolver, |r| {
            r.slots.get(&1).is_some_and(|s| s.failures == 1)
        });
        force_read(&mut resolver);
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .next_attempt = Instant::now();
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "unknown")
        });
        force_read(&mut resolver);
        assert!(
            resolver
                .status(1)
                .ok_or("missing status")?
                .source_id
                .is_none()
        );
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .next_attempt = Instant::now();
        poll_until(&mut resolver, |r| {
            r.status(1)
                .is_some_and(|s| s.source_name.as_deref() == Some("GRAY"))
        });
        assert!(resolver.status(1).ok_or("missing status")?.generation > epoch);
        force_read(&mut resolver);
        resolver
            .slots
            .get_mut(&1)
            .ok_or("missing slot")?
            .next_attempt = Instant::now();
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "conflict")
        });
        assert!(
            resolver
                .status(1)
                .ok_or("missing status")?
                .source_id
                .is_none()
        );
        Ok(())
    }
    #[test]
    fn load_verification_reuses_identity_until_the_physical_mount_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut resolver = MediaResolver::spawn(move |_, _| {
            let calls = calls.fetch_add(1, Ordering::Relaxed) + 1;
            let name = if calls == 1 { "GRAY" } else { "CHRM" };
            Outcome::Trusted {
                media_id: name.into(),
                source_id: format!("usb-fs:{name}"),
                name: name.into(),
                elapsed: 1,
            }
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        let first = resolver.status(1).ok_or("missing Player")?.generation;
        resolver.request_verification(1, Instant::now());
        assert!(
            resolver
                .status(1)
                .ok_or("missing Player")?
                .last_verified_unix_millis
                .is_some()
        );
        poll_until(&mut resolver, |r| {
            r.status(1)
                .is_some_and(|s| s.last_verified_unix_millis.is_some())
        });
        assert_eq!(
            resolver.status(1).ok_or("missing Player")?.generation,
            first
        );
        resolver.observe(&mount(1, USBMountState::Empty), Instant::now());
        resolver.observe(&mount(1, USBMountState::Loaded), Instant::now());
        poll_until(&mut resolver, |r| {
            r.status(1)
                .is_some_and(|s| s.source_name.as_deref() == Some("CHRM"))
        });
        assert!(resolver.status(1).ok_or("missing Player")?.generation > first);
        Ok(())
    }

    #[test]
    fn native_usb_color_is_display_only_and_cleared_on_device_loss()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = MediaResolver::disabled();
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        resolver.observe(
            &BridgeEvent::USBMedia(lumi_prolink_input::USBMedia {
                device_number: 1,
                color_id: Some(7),
            }),
            Instant::now(),
        );
        let status = resolver.status(1).ok_or("missing discovered player")?;
        assert_eq!(status.color_id, Some(7));
        assert_eq!(status.state, "resolving");
        assert!(status.source_name.is_none());
        resolver.observe(
            &BridgeEvent::DeviceLost(Device {
                device_number: 1,
                device_name: "CDJ-1500X".to_owned(),
                address: "192.168.1.1".to_owned(),
            }),
            Instant::now(),
        );
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        assert_eq!(resolver.status(1).and_then(|s| s.color_id), None);
        Ok(())
    }

    #[test]
    fn late_completion_cannot_attach_to_a_reconnected_player()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, calls) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        let mut resolver = MediaResolver::spawn(move |job, _| {
            assert!(entered.send(job.generation).is_ok());
            assert!(wait.recv().is_ok());
            Outcome::Trusted {
                media_id: "first".to_owned(),
                source_id: "usb-fs:old".to_owned(),
                name: "old".to_owned(),
                elapsed: 1,
            }
        });
        let event = found(1, "192.168.1.1");
        resolver.observe(&event, Instant::now());
        resolver.poll(Instant::now());
        assert_eq!(calls.recv_timeout(Duration::from_secs(1))?, 1);
        if let BridgeEvent::DeviceFound(device) = &event {
            resolver.observe(&BridgeEvent::DeviceLost(device.clone()), Instant::now());
        }
        resolver.observe(&event, Instant::now());
        release.send(())?;
        poll_until(&mut resolver, |r| r.inflight == Some((1, 2)));
        assert_eq!(resolver.status(1).ok_or("missing slot")?.state, "resolving");
        assert!(
            resolver
                .status(1)
                .ok_or("missing slot")?
                .source_id
                .is_none()
        );
        calls.recv_timeout(Duration::from_secs(1))?;
        release.send(())?;
        poll_until(&mut resolver, |r| {
            r.status(1).is_some_and(|s| s.state == "trusted")
        });
        Ok(())
    }
    #[test]
    fn four_slots_coalesce_repeated_discovery_and_retry_with_backoff() {
        let mut resolver = MediaResolver::spawn(|_, _| Outcome::Unknown);
        for _ in 0..1000 {
            for i in 1..=5 {
                resolver.observe(&found(i, "192.168.1.1"), Instant::now());
            }
        }
        assert_eq!(resolver.slots.len(), 4);
        poll_until(&mut resolver, |r| {
            r.slots.values().all(|s| s.status.state == "unknown")
        });
        assert!(
            resolver
                .slots
                .values()
                .all(|s| s.generation <= 4 && s.failures == 1 && s.next_attempt > Instant::now())
        );
    }
    #[test]
    fn hung_child_is_reaped_on_deadline_and_on_shutdown() {
        let job = Job {
            player: 1,
            address: "192.168.1.1".to_owned(),
            generation: 1,
            cancel: Arc::new(AtomicBool::new(false)),
        };
        let command = ReaderCommand {
            executable: PathBuf::from("/bin/sleep"),
            arguments: vec!["60".to_owned()],
        };
        let stopped = AtomicBool::new(false);
        let began = Instant::now();
        assert!(read_process(&command, &job, &stopped, Duration::from_millis(50)).is_err());
        assert!(began.elapsed() < Duration::from_secs(1));
        let mut resolver = MediaResolver::spawn(move |job, shutdown| {
            let _ = read_process(&command, job, shutdown, PROCESS_LIMIT);
            Outcome::Unknown
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        resolver.poll(Instant::now());
        thread::sleep(Duration::from_millis(30));
        let began = Instant::now();
        drop(resolver);
        assert!(began.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn address_and_response_are_validated_before_trust_lookup() {
        for address in [
            "localhost",
            "127.0.0.1",
            "8.8.8.8",
            "192.168.1.255",
            "999.1.1.1",
        ] {
            assert!(!private_player_address(address));
        }
        assert!(private_player_address("192.168.1.10"));
        assert!(matches!(
            resolve_reply(b"{invalid", "192.168.1.1", std::path::Path::new("/missing")),
            Outcome::Failed(_)
        ));
    }

    #[test]
    fn java_reply_matches_only_a_locally_registered_marker()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("lumi-resolver-contract-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let database = root.join("library.sqlite");
        let mut repository = SqliteLibraryRepository::open(&database)?;
        repository.sync_device_aliases(
            "usb-fs:legacy",
            "CHRM",
            "rev",
            &mut [],
            &[],
            &[],
            &[],
            &[],
        )?;
        let media_id = "cb28a682-9327-4fda-b38c-8fbd8eacc92e";
        let mut reply = serde_json::json!({"outcome":"marker_read", "playerAddress":"192.168.1.1",
            "mount":"/C/", "path":"/.lumi-media.json", "bytes":108, "sha256":"a".repeat(64),
            "elapsedMillis":75.1, "mediaId":media_id, "sourceId":"usb-fs:v2-current", "exactLocalMatch":false,
            "detail":"bounded reader"});
        let bytes = serde_json::to_vec(&reply)?;
        assert!(matches!(
            resolve_reply(&bytes, "192.168.1.1", &database),
            Outcome::Unknown
        ));
        repository.trust_local_usb_media(
            media_id,
            "usb-fs:v2-current",
            "usb-fs:legacy",
            "usb-fs:v2-current",
        )?;
        assert!(
            matches!(resolve_reply(&bytes, "192.168.1.1", &database), Outcome::Trusted { source_id, .. } if source_id == "usb-fs:legacy")
        );
        assert!(matches!(
            resolve_reply(&bytes, "192.168.1.2", &database),
            Outcome::Failed(_)
        ));
        reply["path"] = serde_json::json!("/other-file");
        assert!(matches!(
            resolve_reply(&serde_json::to_vec(&reply)?, "192.168.1.1", &database),
            Outcome::Failed(_)
        ));
        drop(repository);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn a_slow_media_read_does_not_block_the_pump() -> Result<(), Box<dyn std::error::Error>> {
        let (release, wait) = mpsc::sync_channel(1);
        let mut resolver = MediaResolver::spawn(move |_, _| {
            let _ = wait.recv();
            Outcome::Unknown
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        resolver.poll(Instant::now());
        let began = Instant::now();
        for _ in 0..100_000 {
            resolver.poll(Instant::now());
        }
        let elapsed = began.elapsed();
        release.send(())?;
        eprintln!(
            "100000 non-blocking media polls: {} us",
            elapsed.as_micros()
        );
        assert!(elapsed < Duration::from_secs(1));
        Ok(())
    }

    #[test]
    fn identical_markers_on_two_local_player_mounts_are_not_trusted() {
        let mut resolver = MediaResolver::spawn(|_, _| Outcome::Trusted {
            media_id: "copied-marker".to_owned(),
            source_id: "usb-fs:one".to_owned(),
            name: "same".to_owned(),
            elapsed: 1,
        });
        resolver.observe(&found(1, "192.168.1.1"), Instant::now());
        resolver.observe(&found(2, "192.168.1.2"), Instant::now());
        poll_until(&mut resolver, |r| {
            r.slots.values().all(|s| s.status.state == "conflict")
        });
        assert!(
            resolver
                .slots
                .values()
                .all(|s| s.status.source_id.is_none())
        );
    }
}
