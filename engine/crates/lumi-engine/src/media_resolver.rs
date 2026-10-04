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
use lumi_prolink_input::{BridgeEvent, SourceCondition};
use serde::{Deserialize, Serialize};

const RECHECK: Duration = Duration::from_secs(15);
const PROCESS_LIMIT: Duration = Duration::from_secs(8);
const MAX_OUTPUT: u64 = 16_384;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaStatus {
    pub state: &'static str,
    pub source_id: Option<String>,
    pub source_name: Option<String>,
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
    Failed(&'static str),
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
                            generation,
                            last_verified_unix_millis: None,
                            elapsed_millis: None,
                            detail: "Reading USB identity independently of show output".to_owned(),
                        },
                        media_id: None,
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
                    ("trusted", "Trusted USB identified")
                }
                Outcome::Unknown => (
                    "unknown",
                    "Connect this USB to the Mac and scan it in Import & Sources",
                ),
                Outcome::Conflict => (
                    "conflict",
                    "USB identity conflicts with its local registration; review the source on the Mac",
                ),
                Outcome::Failed(detail) => ("unavailable", detail),
            };
            slot.status.state = state;
            slot.status.detail = detail.to_owned();
            if state != "trusted" {
                slot.status.last_verified_unix_millis = None;
                slot.failures = slot.failures.saturating_add(1);
            }
            let backoff = if state == "trusted" {
                RECHECK
            } else {
                Duration::from_secs(2_u64.saturating_pow(slot.failures.min(5)).min(30))
            };
            slot.next_attempt = now + backoff;
            if let Some(other) = duplicate {
                for player in [result.player, other] {
                    if let Some(slot) = self.slots.get_mut(&player) {
                        slot.status.state = "conflict";
                        slot.status.source_id = None;
                        slot.status.source_name = None;
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
            .filter(|(_, slot)| slot.next_attempt <= now)
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
) -> Result<Vec<u8>, &'static str> {
    if !private_player_address(&job.address) {
        return Err("Only a discovered private IPv4 Player address is allowed");
    }
    if job.cancel.load(Ordering::Acquire) || shutdown.load(Ordering::Acquire) {
        return Err("USB identity request cancelled");
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
        return Err("USB identity reader has no response channel");
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
            break Err("USB identity request cancelled");
        }
        if Instant::now() >= deadline {
            break Err("USB identity reader timed out; show output continues");
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err("USB identity unavailable; check the Player's USB and local registration")
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => break Err("USB identity reader stopped unexpectedly"),
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
    outcome?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("USB identity response exceeded its size limit");
    }
    Ok(bytes)
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
        return Outcome::Failed("Invalid USB identity response");
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
        return Outcome::Failed("Invalid USB identity response");
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
        Err(_) => {
            Outcome::Failed("Local USB registration could not be read; retrying independently")
        }
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
