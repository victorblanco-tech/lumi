//! Channel-local timing persistence. File I/O never runs in the integration pump.

use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredTiming {
    version: u8,
    millis: i16,
}

pub(crate) struct TimingPreferences {
    sender: Option<SyncSender<i16>>,
    results: Receiver<Result<i16, String>>,
    pub saved: Option<i16>,
    pub pending_writes: usize,
    pub error: Option<String>,
}

impl TimingPreferences {
    pub fn open(path: Option<PathBuf>) -> io::Result<Self> {
        let (result_tx, results) = mpsc::channel();
        let Some(path) = path else {
            return Ok(Self {
                sender: None,
                results,
                saved: None,
                pending_writes: 0,
                error: None,
            });
        };
        // Startup only: malformed storage is surfaced, never silently overwritten.
        let (saved, error) = match read(&path) {
            Ok(saved) => (saved, None),
            Err(error) => (None, Some(error.to_string())),
        };
        let (sender, receiver) = mpsc::sync_channel(16);
        std::thread::Builder::new()
            .name("lumi-timing-settings".into())
            .spawn(move || {
                for millis in receiver {
                    let result = write(&path, millis)
                        .map(|()| millis)
                        .map_err(|error| error.to_string());
                    if result_tx.send(result).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            results,
            saved,
            pending_writes: 0,
            error,
        })
    }

    pub fn request(&mut self, millis: i16) -> Result<(), String> {
        if !(-250..=250).contains(&millis) {
            return Err("Timing must be within -250...250 ms".into());
        }
        self.poll();
        if self.pending_writes == 0 && self.saved == Some(millis) && self.error.is_none() {
            return Ok(());
        }
        if let Some(sender) = &self.sender {
            sender
                .try_send(millis)
                .map_err(|_| "Timing settings writer is busy or unavailable".to_owned())?;
            self.pending_writes += 1;
        } else {
            // Headless unit fixtures intentionally have no persistent channel.
            self.saved = Some(millis);
        }
        self.error = None;
        Ok(())
    }

    pub fn poll(&mut self) {
        while let Ok(result) = self.results.try_recv() {
            self.pending_writes = self.pending_writes.saturating_sub(1);
            match result {
                Ok(millis) => {
                    self.saved = Some(millis);
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
    }
}

fn read(path: &Path) -> io::Result<Option<i16>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    let stored: StoredTiming = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if bytes.len() > 1024 || stored.version != 1 || !(-250..=250).contains(&stored.millis) {
        return Err(io::Error::other("Invalid saved lighting timing"));
    }
    Ok(Some(stored.millis))
}

fn write(path: &Path, millis: i16) -> io::Result<()> {
    static NEXT_WRITE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_WRITE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = path.with_extension(format!("{}.{sequence}.pending", std::process::id()));
    let bytes =
        serde_json::to_vec(&StoredTiming { version: 1, millis }).map_err(io::Error::other)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct TempDirectory(PathBuf);
    impl TempDirectory {
        fn new() -> io::Result<Self> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("lumi-timing-{}-{id}", std::process::id()));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn finish(store: &mut TimingPreferences) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while store.pending_writes > 0 && Instant::now() < deadline {
            store.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(store.pending_writes, 0);
    }

    #[test]
    fn ordered_updates_survive_reopen_and_channels_remain_separate() -> io::Result<()> {
        let directory = TempDirectory::new()?;
        let path = directory.path().join("timing.json");
        let mut store = TimingPreferences::open(Some(path.clone()))?;
        assert_eq!(store.saved, None);
        for millis in [-250, 250, -125] {
            store.request(millis).map_err(io::Error::other)?;
        }
        finish(&mut store);
        assert_eq!(store.saved, Some(-125));
        assert_eq!(store.error, None);
        drop(store);
        assert_eq!(TimingPreferences::open(Some(path))?.saved, Some(-125));
        assert_eq!(
            TimingPreferences::open(Some(directory.path().join("prod.json")))?.saved,
            None
        );
        Ok(())
    }

    #[test]
    fn failed_write_is_not_reported_saved_and_can_be_retried() -> io::Result<()> {
        let directory = TempDirectory::new()?;
        let parent = directory.path().join("missing");
        let mut store = TimingPreferences::open(Some(parent.join("timing.json")))?;
        store.request(-200).map_err(io::Error::other)?;
        finish(&mut store);
        assert_eq!(store.saved, None);
        assert!(store.error.is_some());
        fs::create_dir(&parent)?;
        store.request(-200).map_err(io::Error::other)?;
        finish(&mut store);
        assert_eq!(store.saved, Some(-200));
        assert!(store.error.is_none());
        assert!(store.request(251).is_err());
        Ok(())
    }

    #[test]
    fn invalid_persisted_value_is_not_silently_reset() -> io::Result<()> {
        let directory = TempDirectory::new()?;
        let path = directory.path().join("timing.json");
        fs::write(&path, br#"{"version":1,"millis":999}"#)?;
        let store = TimingPreferences::open(Some(path.clone()))?;
        assert_eq!(store.saved, None);
        assert!(store.error.is_some());
        assert_eq!(fs::read_to_string(path)?, r#"{"version":1,"millis":999}"#);
        Ok(())
    }
}
