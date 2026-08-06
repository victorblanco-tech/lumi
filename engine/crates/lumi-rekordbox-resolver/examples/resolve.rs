use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use lumi_rekordbox_analysis::{
    ResolvedAnalysisRequest, ResolvedAnalysisTrack, snapshot_resolved_analysis,
};
use lumi_rekordbox_resolver::{
    DatabaseKey, RequestedTrack, SqlCipherResolver, create_database_snapshot,
};
use lumi_rekordbox_xml::{RekordboxXmlSyncRequest, load_latest_mirror};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse()?;
    let mirror = load_latest_mirror(&RekordboxXmlSyncRequest::try_new(
        arguments.xml_folder,
        arguments.playlists,
        arguments.include_children,
    )?)?;
    let requested = mirror
        .tracks()
        .iter()
        .map(|track| RequestedTrack::try_new(track.source_track_id(), track.location()))
        .collect::<Result<Vec<_>, _>>()?;
    let snapshot = create_database_snapshot(arguments.database, arguments.database_snapshot)?;
    let key_value = env::var("LUMI_REKORDBOX_DB_KEY")
        .map_err(|_| io::Error::other("LUMI_REKORDBOX_DB_KEY is required"))?;
    let key = DatabaseKey::try_new(key_value)?;
    let resolver = SqlCipherResolver::try_new(arguments.sqlcipher)?;
    let resolved = resolver.resolve(&snapshot, &key, &arguments.analysis_root, requested)?;
    let analysis_tracks = resolved
        .tracks
        .values()
        .map(|track| ResolvedAnalysisTrack::try_new(track.source_track_id(), track.analysis_file()))
        .collect::<Result<Vec<_>, _>>()?;
    let analysis = snapshot_resolved_analysis(&ResolvedAnalysisRequest::try_new(
        &arguments.analysis_root,
        arguments.analysis_snapshot,
        analysis_tracks,
    )?)?;

    println!("database_snapshot_sha256={}", snapshot.sha256());
    println!("requested_tracks={}", resolved.report.requested_tracks);
    println!("resolved_tracks={}", resolved.report.resolved_tracks);
    println!(
        "missing_database_rows={}",
        resolved.report.missing_database_rows
    );
    println!(
        "missing_analysis_paths={}",
        resolved.report.missing_analysis_paths
    );
    println!(
        "audio_path_mismatches={}",
        resolved.report.audio_path_mismatches
    );
    println!("analysis_snapshot_files={}", analysis.report.snapshot_files);
    println!(
        "tracks_with_beat_grid={}",
        analysis.report.tracks_with_beat_grid
    );
    println!(
        "tracks_with_phrases={}",
        analysis.report.tracks_with_phrases
    );
    println!(
        "tracks_with_color_waveform={}",
        analysis.report.tracks_with_color_waveform
    );
    println!(
        "tracks_with_three_band_waveform={}",
        analysis.report.tracks_with_three_band_waveform
    );
    Ok(())
}

#[derive(Debug)]
struct Arguments {
    xml_folder: PathBuf,
    database: PathBuf,
    database_snapshot: PathBuf,
    analysis_root: PathBuf,
    analysis_snapshot: PathBuf,
    sqlcipher: PathBuf,
    playlists: Vec<String>,
    include_children: bool,
}

impl Arguments {
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut values = env::args().skip(1);
        let mut xml_folder = None;
        let mut database = None;
        let mut database_snapshot = None;
        let mut analysis_root = None;
        let mut analysis_snapshot = None;
        let mut sqlcipher = PathBuf::from("/usr/local/bin/sqlcipher");
        let mut playlists = Vec::new();
        let mut include_children = false;
        while let Some(argument) = values.next() {
            match argument.as_str() {
                "--xml-folder" => xml_folder = Some(PathBuf::from(next_value(&mut values)?)),
                "--database" => database = Some(PathBuf::from(next_value(&mut values)?)),
                "--database-snapshot" => {
                    database_snapshot = Some(PathBuf::from(next_value(&mut values)?));
                }
                "--analysis-root" => {
                    analysis_root = Some(PathBuf::from(next_value(&mut values)?));
                }
                "--analysis-snapshot" => {
                    analysis_snapshot = Some(PathBuf::from(next_value(&mut values)?));
                }
                "--sqlcipher" => sqlcipher = PathBuf::from(next_value(&mut values)?),
                "--playlist" => playlists.push(next_value(&mut values)?),
                "--include-children" => include_children = true,
                _ => return Err(io::Error::other("unknown resolver argument").into()),
            }
        }
        Ok(Self {
            xml_folder: xml_folder.ok_or_else(|| io::Error::other("missing --xml-folder"))?,
            database: database.ok_or_else(|| io::Error::other("missing --database"))?,
            database_snapshot: database_snapshot
                .ok_or_else(|| io::Error::other("missing --database-snapshot"))?,
            analysis_root: analysis_root
                .ok_or_else(|| io::Error::other("missing --analysis-root"))?,
            analysis_snapshot: analysis_snapshot
                .ok_or_else(|| io::Error::other("missing --analysis-snapshot"))?,
            sqlcipher,
            playlists,
            include_children,
        })
    }
}

fn next_value(values: &mut impl Iterator<Item = String>) -> Result<String, io::Error> {
    values
        .next()
        .ok_or_else(|| io::Error::other("missing resolver argument value"))
}
