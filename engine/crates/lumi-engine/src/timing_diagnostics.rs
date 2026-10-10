//! Bounded, best-effort local flight recorder. Never owns a control connection.
//! Disk I/O and serialization run on a separate thread; saturation drops
//! diagnostics, never transport. The next admitted record reports that loss.
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::mpsc::{self, SyncSender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const QUEUE_LIMIT: usize = 512;
const HISTORY_LIMIT: usize = 8192;

#[derive(Default)]
pub(crate) struct TimingDiagnostics {
    sender: Option<SyncSender<Value>>,
    origin: Option<Instant>,
    dropped: u64,
}

impl TimingDiagnostics {
    pub fn start(path: PathBuf) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_LIMIT);
        std::thread::Builder::new()
            .name("lumi-timing-diagnostics".into())
            .spawn(move || {
                let mut history = VecDeque::with_capacity(HISTORY_LIMIT);
                let mut last_write = Instant::now();
                let mut revision = 0_u64;
                loop {
                    let disconnected = match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(value) => {
                            if history.len() == HISTORY_LIMIT {
                                history.pop_front();
                            }
                            history.push_back(value);
                            false
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => false,
                        Err(mpsc::RecvTimeoutError::Disconnected) => true,
                    };
                    if (disconnected || last_write.elapsed() >= Duration::from_secs(1))
                        && !history.is_empty()
                    {
                        revision = revision.saturating_add(1);
                        // A failed export must not fail the live engine. Atomic replacement
                        // also prevents readers from observing half-written JSON.
                        let _ = write_history(&path, revision, &history);
                        last_write = Instant::now();
                    }
                    if disconnected {
                        break;
                    }
                }
            })?;
        let mut diagnostics = Self {
            sender: Some(sender),
            origin: Some(Instant::now()),
            dropped: 0,
        };
        diagnostics.record(json!({"stage":"session", "processId":std::process::id(),
            "version":env!("CARGO_PKG_VERSION"), "unixMillis":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64}));
        Ok(diagnostics)
    }

    pub fn record(&mut self, mut value: Value) {
        let (Some(sender), Some(origin)) = (&self.sender, self.origin) else {
            return;
        };
        value["engineMicros"] = json!(origin.elapsed().as_micros() as u64);
        value["droppedRecords"] = json!(self.dropped);
        if sender.try_send(value).is_err() {
            self.dropped = self.dropped.saturating_add(1);
        }
    }
}

fn write_history(
    path: &std::path::Path,
    revision: u64,
    history: &VecDeque<Value>,
) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("{}.{revision}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        serde_json::to_writer(
            &mut file,
            &json!({"schemaVersion":1,"processId":std::process::id(),"version":env!("CARGO_PKG_VERSION"),"capacity":HISTORY_LIMIT,"events":history}),
        )?;
        file.flush()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saturated_recorder_never_waits_and_reports_loss() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut recorder = TimingDiagnostics {
            sender: Some(sender),
            origin: Some(Instant::now()),
            dropped: 0,
        };
        recorder.record(json!({"stage":"first"}));
        recorder.record(json!({"stage":"dropped"}));
        assert_eq!(recorder.dropped, 1);
        let _ = receiver.recv().unwrap_or_else(|e| panic!("receive: {e}"));
        recorder.record(json!({"stage":"next"}));
        assert_eq!(
            receiver.recv().unwrap_or_else(|e| panic!("receive: {e}"))["droppedRecords"],
            1
        );
    }

    #[test]
    fn export_is_private_atomic_and_readable_without_control_connection() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!(
            "lumi-trace-test-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let history = VecDeque::from([
            json!({"stage":"schedule","generation":7}),
            json!({"stage":"midiDispatch","generation":7}),
        ]);
        write_history(&path, 1, &history).unwrap_or_else(|e| panic!("write trace: {e}"));
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read trace: {e}"));
        let value: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("decode trace: {e}"));
        assert_eq!(value["events"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            fs::metadata(&path)
                .unwrap_or_else(|e| panic!("metadata: {e}"))
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_file(path).unwrap_or_else(|e| panic!("cleanup trace: {e}"));
    }
}
