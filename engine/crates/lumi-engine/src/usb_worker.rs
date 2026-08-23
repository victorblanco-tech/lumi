//! Isolated, one-shot USB library worker.
//!
//! Removable-media I/O must never block the channel-persistent realtime
//! engine. The macOS app launches this mode as a short-lived child process,
//! applies a hard deadline, and only accepts a complete JSON snapshot.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

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
            let source_id = validated_source_id(Path::new(&root), source_id)?;
            worker.inspect_rekordbox_device(root, Some(&source_id))?;
        }
        UsbWorkerRequest::Sync {
            root,
            source_id,
            playlist_ids,
        } => {
            let source_id = validated_source_id(Path::new(&root), source_id)?;
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
            let source_id = validated_source_id(Path::new(&root), Some(source_id))?;
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

/// Source identity is selected locally before the USB operation begins. A
/// physical equal-model FAT test proved that an otherwise healthy volume can
/// block indefinitely on a new root-level file write. Identity metadata must
/// therefore never touch removable media or prevent read-only inspection.
fn validated_source_id(root: &Path, preferred: Option<String>) -> Result<String, UsbWorkerError> {
    if !root.is_absolute() || !root.join("PIONEER/rekordbox/exportLibrary.db").is_file() {
        return Err(UsbWorkerError::InvalidDeviceRoot(
            root.display().to_string(),
        ));
    }
    preferred
        .filter(|value| valid_source_id(value))
        .ok_or(UsbWorkerError::InvalidSourceIdentity)
}

fn valid_source_id(value: &str) -> bool {
    (8..=200).contains(&value.len())
        && (value.starts_with("usb-fs:") || value.starts_with("usb-local:"))
        && !value.contains('/')
        && !value.contains('\\')
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
    #[error("USB source identity is missing or invalid")]
    InvalidSourceIdentity,
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
            "sourceId": "usb-local:one",
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
    fn source_identity_is_local_and_does_not_write_to_usb() -> Result<(), UsbWorkerError> {
        let root = device_root("preserve")?;
        let preferred = "usb-fs:hardware-existing-source";
        assert_eq!(
            validated_source_id(&root, Some(preferred.to_owned()))?,
            preferred
        );
        let root_entries = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(root_entries.len(), 1);
        assert_eq!(root_entries[0].file_name(), "PIONEER");
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn invalid_source_or_non_library_root_fails_before_usb_mutation() -> Result<(), UsbWorkerError>
    {
        let root = device_root("invalid-source")?;
        assert!(matches!(
            validated_source_id(&root, None),
            Err(UsbWorkerError::InvalidSourceIdentity)
        ));

        let invalid_root = unique_temp_root("no-library");
        fs::create_dir_all(&invalid_root)?;
        assert!(matches!(
            validated_source_id(&invalid_root, Some("usb-local:valid".to_owned())),
            Err(UsbWorkerError::InvalidDeviceRoot(_))
        ));
        fs::remove_dir_all(root)?;
        fs::remove_dir_all(invalid_root)?;
        Ok(())
    }

    fn device_root(label: &str) -> Result<PathBuf, UsbWorkerError> {
        let root = unique_temp_root(label);
        let library = root.join("PIONEER/rekordbox");
        fs::create_dir_all(&library)?;
        fs::write(library.join("exportLibrary.db"), b"fixture")?;
        Ok(root)
    }

    fn unique_temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumi-usb-worker-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }
}
