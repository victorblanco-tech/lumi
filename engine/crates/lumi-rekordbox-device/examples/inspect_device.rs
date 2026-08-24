use std::env;
use std::process::ExitCode;

use lumi_rekordbox_device::read_device_library;

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(root) = arguments.next() else {
        eprintln!("usage: inspect_device <device-root> [device-track-id]");
        return ExitCode::FAILURE;
    };
    let requested_track = arguments.next();
    let snapshot = match read_device_library(&root) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("device read failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "source={} display={} tracks={} playlists={} database_revision={}",
        snapshot.source_id,
        snapshot.display_name,
        snapshot.tracks.len(),
        snapshot.playlists.len(),
        snapshot.database_revision
    );
    for playlist in snapshot.playlists.iter().take(20) {
        println!(
            "playlist_id={} path={:?} tracks={}",
            playlist.device_playlist_id,
            playlist.path,
            playlist.track_ids.len()
        );
    }
    let selected = requested_track.as_deref().and_then(|requested| {
        requested.parse::<u32>().ok().map_or_else(
            || {
                snapshot.tracks.values().find(|track| {
                    track
                        .title
                        .to_lowercase()
                        .contains(&requested.to_lowercase())
                })
            },
            |device_track_id| snapshot.track(device_track_id),
        )
    });
    if let Some(track) = selected {
        println!(
            "id={} title={:?} artist={:?} color_rgb={:?} bpm_milli={} duration_millis={} file_size={} simulator_signature={} audio={} analysis={} metadata_revision={} analysis_revision={}",
            track.device_track_id,
            track.title,
            track.artist,
            track.color_rgb,
            track.bpm_milli,
            track.duration_millis,
            track.file_size,
            track.simulator_signature,
            track.audio_path.display(),
            track.analysis_dat_path.display(),
            track.metadata_revision,
            track.analysis_revision
        );
    } else if let Some(requested) = requested_track {
        eprintln!("device track {requested:?} not found");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
