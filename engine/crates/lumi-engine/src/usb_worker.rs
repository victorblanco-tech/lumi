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
            worker.inspect_rekordbox_device(root, source_id.as_deref())?;
        }
        UsbWorkerRequest::Sync {
            root,
            source_id,
            playlist_ids,
        } => {
            worker.sync_rekordbox_device(root, source_id.as_deref(), &playlist_ids)?;
        }
        UsbWorkerRequest::ResolveConflict {
            root,
            source_id,
            device_track_id,
            expected_incoming_revision,
            expected_active_revision,
            choice,
        } => {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
