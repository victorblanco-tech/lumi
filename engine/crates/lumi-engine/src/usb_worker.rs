//! Isolated, one-shot USB library worker.
//!
//! Removable-media I/O must never block the channel-persistent realtime
//! engine. The macOS app launches this mode as a short-lived child process,
//! applies a hard deadline, and only accepts a complete JSON snapshot.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::library::{DeviceReviewChoice, LibraryWorker, LibraryWorkerError};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum UsbWorkerRequest {
    Inspect {
        root: String,
        #[serde(default)]
        #[serde(rename = "sourceId")]
        source_id: Option<String>,
    },
    Sync {
        root: String,
        #[serde(default)]
        #[serde(rename = "sourceId")]
        source_id: Option<String>,
        #[serde(rename = "playlistIds")]
        playlist_ids: Vec<u32>,
    },
    ResolveConflict {
        root: String,
        #[serde(rename = "sourceId")]
        source_id: String,
        #[serde(rename = "deviceTrackId")]
        device_track_id: u32,
        #[serde(rename = "expectedIncomingRevision")]
        expected_incoming_revision: String,
        #[serde(rename = "expectedActiveRevision")]
        expected_active_revision: String,
        choice: String,
    },
}

pub fn run_usb_worker(request_path: &Path, response_path: &Path) -> Result<(), UsbWorkerError> {
    let request: UsbWorkerRequest = serde_json::from_slice(&fs::read(request_path)?)?;
    let mut worker = LibraryWorker::demo()?;
    match request {
        UsbWorkerRequest::Inspect { root, source_id } => {
            let source_id = ensure_source_marker(Path::new(&root), source_id.as_deref())?;
            worker.inspect_rekordbox_device(root, Some(&source_id))?;
        }
        UsbWorkerRequest::Sync {
            root,
            source_id,
            playlist_ids,
        } => {
            let source_id = ensure_source_marker(Path::new(&root), source_id.as_deref())?;
            worker.sync_rekordbox_device(root, Some(&source_id), &playlist_ids)?;
        }
        UsbWorkerRequest::ResolveConflict {
            root,
            source_id,
            device_track_id,
            expected_incoming_revision,
            expected_active_revision,
            choice,
        } => {
            let source_id = ensure_source_marker(Path::new(&root), Some(&source_id))?;
            let choice = match choice.as_str() {
                "keep-lumi" => DeviceReviewChoice::KeepLumi,
                "use-usb" => DeviceReviewChoice::UseUsb,
                _ => return Err(UsbWorkerError::InvalidReviewChoice(choice)),
            };
            worker.resolve_rekordbox_device_conflict(
                root,
                &source_id,
                device_track_id,
                &expected_incoming_revision,
                &expected_active_revision,
                choice,
            )?;
        }
    }
    let response = json!({ "library": worker.snapshot_json()? });
    let temporary = response_path.with_extension("json.partial");
    fs::write(&temporary, serde_json::to_vec(&response)?)?;
    fs::rename(temporary, response_path)?;
    Ok(())
}

const SOURCE_MARKER_FILE: &str = ".lumi-source.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceMarker {
    format_version: u8,
    source_id: String,
    created_at: String,
}

/// Marker access is deliberately inside the disposable USB worker. Even a
/// tiny FAT `open(2)` can stall indefinitely on unhealthy removable media;
/// no marker read or write may therefore happen on SwiftUI's main thread or
/// inside the channel-persistent realtime engine.
fn ensure_source_marker(root: &Path, preferred: Option<&str>) -> Result<String, UsbWorkerError> {
    if !root.is_absolute() || !root.join("PIONEER/rekordbox/exportLibrary.db").is_file() {
        return Err(UsbWorkerError::InvalidDeviceRoot(
            root.display().to_string(),
        ));
    }
    let marker_path = root.join(SOURCE_MARKER_FILE);
    match fs::read(&marker_path) {
        Ok(data) => {
            if data.len() <= 4_096
                && let Ok(marker) = serde_json::from_slice::<SourceMarker>(&data)
                && marker.format_version == 1
                && valid_source_id(&marker.source_id)
            {
                return Ok(marker.source_id);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let source_id = preferred
        .filter(|value| valid_source_id(value))
        .map(str::to_owned)
        .unwrap_or_else(generated_source_id);
    let marker = SourceMarker {
        format_version: 1,
        source_id: source_id.clone(),
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_owned()),
    };
    let temporary = root.join(format!(
        "{SOURCE_MARKER_FILE}.{}.partial",
        std::process::id()
    ));
    fs::write(&temporary, serde_json::to_vec(&marker)?)?;
    if let Err(error) = fs::rename(&temporary, &marker_path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(source_id)
}

fn valid_source_id(value: &str) -> bool {
    (8..=200).contains(&value.len())
        && value.starts_with("usb-")
        && !value.contains('/')
        && !value.contains('\\')
}

fn generated_source_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let digest = Sha256::digest(format!("{}:{now}:{counter}", std::process::id()).as_bytes());
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("usb-marker:{suffix}")
}

#[derive(Debug, Error)]
pub enum UsbWorkerError {
    #[error("USB worker file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("USB worker request is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("USB library operation failed: {0}")]
    Library(#[from] LibraryWorkerError),
    #[error("USB review choice is invalid: {0}")]
    InvalidReviewChoice(String),
    #[error("USB device root is invalid or has no OneLibrary database: {0}")]
    InvalidDeviceRoot(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn request_contract_uses_bounded_explicit_operations() -> Result<(), serde_json::Error> {
        let request: UsbWorkerRequest = serde_json::from_value(json!({
            "kind": "sync",
            "root": "/Volumes/USB",
            "sourceId": "usb-marker:one",
            "playlistIds": [7, 9]
        }))?;
        assert!(matches!(
            request,
            UsbWorkerRequest::Sync { playlist_ids, .. } if playlist_ids == vec![7, 9]
        ));
        Ok(())
    }

    #[test]
    fn unknown_review_choice_is_rejected_before_mutation() {
        let error = UsbWorkerError::InvalidReviewChoice("surprise".to_owned());
        assert!(error.to_string().contains("surprise"));
    }

    #[test]
    fn marker_preserves_preferred_identity_and_is_idempotent() -> Result<(), UsbWorkerError> {
        let root = device_root("preserve")?;
        let preferred = "usb-fs:hardware-existing-source";
        assert_eq!(ensure_source_marker(&root, Some(preferred))?, preferred);
        assert_eq!(
            ensure_source_marker(&root, Some("usb-fs:ignored-later"))?,
            preferred
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn equal_model_media_without_a_preference_receive_distinct_markers()
    -> Result<(), UsbWorkerError> {
        let first = device_root("first")?;
        let second = device_root("second")?;
        let first_id = ensure_source_marker(&first, None)?;
        let second_id = ensure_source_marker(&second, None)?;
        assert!(first_id.starts_with("usb-marker:"));
        assert!(second_id.starts_with("usb-marker:"));
        assert_ne!(first_id, second_id);
        fs::remove_dir_all(first)?;
        fs::remove_dir_all(second)?;
        Ok(())
    }

    #[test]
    fn invalid_marker_is_replaced_only_beside_a_valid_onelibrary() -> Result<(), UsbWorkerError> {
        let root = device_root("repair")?;
        fs::write(root.join(SOURCE_MARKER_FILE), b"not-json")?;
        let repaired = ensure_source_marker(&root, Some("usb-fs:hardware-repaired"))?;
        assert_eq!(repaired, "usb-fs:hardware-repaired");

        let invalid_root = std::env::temp_dir().join(format!(
            "lumi-usb-worker-no-library-{}",
            generated_source_id().replace(':', "-")
        ));
        fs::create_dir_all(&invalid_root)?;
        assert!(matches!(
            ensure_source_marker(&invalid_root, None),
            Err(UsbWorkerError::InvalidDeviceRoot(_))
        ));
        assert!(!invalid_root.join(SOURCE_MARKER_FILE).exists());
        fs::remove_dir_all(root)?;
        fs::remove_dir_all(invalid_root)?;
        Ok(())
    }

    fn device_root(label: &str) -> Result<PathBuf, UsbWorkerError> {
        let root = std::env::temp_dir().join(format!(
            "lumi-usb-worker-{label}-{}",
            generated_source_id().replace(':', "-")
        ));
        let library = root.join("PIONEER/rekordbox");
        fs::create_dir_all(&library)?;
        fs::write(library.join("exportLibrary.db"), b"fixture")?;
        Ok(root)
    }
}
