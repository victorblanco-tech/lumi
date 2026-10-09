//! Read-only OneLibrary projection for the separately packaged simulator.
//! Uses Lumi's importer so simulator IDs and playlist ancestry match real sync.
use std::io::{self, Write};
use std::process::ExitCode;

use lumi_rekordbox_device::{DeviceLibrarySnapshot, read_device_library};
use serde_json::{Value, json};

fn projection(library: &DeviceLibrarySnapshot) -> Value {
    json!({
        "schemaVersion": 1,
        "format": "OneLibrary",
        "name": library.display_name,
        "tracks": library.tracks.values().map(|track| json!({
            "id": track.device_track_id,
            "title": track.title,
            "artist": track.artist,
            "tempoCentiBpm": track.bpm_milli / 10,
            "durationMillis": track.duration_millis,
            "analysisPath": track.analysis_dat_path,
        })).collect::<Vec<_>>(),
        "playlists": library.playlists.iter().map(|playlist| json!({
            "id": playlist.device_playlist_id,
            "name": playlist.name,
            "folders": playlist.folder_names,
            "trackIds": playlist.track_ids,
        })).collect::<Vec<_>>(),
    })
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let root = arguments
        .next()
        .ok_or("A OneLibrary USB root is required")?;
    if arguments.next().is_some() {
        return Err("Expected exactly one USB root".into());
    }
    let library = read_device_library(root)?;
    let bytes = serde_json::to_vec(&projection(&library))?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Simulator library projection exceeds 16 MiB".into());
    }
    io::stdout().lock().write_all(&bytes)?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("OneLibrary could not be read: {error}");
            ExitCode::FAILURE
        }
    }
}
