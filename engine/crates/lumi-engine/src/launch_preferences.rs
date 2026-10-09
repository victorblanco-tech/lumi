//! Channel-local launch setting; bounded writes stay outside the timing pump.
use crate::launch_policy::LaunchPolicy;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u8,
    policy: LaunchPolicy,
}

pub(crate) struct LaunchPreferences {
    sender: Option<mpsc::SyncSender<LaunchPolicy>>,
    results: mpsc::Receiver<Result<LaunchPolicy, String>>,
    pub saved: Option<LaunchPolicy>,
    pub pending: usize,
    pub error: Option<String>,
}

impl LaunchPreferences {
    pub fn open(path: Option<PathBuf>) -> io::Result<Self> {
        let (tx, results) = mpsc::channel();
        let Some(path) = path else {
            return Ok(Self {
                sender: None,
                results,
                saved: None,
                pending: 0,
                error: None,
            });
        };
        let (saved, error) = match read(&path) {
            Ok(value) => (value, None),
            Err(error) => (None, Some(error.to_string())),
        };
        let (sender, requests) = mpsc::sync_channel(16);
        std::thread::Builder::new()
            .name("lumi-launch-settings".into())
            .spawn(move || {
                for policy in requests {
                    let result = write(&path, policy)
                        .map(|()| policy)
                        .map_err(|e| e.to_string());
                    if tx.send(result).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            results,
            saved,
            pending: 0,
            error,
        })
    }
    pub fn request(&mut self, policy: LaunchPolicy) -> Result<(), String> {
        self.poll();
        if self.pending == 0 && self.saved == Some(policy) && self.error.is_none() {
            return Ok(());
        }
        if let Some(sender) = &self.sender {
            sender
                .try_send(policy)
                .map_err(|_| "Launch settings writer is busy or unavailable")?;
            self.pending += 1;
        } else {
            self.saved = Some(policy);
        }
        self.error = None;
        Ok(())
    }
    pub fn poll(&mut self) {
        while let Ok(result) = self.results.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            match result {
                Ok(value) => {
                    self.saved = Some(value);
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
    }
}

fn read(path: &Path) -> io::Result<Option<LaunchPolicy>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 {
        return Err(io::Error::other("Invalid saved launch policy"));
    }
    let stored: Stored = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if stored.version != 1 {
        return Err(io::Error::other("Unsupported saved launch policy version"));
    }
    Ok(Some(stored.policy))
}

fn write(path: &Path, policy: LaunchPolicy) -> io::Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = path.with_extension(format!("{}.{sequence}.pending", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(
            &serde_json::to_vec(&Stored { version: 1, policy }).map_err(io::Error::other)?,
        )?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_rejects_unknown_and_preserves_invalid_file() -> io::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "lumi-launch-policy-test-{}.json",
            std::process::id()
        ));
        write(&path, LaunchPolicy::OnPhraseStart)?;
        assert_eq!(read(&path)?, Some(LaunchPolicy::OnPhraseStart));
        fs::write(&path, br#"{"version":2,"policy":"immediate"}"#)?;
        let store = LaunchPreferences::open(Some(path.clone()))?;
        assert!(store.error.is_some());
        assert_eq!(fs::read(&path)?, br#"{"version":2,"policy":"immediate"}"#);
        fs::remove_file(path)?;
        Ok(())
    }
}
