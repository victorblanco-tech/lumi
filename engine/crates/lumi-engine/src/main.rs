//! Lumi engine executable entry point.

#![forbid(unsafe_code)]

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.get(1).map(String::as_str) == Some("--usb-worker") {
        let Some(request_path) = arguments.get(2) else {
            eprintln!("lumi-engine USB worker requires a request path");
            return ExitCode::FAILURE;
        };
        let Some(response_path) = arguments.get(3) else {
            eprintln!("lumi-engine USB worker requires a response path");
            return ExitCode::FAILURE;
        };
        return match lumi_engine::run_usb_worker(Path::new(request_path), Path::new(response_path))
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("lumi-engine USB worker failed: {error}");
                ExitCode::FAILURE
            }
        };
    }
    match lumi_engine::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            record_fatal_error(&error.to_string());
            eprintln!("lumi-engine failed: {error}");
            ExitCode::FAILURE
        }
    }
}

/// A launchd-owned helper has no terminal. Preserve the final error beside the
/// channel database so a service restart never erases the only useful cause.
fn record_fatal_error(error: &str) {
    let Some(path) = fatal_log_path() else { return };
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
    else {
        return;
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let _ = writeln!(file, "{timestamp} {error}");
}

fn fatal_log_path() -> Option<PathBuf> {
    if std::env::var("LUMI_SERVICE_MODE").as_deref() != Ok("launchd") {
        return None;
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let directory = std::env::var("LUMI_DATA_DIRECTORY_NAME").ok()?;
    if directory.is_empty()
        || directory == "."
        || directory == ".."
        || directory.contains('/')
        || directory.contains('\0')
    {
        return None;
    }
    Some(
        home.join("Library")
            .join("Application Support")
            .join(directory)
            .join("engine-fatal.log"),
    )
}
