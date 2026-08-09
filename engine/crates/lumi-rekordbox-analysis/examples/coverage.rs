use std::error::Error;
use std::io;
use std::path::PathBuf;

use lumi_rekordbox_analysis::{AnalysisScanRequest, scan_and_snapshot};
use lumi_rekordbox_xml::{RekordboxXmlSyncRequest, load_latest_mirror};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse()?;
    let xml_request = RekordboxXmlSyncRequest::try_new(
        arguments.xml_folder,
        arguments.playlists,
        arguments.include_children,
    )?;
    let mirror = load_latest_mirror(&xml_request)?;
    let locations = mirror
        .tracks()
        .iter()
        .map(|track| track.location().to_owned())
        .collect::<Vec<_>>();
    let analysis_request =
        AnalysisScanRequest::try_new(arguments.analysis_root, arguments.snapshot_root, locations)?
            .allow_provisional_unique_filename_matches();
    let result = scan_and_snapshot(&analysis_request)?;
    let report = result.report;
    println!("requested_tracks={}", report.requested_tracks);
    println!(
        "requested_locations_present={}",
        report.requested_locations_present
    );
    println!("scanned_files={}", report.scanned_files);
    println!("scanned_analysis_sets={}", report.scanned_analysis_sets);
    println!(
        "analysis_locations_present={}",
        report.analysis_locations_present
    );
    println!("basename_candidates={}", report.basename_candidates);
    println!("exact_path_matches={}", report.exact_path_matches);
    println!(
        "relocated_suffix_matches={}",
        report.relocated_suffix_matches
    );
    println!(
        "ambiguous_relocated_candidates={}",
        report.ambiguous_relocated_candidates
    );
    println!(
        "provisional_filename_matches={}",
        report.provisional_filename_matches
    );
    println!(
        "ambiguous_filename_candidates={}",
        report.ambiguous_filename_candidates
    );
    println!("malformed_analysis_sets={}", report.malformed_analysis_sets);
    println!("matched_tracks={}", report.matched_tracks);
    println!("missing_tracks={}", report.missing_tracks);
    println!("tracks_with_beat_grid={}", report.tracks_with_beat_grid);
    println!("tracks_with_phrases={}", report.tracks_with_phrases);
    println!(
        "tracks_with_color_waveform={}",
        report.tracks_with_color_waveform
    );
    println!(
        "tracks_with_three_band_waveform={}",
        report.tracks_with_three_band_waveform
    );
    println!("snapshot_files={}", report.snapshot_files);
    println!("total_beat_grid_entries={}", report.total_beat_grid_entries);
    println!("total_phrase_entries={}", report.total_phrase_entries);
    Ok(())
}

#[derive(Debug)]
struct Arguments {
    xml_folder: PathBuf,
    analysis_root: PathBuf,
    snapshot_root: PathBuf,
    playlists: Vec<String>,
    include_children: bool,
}

impl Arguments {
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut values = std::env::args().skip(1);
        let mut xml_folder = None;
        let mut analysis_root = None;
        let mut snapshot_root = None;
        let mut playlists = Vec::new();
        let mut include_children = false;
        while let Some(argument) = values.next() {
            match argument.as_str() {
                "--xml-folder" => xml_folder = Some(PathBuf::from(next_value(&mut values)?)),
                "--analysis-root" => {
                    analysis_root = Some(PathBuf::from(next_value(&mut values)?));
                }
                "--snapshot-root" => {
                    snapshot_root = Some(PathBuf::from(next_value(&mut values)?));
                }
                "--playlist" => playlists.push(next_value(&mut values)?),
                "--include-children" => include_children = true,
                _ => return Err(io::Error::other("unknown coverage argument").into()),
            }
        }
        Ok(Self {
            xml_folder: xml_folder.ok_or_else(|| io::Error::other("missing --xml-folder"))?,
            analysis_root: analysis_root
                .ok_or_else(|| io::Error::other("missing --analysis-root"))?,
            snapshot_root: snapshot_root
                .ok_or_else(|| io::Error::other("missing --snapshot-root"))?,
            playlists,
            include_children,
        })
    }
}

fn next_value(values: &mut impl Iterator<Item = String>) -> Result<String, io::Error> {
    values
        .next()
        .ok_or_else(|| io::Error::other("missing coverage argument value"))
}
