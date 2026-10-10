//! Library I/O is isolated from the five-millisecond show pump. Only locally
//! verified USB bindings enter this bounded, read-only preparation worker.
use crate::library::{ConnectedLibraryTrack, LibraryWorker, LibraryWorkerError};
use lumi_domain::{DeckId, TrackLoadId};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct LookupKey {
    pub deck: DeckId,
    pub load: TrackLoadId,
    pub source_player: u8,
    pub source_id: String,
    pub media_generation: u64,
    pub rekordbox_id: u32,
    pub library_revision: u64,
}

pub(crate) enum Preparation {
    Ready(Box<ConnectedLibraryTrack>),
    Unknown,
    Unprepared,
    Unavailable,
    Unchanged,
}

pub(crate) struct Completion {
    pub key: LookupKey,
    pub prepared: Preparation,
    pub elapsed_micros: u64,
}

pub(crate) struct LiveLibraryResolver {
    requests: Option<SyncSender<LookupKey>>,
    results: Receiver<Completion>,
    slots: BTreeMap<DeckId, (LookupKey, Instant)>,
    inflight: Option<LookupKey>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LiveLibraryResolver {
    pub fn disabled() -> Self {
        let (_, results) = mpsc::sync_channel(4);
        Self {
            requests: None,
            results,
            slots: BTreeMap::new(),
            inflight: None,
            stopped: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn new(database: PathBuf) -> Self {
        let mut reader: Option<LibraryWorker> = None;
        let mut revision = None;
        let mut cache = BTreeMap::<DeckId, (LookupKey, u64)>::new();
        Self::spawn(move |key| {
            let result = (|| -> Result<Preparation, LibraryWorkerError> {
                if revision != Some(key.library_revision) || reader.is_none() {
                    reader = Some(LibraryWorker::live_reader(&database)?);
                    revision = Some(key.library_revision);
                    cache.clear();
                }
                let worker = reader
                    .as_ref()
                    .ok_or(LibraryWorkerError::MissingLibrarySource)?;
                let version = worker.data_version()?;
                if cache
                    .get(&key.deck)
                    .is_some_and(|(previous, v)| previous == key && *v == version)
                {
                    return Ok(Preparation::Unchanged);
                }
                // Even after a concurrent commit, cache only the version seen
                // before reading. A later poll must revalidate newer content.
                let prepared =
                    match worker.connected_track_for_source(&key.source_id, key.rekordbox_id) {
                        Ok(Some(track)) => Preparation::Ready(Box::new(track)),
                        Ok(None) => Preparation::Unknown,
                        Err(LibraryWorkerError::MissingTimeline) => Preparation::Unprepared,
                        Err(error) => return Err(error),
                    };
                cache.insert(key.deck, (key.clone(), version));
                Ok(prepared)
            })();
            result.unwrap_or(Preparation::Unavailable)
        })
    }

    fn spawn<F: FnMut(&LookupKey) -> Preparation + Send + 'static>(mut prepare: F) -> Self {
        let (requests, receiver) = mpsc::sync_channel::<LookupKey>(1);
        let (sender, results) = mpsc::sync_channel(4);
        let stopped = Arc::new(AtomicBool::new(false));
        let shutdown = stopped.clone();
        let worker = thread::Builder::new()
            .name("lumi-live-library".into())
            .spawn(move || {
                while !shutdown.load(Ordering::Acquire) {
                    let key = match receiver.recv_timeout(Duration::from_millis(50)) {
                        Ok(key) => key,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let started = Instant::now();
                    // A failed parser/reader must not silently strand a pending
                    // Player or unwind into the independent show lanes.
                    let prepared =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prepare(&key)))
                            .unwrap_or(Preparation::Unavailable);
                    let elapsed_micros =
                        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                    let _ = sender.try_send(Completion {
                        key,
                        prepared,
                        elapsed_micros,
                    });
                }
            })
            .ok();
        Self {
            requests: worker.as_ref().map(|_| requests),
            results,
            slots: BTreeMap::new(),
            inflight: None,
            stopped,
            worker,
        }
    }

    /// At most six keys, four replies, and one non-blocking send per pump tick.
    pub fn poll(&mut self, keys: Vec<LookupKey>, now: Instant) -> Vec<Completion> {
        self.slots
            .retain(|deck, _| keys.iter().any(|key| key.deck == *deck));
        for key in keys.into_iter().take(6) {
            let slot = self
                .slots
                .entry(key.deck)
                .or_insert_with(|| (key.clone(), now));
            if slot.0 != key {
                *slot = (key, now);
            }
        }
        let mut accepted = Vec::new();
        for _ in 0..4 {
            let result = match self.results.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.requests = None;
                    self.inflight = None;
                    break;
                }
            };
            if self.inflight.as_ref() == Some(&result.key) {
                self.inflight = None;
            }
            if self
                .slots
                .get(&result.key.deck)
                .is_some_and(|slot| slot.0 == result.key)
            {
                accepted.push(result);
            }
        }
        if self.requests.is_none() {
            for slot in self.slots.values_mut().filter(|slot| slot.1 <= now) {
                accepted.push(Completion {
                    key: slot.0.clone(),
                    prepared: Preparation::Unavailable,
                    elapsed_micros: 0,
                });
                slot.1 = now + Duration::from_secs(1);
            }
        }
        if self.inflight.is_none()
            && let Some(slot) = self
                .slots
                .values_mut()
                .filter(|slot| slot.1 <= now)
                .min_by_key(|slot| slot.1)
            && let Some(sender) = self.requests.as_ref()
            && sender.try_send(slot.0.clone()).is_ok()
        {
            self.inflight = Some(slot.0.clone());
            slot.1 = now + Duration::from_secs(1);
        }
        accepted
    }
}

impl Drop for LiveLibraryResolver {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(deck: u8, load: u64, source: &str) -> LookupKey {
        LookupKey {
            deck: DeckId::new(deck),
            load: TrackLoadId::new(load),
            source_player: 1,
            source_id: source.into(),
            media_generation: 1,
            rekordbox_id: 42,
            library_revision: 1,
        }
    }
    fn wait(
        resolver: &mut LiveLibraryResolver,
        keys: Vec<LookupKey>,
    ) -> Result<Completion, &'static str> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Some(result) = resolver
                .poll(keys.clone(), Instant::now())
                .into_iter()
                .next()
            {
                return Ok(result);
            }
            thread::sleep(Duration::from_millis(1));
        }
        Err("preparation did not complete")
    }
    #[test]
    fn slow_read_never_blocks_polls_and_replaced_load_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = LiveLibraryResolver::spawn(|_| {
            thread::sleep(Duration::from_millis(80));
            Preparation::Unknown
        });
        let old = key(2, 10, "GRAY");
        resolver.poll(vec![old.clone()], Instant::now());
        let mut current = key(2, 11, "CHRM");
        current.media_generation = 2;
        let mut max_micros = 0;
        for _ in 0..50 {
            let started = Instant::now();
            let replies = resolver.poll(vec![current.clone()], started);
            max_micros = max_micros.max(started.elapsed().as_micros());
            assert!(replies.iter().all(|result| result.key != old));
            thread::sleep(Duration::from_millis(1));
        }
        assert!(max_micros < 20_000, "pump blocked: {max_micros} us");
        assert_eq!(wait(&mut resolver, vec![current.clone()])?.key, current);
        eprintln!("80ms injected Library read: maximum local poll cost {max_micros} us");
        Ok(())
    }
    #[test]
    fn failed_reader_does_not_strand_other_players() -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = LiveLibraryResolver::spawn(|received| {
            if received.deck == DeckId::new(1) {
                panic!("injected reader fault");
            }
            Preparation::Unprepared
        });
        let keys = vec![key(1, 10, "GRAY"), key(2, 11, "CHRM")];
        assert!(matches!(
            wait(&mut resolver, keys.clone())?.prepared,
            Preparation::Unavailable
        ));
        let next = wait(&mut resolver, keys)?;
        assert_eq!(next.key.deck, DeckId::new(2));
        assert!(matches!(next.prepared, Preparation::Unprepared));
        Ok(())
    }
    #[test]
    fn missing_worker_reports_failure_instead_of_waiting_forever() {
        let mut resolver = LiveLibraryResolver::disabled();
        let now = Instant::now();
        let replies = resolver.poll(vec![key(1, 10, "GRAY")], now);
        assert_eq!(replies.len(), 1);
        assert!(matches!(replies[0].prepared, Preparation::Unavailable));
        assert!(resolver.poll(vec![key(1, 10, "GRAY")], now).is_empty());
    }
    #[test]
    fn waiting_library_read_has_a_measured_bounded_poll_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let (release, blocked) = mpsc::sync_channel(1);
        let mut resolver = LiveLibraryResolver::spawn(move |_| {
            let _ = blocked.recv();
            Preparation::Unknown
        });
        let keys = vec![key(1, 10, "GRAY"), key(2, 11, "CHRM")];
        let mut costs = Vec::with_capacity(10_000);
        let began = Instant::now();
        for _ in 0..10_000 {
            let started = Instant::now();
            let _ = resolver.poll(keys.clone(), started);
            costs.push(started.elapsed().as_nanos());
        }
        let total = began.elapsed();
        costs.sort_unstable();
        eprintln!(
            "Library poll (10,000 ticks, blocked reader): p95={}ns p99={}ns max={}ns total={}us",
            costs[9_499],
            costs[9_899],
            costs[9_999],
            total.as_micros()
        );
        release.send(())?;
        drop(resolver);
        assert!(
            costs[9_899] < 500_000,
            "p99 Library poll exceeded 0.5ms component budget"
        );
        assert!(
            total < Duration::from_secs(1),
            "Library polling monopolized the pump"
        );
        Ok(())
    }

    #[test]
    fn cross_player_keeps_physical_source_and_unload_discards_reply()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = key(2, 10, "GRAY");
        let mut resolver = LiveLibraryResolver::spawn(|received| {
            assert_eq!(received.deck, DeckId::new(2));
            assert_eq!(received.source_player, 1);
            assert_eq!(received.source_id, "GRAY");
            Preparation::Unprepared
        });
        assert!(matches!(
            wait(&mut resolver, vec![expected.clone()])?.prepared,
            Preparation::Unprepared
        ));
        resolver.poll(vec![expected], Instant::now() + Duration::from_secs(2));
        thread::sleep(Duration::from_millis(10));
        assert!(resolver.poll(Vec::new(), Instant::now()).is_empty());
        Ok(())
    }
    #[test]
    fn a_real_synced_track_without_phrases_is_a_local_preparation_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        use lumi_library::{LibraryRepository, TrackPageRequest};
        use lumi_library_demo::DemoLibrarySourceProvider;
        use lumi_library_source::MusicLibrarySourceProvider;
        use lumi_library_sqlite::{DeviceAliasUpsert, SqliteLibraryRepository};
        let root =
            std::env::temp_dir().join(format!("lumi-unprepared-reader-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let path = root.join("library.sqlite");
        let mut writer = SqliteLibraryRepository::open(&path)?;
        writer.import_baseline(&DemoLibrarySourceProvider::curated().load_baseline()?)?;
        let track = writer
            .page_tracks(TrackPageRequest::try_new(0, 1)?)?
            .tracks()[0]
            .id();
        assert!(writer.timeline_head(track)?.is_none());
        writer.sync_device_aliases(
            "usb-fs:test",
            "Test",
            "v1",
            &mut [DeviceAliasUpsert {
                device_track_id: 42,
                simulator_signature: 0,
                audio_signature: "audio-full-v1:fixture".into(),
                canonical_track_id: Some(track),
                match_kind: "fixture".into(),
                title: "Test".into(),
                artist: "Test".into(),
                bpm_milli: 140_000,
                duration_millis: 100_000,
                file_size: 10,
                audio_uri: "file://localhost/missing-test.mp3".into(),
                metadata_revision: "v1".into(),
                color_rgb: None,
                master_database_id: 1,
                master_content_id: 1,
                information_update_count: 1,
                analysis_revision: "v1".into(),
                analyzed_at: "2026-10-05".into(),
                sync_disposition: "current".into(),
            }],
            &[],
            &[],
            &[],
            &[],
        )?;
        let mut resolver = LiveLibraryResolver::new(path);
        assert!(matches!(
            wait(&mut resolver, vec![key(1, 10, "usb-fs:test")])?.prepared,
            Preparation::Unprepared
        ));
        assert!(
            writer.timeline_head(track)?.is_none(),
            "read-only preparation fabricated phrases"
        );
        drop(resolver);
        drop(writer);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn real_committed_sync_is_seen_without_a_new_load() -> Result<(), Box<dyn std::error::Error>> {
        use lumi_library::{LibraryRepository, TrackPageRequest};
        use lumi_library_sqlite::{DeviceAliasUpsert, SqliteLibraryRepository};
        let root = std::env::temp_dir().join(format!(
            "lumi-live-reader-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir_all(&root)?;
        let path = root.join("library.sqlite");
        let initial = LibraryWorker::demo_at(&path)?;
        let mut writer = SqliteLibraryRepository::open(&path)?;
        let track = writer
            .page_tracks(TrackPageRequest::try_new(0, 1)?)?
            .tracks()[0]
            .id();
        let mut resolver = LiveLibraryResolver::new(path.clone());
        let loaded = key(1, 10, "usb-fs:test");
        assert!(matches!(
            wait(&mut resolver, vec![loaded.clone()])?.prepared,
            Preparation::Unknown
        ));
        writer.sync_device_aliases(
            "usb-fs:test",
            "Test",
            "v1",
            &mut [DeviceAliasUpsert {
                device_track_id: 42,
                simulator_signature: 0,
                audio_signature: "audio-full-v1:fixture".into(),
                canonical_track_id: Some(track),
                match_kind: "fixture".into(),
                title: "Test".into(),
                artist: "Test".into(),
                bpm_milli: 140_000,
                duration_millis: 100_000,
                file_size: 10,
                audio_uri: "file://localhost/missing-test.mp3".into(),
                metadata_revision: "v1".into(),
                color_rgb: None,
                master_database_id: 1,
                master_content_id: 1,
                information_update_count: 1,
                analysis_revision: "v1".into(),
                analyzed_at: "2026-10-05".into(),
                sync_disposition: "current".into(),
            }],
            &[],
            &[],
            &[],
            &[],
        )?;
        let reply = wait(&mut resolver, vec![loaded.clone()])?;
        assert_eq!(reply.key.load, loaded.load);
        assert!(matches!(reply.prepared, Preparation::Ready(_)));
        drop(resolver);
        drop(writer);
        drop(initial);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
