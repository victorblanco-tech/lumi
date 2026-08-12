//! Bounded, read-only Rekordbox ANLZ capability discovery and snapshot parsing.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

const ANALYSIS_MAGIC: &[u8; 4] = b"PMAI";
const TAG_HEADER_BYTES: usize = 12;
const MAX_REQUESTED_TRACKS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisScanLimits {
    pub maximum_files: usize,
    pub maximum_depth: usize,
    pub maximum_file_bytes: u64,
}

impl Default for AnalysisScanLimits {
    fn default() -> Self {
        Self {
            maximum_files: 250_000,
            maximum_depth: 8,
            maximum_file_bytes: 64 * 1_024 * 1_024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisScanRequest {
    analysis_root: PathBuf,
    snapshot_root: PathBuf,
    requested_locations: BTreeSet<String>,
    limits: AnalysisScanLimits,
    allow_provisional_unique_filename_matches: bool,
}

impl AnalysisScanRequest {
    pub fn try_new(
        analysis_root: impl Into<PathBuf>,
        snapshot_root: impl Into<PathBuf>,
        requested_locations: impl IntoIterator<Item = String>,
    ) -> Result<Self, AnalysisError> {
        let analysis_root = analysis_root.into();
        let snapshot_root = snapshot_root.into();
        let requested_locations = requested_locations
            .into_iter()
            .filter_map(|location| normalize_location(&location))
            .collect::<BTreeSet<_>>();
        if requested_locations.is_empty() || requested_locations.len() > MAX_REQUESTED_TRACKS {
            return Err(AnalysisError::InvalidRequestedTrackCount(
                requested_locations.len(),
            ));
        }
        if !analysis_root.is_dir() {
            return Err(AnalysisError::InvalidAnalysisRoot);
        }
        if snapshot_root.exists() {
            let mut entries = fs::read_dir(&snapshot_root)?;
            if entries.next().transpose()?.is_some() {
                return Err(AnalysisError::SnapshotRootNotEmpty);
            }
        }
        let canonical_analysis = fs::canonicalize(&analysis_root)?;
        let canonical_snapshot_parent = canonical_existing_ancestor(&snapshot_root)?;
        if canonical_snapshot_parent.starts_with(&canonical_analysis) {
            return Err(AnalysisError::SnapshotInsideAnalysisRoot);
        }
        Ok(Self {
            analysis_root,
            snapshot_root,
            requested_locations,
            limits: AnalysisScanLimits::default(),
            allow_provisional_unique_filename_matches: false,
        })
    }

    #[must_use]
    pub const fn with_limits(mut self, limits: AnalysisScanLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Enables a POC-only match when one filename occurs exactly once in both
    /// the requested XML scope and the complete analysis tree. Production
    /// enrichment must use a stronger source identity.
    #[must_use]
    pub const fn allow_provisional_unique_filename_matches(mut self) -> Self {
        self.allow_provisional_unique_filename_matches = true;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WaveformCoverage {
    pub preview_points: usize,
    pub detailed_points: usize,
    pub color_preview_points: usize,
    pub color_detailed_points: usize,
    pub three_band_preview_points: usize,
    pub three_band_detailed_points: usize,
}

impl WaveformCoverage {
    fn merge(&mut self, other: Self) {
        self.preview_points = self.preview_points.max(other.preview_points);
        self.detailed_points = self.detailed_points.max(other.detailed_points);
        self.color_preview_points = self.color_preview_points.max(other.color_preview_points);
        self.color_detailed_points = self.color_detailed_points.max(other.color_detailed_points);
        self.three_band_preview_points = self
            .three_band_preview_points
            .max(other.three_band_preview_points);
        self.three_band_detailed_points = self
            .three_band_detailed_points
            .max(other.three_band_detailed_points);
    }

    #[must_use]
    pub const fn has_color_waveform(self) -> bool {
        self.color_preview_points > 0 || self.color_detailed_points > 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrackAnalysisCoverage {
    pub beat_grid_entries: usize,
    pub phrase_entries: usize,
    pub hot_cue_entries: usize,
    pub waveform: WaveformCoverage,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisCoverageReport {
    pub requested_tracks: usize,
    pub requested_locations_present: usize,
    pub scanned_files: usize,
    pub scanned_analysis_sets: usize,
    pub analysis_locations_present: usize,
    pub basename_candidates: usize,
    pub exact_path_matches: usize,
    pub relocated_suffix_matches: usize,
    pub ambiguous_relocated_candidates: usize,
    pub provisional_filename_matches: usize,
    pub ambiguous_filename_candidates: usize,
    pub malformed_analysis_sets: usize,
    pub matched_tracks: usize,
    pub missing_tracks: usize,
    pub tracks_with_beat_grid: usize,
    pub tracks_with_phrases: usize,
    pub tracks_with_color_waveform: usize,
    pub tracks_with_three_band_waveform: usize,
    pub snapshot_files: usize,
    pub total_beat_grid_entries: usize,
    pub total_phrase_entries: usize,
    pub total_hot_cue_entries: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisScanResult {
    pub report: AnalysisCoverageReport,
    pub tracks: BTreeMap<String, TrackAnalysisCoverage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisBeat {
    pub beat_number: u16,
    pub tempo_centi_bpm: u16,
    pub time_millis: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisWaveformPoint {
    pub low: u8,
    pub mid: u8,
    pub high: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisPhrase {
    pub start_beat: u32,
    pub end_beat: u32,
    pub source_label: String,
}

/// Read-only Rekordbox performance marker. `index` is one-based (A = 1),
/// while timing stays in source milliseconds so it remains aligned to the
/// authoritative Rekordbox beat grid after import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisHotCue {
    pub index: u8,
    pub time_millis: u32,
    pub loop_end_millis: Option<u32>,
    pub comment: String,
    pub color_rgb: [u8; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTrackAnalysis {
    pub coverage: TrackAnalysisCoverage,
    pub beat_grid: Vec<AnalysisBeat>,
    pub waveform: Vec<AnalysisWaveformPoint>,
    pub phrases: Vec<AnalysisPhrase>,
    pub hot_cues: Vec<AnalysisHotCue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAnalysisDataResult {
    pub report: AnalysisCoverageReport,
    pub tracks: BTreeMap<String, ResolvedTrackAnalysis>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAnalysisTrack {
    identity: String,
    dat_path: PathBuf,
}

impl ResolvedAnalysisTrack {
    pub fn try_new(
        identity: impl Into<String>,
        dat_path: impl Into<PathBuf>,
    ) -> Result<Self, AnalysisError> {
        let identity = identity.into();
        let dat_path = dat_path.into();
        if identity.trim().is_empty() {
            return Err(AnalysisError::InvalidResolvedIdentity);
        }
        Ok(Self { identity, dat_path })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAnalysisRequest {
    analysis_root: PathBuf,
    snapshot_root: PathBuf,
    tracks: Vec<ResolvedAnalysisTrack>,
    limits: AnalysisScanLimits,
}

impl ResolvedAnalysisRequest {
    pub fn try_new(
        analysis_root: impl Into<PathBuf>,
        snapshot_root: impl Into<PathBuf>,
        tracks: impl IntoIterator<Item = ResolvedAnalysisTrack>,
    ) -> Result<Self, AnalysisError> {
        let analysis_root = analysis_root.into();
        let snapshot_root = snapshot_root.into();
        let tracks = tracks.into_iter().collect::<Vec<_>>();
        if tracks.is_empty() || tracks.len() > MAX_REQUESTED_TRACKS {
            return Err(AnalysisError::InvalidRequestedTrackCount(tracks.len()));
        }
        if tracks
            .iter()
            .map(|track| track.identity.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != tracks.len()
        {
            return Err(AnalysisError::DuplicateResolvedIdentity);
        }
        if !analysis_root.is_dir() {
            return Err(AnalysisError::InvalidAnalysisRoot);
        }
        if snapshot_root.exists() {
            let mut entries = fs::read_dir(&snapshot_root)?;
            if entries.next().transpose()?.is_some() {
                return Err(AnalysisError::SnapshotRootNotEmpty);
            }
        }
        let canonical_analysis = fs::canonicalize(&analysis_root)?;
        let canonical_snapshot_parent = canonical_existing_ancestor(&snapshot_root)?;
        if canonical_snapshot_parent.starts_with(&canonical_analysis) {
            return Err(AnalysisError::SnapshotInsideAnalysisRoot);
        }
        for track in &tracks {
            let canonical = fs::canonicalize(&track.dat_path)?;
            if !canonical.starts_with(&canonical_analysis) || !is_analysis_dat(&canonical) {
                return Err(AnalysisError::ResolvedFileOutsideAnalysisRoot);
            }
        }
        Ok(Self {
            analysis_root: canonical_analysis,
            snapshot_root,
            tracks,
            limits: AnalysisScanLimits::default(),
        })
    }

    #[must_use]
    pub const fn with_limits(mut self, limits: AnalysisScanLimits) -> Self {
        self.limits = limits;
        self
    }
}

/// Scans only `ANLZ*.DAT` files, matches their `PPTH` audio location to the
/// requested XML locations, and parses copied companions from a Lumi-owned snapshot.
pub fn scan_and_snapshot(
    request: &AnalysisScanRequest,
) -> Result<AnalysisScanResult, AnalysisError> {
    fs::create_dir_all(&request.snapshot_root)?;
    let candidates = discover_dat_files(
        &request.analysis_root,
        request.limits.maximum_depth,
        request.limits.maximum_files,
    )?;
    let mut report = AnalysisCoverageReport {
        requested_tracks: request.requested_locations.len(),
        requested_locations_present: request
            .requested_locations
            .iter()
            .filter(|location| Path::new(location).is_file())
            .count(),
        scanned_files: candidates.scanned_files,
        scanned_analysis_sets: candidates.dat_files.len(),
        ..AnalysisCoverageReport::default()
    };
    let mut remaining = request.requested_locations.clone();
    let requested_basenames = request
        .requested_locations
        .iter()
        .filter_map(|location| Path::new(location).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    let mut analysis_candidates = Vec::new();

    for dat_path in candidates.dat_files {
        let dat = match read_bounded(&dat_path, request.limits.maximum_file_bytes) {
            Ok(bytes) => bytes,
            Err(AnalysisError::InvalidFileSize(_)) => {
                report.malformed_analysis_sets += 1;
                continue;
            }
            Err(error) => return Err(error),
        };
        let summary = match parse_analysis_file(&dat) {
            Ok(summary) => summary,
            Err(AnalysisError::MalformedAnalysisFile) => {
                report.malformed_analysis_sets += 1;
                continue;
            }
            Err(error) => return Err(error),
        };
        let Some(location) = summary.audio_location else {
            continue;
        };
        if Path::new(&location).is_file() {
            report.analysis_locations_present += 1;
        }
        if Path::new(&location)
            .file_name()
            .is_some_and(|name| requested_basenames.contains(name.to_string_lossy().as_ref()))
        {
            report.basename_candidates += 1;
        }
        analysis_candidates.push(AnalysisCandidate { dat_path, location });
    }

    let mut matched_candidate_indexes = BTreeSet::new();
    let mut matches = Vec::new();
    let mut exact_candidates = BTreeMap::<String, Vec<usize>>::new();
    for (index, candidate) in analysis_candidates.iter().enumerate() {
        exact_candidates
            .entry(candidate.location.clone())
            .or_default()
            .push(index);
    }
    for requested in &request.requested_locations {
        let Some(indexes) = exact_candidates.get(requested) else {
            continue;
        };
        if indexes.len() == 1 {
            let index = indexes[0];
            matched_candidate_indexes.insert(index);
            remaining.remove(requested);
            matches.push((requested.clone(), index));
            report.exact_path_matches += 1;
        }
    }

    let mut requested_by_suffix = BTreeMap::<String, Vec<String>>::new();
    for requested in &remaining {
        if let Some(suffix) = location_suffix(requested, 4) {
            requested_by_suffix
                .entry(suffix)
                .or_default()
                .push(requested.clone());
        }
    }
    let mut candidates_by_suffix = BTreeMap::<String, Vec<usize>>::new();
    for (index, candidate) in analysis_candidates.iter().enumerate() {
        if matched_candidate_indexes.contains(&index) {
            continue;
        }
        if let Some(suffix) = location_suffix(&candidate.location, 4) {
            candidates_by_suffix.entry(suffix).or_default().push(index);
        }
    }
    for (suffix, requested) in requested_by_suffix {
        let Some(candidate_indexes) = candidates_by_suffix.get(&suffix) else {
            continue;
        };
        if requested.len() == 1 && candidate_indexes.len() == 1 {
            let requested_location = requested[0].clone();
            let index = candidate_indexes[0];
            matched_candidate_indexes.insert(index);
            remaining.remove(&requested_location);
            matches.push((requested_location, index));
            report.relocated_suffix_matches += 1;
        } else {
            report.ambiguous_relocated_candidates += 1;
        }
    }

    if request.allow_provisional_unique_filename_matches {
        let mut requested_by_filename = BTreeMap::<String, Vec<String>>::new();
        for requested in &remaining {
            if let Some(filename) = location_filename(requested) {
                requested_by_filename
                    .entry(filename)
                    .or_default()
                    .push(requested.clone());
            }
        }
        let mut candidates_by_filename = BTreeMap::<String, Vec<usize>>::new();
        for (index, candidate) in analysis_candidates.iter().enumerate() {
            if matched_candidate_indexes.contains(&index) {
                continue;
            }
            if let Some(filename) = location_filename(&candidate.location) {
                candidates_by_filename
                    .entry(filename)
                    .or_default()
                    .push(index);
            }
        }
        for (filename, requested) in requested_by_filename {
            let Some(candidate_indexes) = candidates_by_filename.get(&filename) else {
                continue;
            };
            if requested.len() == 1 && candidate_indexes.len() == 1 {
                let requested_location = requested[0].clone();
                let index = candidate_indexes[0];
                matched_candidate_indexes.insert(index);
                remaining.remove(&requested_location);
                matches.push((requested_location, index));
                report.provisional_filename_matches += 1;
            } else {
                report.ambiguous_filename_candidates += 1;
            }
        }
    }

    let mut tracks = BTreeMap::new();
    for (requested_location, index) in matches {
        let candidate = &analysis_candidates[index];
        let snapshot_directory = request
            .snapshot_root
            .join(sha256_hex(requested_location.as_bytes()));
        fs::create_dir(&snapshot_directory)?;
        let copied = copy_analysis_set(&candidate.dat_path, &snapshot_directory)?;
        report.snapshot_files += copied.len();
        let mut coverage = TrackAnalysisCoverage::default();
        for copied_path in copied {
            let bytes = read_bounded(&copied_path, request.limits.maximum_file_bytes)?;
            let parsed = parse_analysis_file(&bytes)?;
            let parsed_coverage = parsed.coverage();
            coverage.beat_grid_entries = coverage
                .beat_grid_entries
                .max(parsed_coverage.beat_grid_entries);
            coverage.phrase_entries = coverage.phrase_entries.max(parsed_coverage.phrase_entries);
            coverage.waveform.merge(parsed_coverage.waveform);
        }
        accumulate_coverage(&mut report, coverage);
        tracks.insert(requested_location, coverage);
    }

    report.matched_tracks = tracks.len();
    report.missing_tracks = request.requested_locations.len() - report.matched_tracks;
    Ok(AnalysisScanResult { report, tracks })
}

/// Snapshots and parses database-resolved analysis sets. No path or filename
/// matching is performed: the caller supplies the authoritative source ID → DAT mapping.
pub fn snapshot_resolved_analysis(
    request: &ResolvedAnalysisRequest,
) -> Result<AnalysisScanResult, AnalysisError> {
    let result = snapshot_resolved_analysis_data(request)?;
    Ok(AnalysisScanResult {
        report: result.report,
        tracks: result
            .tracks
            .into_iter()
            .map(|(identity, analysis)| (identity, analysis.coverage))
            .collect(),
    })
}

/// Snapshots database-resolved analysis sets and returns bounded typed data for
/// a canonical Lumi import. The production files are never parsed in place.
pub fn snapshot_resolved_analysis_data(
    request: &ResolvedAnalysisRequest,
) -> Result<ResolvedAnalysisDataResult, AnalysisError> {
    fs::create_dir_all(&request.snapshot_root)?;
    let mut report = AnalysisCoverageReport {
        requested_tracks: request.tracks.len(),
        ..AnalysisCoverageReport::default()
    };
    let mut tracks = BTreeMap::new();
    for track in &request.tracks {
        let dat_path = fs::canonicalize(&track.dat_path)?;
        if !dat_path.starts_with(&request.analysis_root) || !is_analysis_dat(&dat_path) {
            return Err(AnalysisError::ResolvedFileOutsideAnalysisRoot);
        }
        let snapshot_directory = request
            .snapshot_root
            .join(sha256_hex(track.identity.as_bytes()));
        fs::create_dir(&snapshot_directory)?;
        let copied = copy_analysis_set(&dat_path, &snapshot_directory)?;
        report.snapshot_files += copied.len();
        let mut merged = ParsedAnalysisFile::default();
        for copied_path in copied {
            let bytes = read_bounded(&copied_path, request.limits.maximum_file_bytes)?;
            let parsed = parse_analysis_file(&bytes)?;
            merged.merge(parsed);
        }
        let coverage = merged.coverage();
        accumulate_coverage(&mut report, coverage);
        tracks.insert(
            track.identity.clone(),
            ResolvedTrackAnalysis {
                coverage,
                beat_grid: merged.beat_grid,
                waveform: merged.waveform,
                phrases: merged.phrases,
                hot_cues: merged.hot_cues,
            },
        );
    }
    report.matched_tracks = tracks.len();
    Ok(ResolvedAnalysisDataResult { report, tracks })
}

#[derive(Debug)]
struct AnalysisCandidate {
    dat_path: PathBuf,
    location: String,
}

fn location_suffix(location: &str, component_count: usize) -> Option<String> {
    let components = Path::new(location)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if components.len() < component_count {
        return None;
    }
    Some(components[components.len() - component_count..].join("\u{1f}"))
}

fn location_filename(location: &str) -> Option<String> {
    Path::new(location)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn is_analysis_dat(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("DAT"))
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem.starts_with("ANLZ"))
}

fn accumulate_coverage(report: &mut AnalysisCoverageReport, coverage: TrackAnalysisCoverage) {
    if coverage.beat_grid_entries > 0 {
        report.tracks_with_beat_grid += 1;
    }
    if coverage.phrase_entries > 0 {
        report.tracks_with_phrases += 1;
    }
    if coverage.waveform.has_color_waveform() {
        report.tracks_with_color_waveform += 1;
    }
    if coverage.waveform.three_band_preview_points > 0
        || coverage.waveform.three_band_detailed_points > 0
    {
        report.tracks_with_three_band_waveform += 1;
    }
    report.total_beat_grid_entries += coverage.beat_grid_entries;
    report.total_phrase_entries += coverage.phrase_entries;
    report.total_hot_cue_entries += coverage.hot_cue_entries;
}

#[derive(Debug)]
struct Discovery {
    scanned_files: usize,
    dat_files: Vec<PathBuf>,
}

fn discover_dat_files(
    root: &Path,
    maximum_depth: usize,
    maximum_files: usize,
) -> Result<Discovery, AnalysisError> {
    let mut directories = vec![(root.to_path_buf(), 0_usize)];
    let mut dat_files = Vec::new();
    let mut scanned_files = 0_usize;
    while let Some((directory, depth)) = directories.pop() {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if depth >= maximum_depth {
                    return Err(AnalysisError::TraversalLimitExceeded);
                }
                directories.push((entry.path(), depth + 1));
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            scanned_files += 1;
            if scanned_files > maximum_files {
                return Err(AnalysisError::TraversalLimitExceeded);
            }
            let path = entry.path();
            let is_dat = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("DAT"));
            let is_analysis = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.starts_with("ANLZ"));
            if is_dat && is_analysis {
                dat_files.push(path);
            }
        }
    }
    dat_files.sort();
    Ok(Discovery {
        scanned_files,
        dat_files,
    })
}

fn copy_analysis_set(dat_path: &Path, destination: &Path) -> Result<Vec<PathBuf>, AnalysisError> {
    let mut sources = vec![dat_path.to_path_buf()];
    for extension in ["EXT", "2EX"] {
        let companion = dat_path.with_extension(extension);
        if companion.is_file() {
            sources.push(companion);
        }
    }
    let mut copied = Vec::with_capacity(sources.len());
    for source in sources {
        let before = fs::metadata(&source)?;
        let file_name = source
            .file_name()
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
        let destination_path = destination.join(file_name);
        let temporary_path = destination.join(format!("{}.copying", file_name.to_string_lossy()));
        fs::copy(&source, &temporary_path)?;
        let after = fs::metadata(&source)?;
        if !same_source_version(&before, &after) {
            return Err(AnalysisError::SourceChangedDuringSnapshot);
        }
        fs::rename(&temporary_path, &destination_path)?;
        copied.push(destination_path);
    }
    Ok(copied)
}

fn same_source_version(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
        && before.created().ok() == after.created().ok()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedAnalysisFile {
    audio_location: Option<String>,
    beat_grid: Vec<AnalysisBeat>,
    phrases: Vec<AnalysisPhrase>,
    hot_cues: Vec<AnalysisHotCue>,
    hot_cue_priority: u8,
    waveform: Vec<AnalysisWaveformPoint>,
    waveform_priority: u8,
    waveform_coverage: WaveformCoverage,
}

impl ParsedAnalysisFile {
    fn coverage(&self) -> TrackAnalysisCoverage {
        TrackAnalysisCoverage {
            beat_grid_entries: self.beat_grid.len(),
            phrase_entries: self.phrases.len(),
            hot_cue_entries: self.hot_cues.len(),
            waveform: self.waveform_coverage,
        }
    }

    fn merge(&mut self, other: Self) {
        if other.beat_grid.len() > self.beat_grid.len() {
            self.beat_grid = other.beat_grid;
        }
        if other.phrases.len() > self.phrases.len() {
            self.phrases = other.phrases;
        }
        if other.hot_cue_priority > self.hot_cue_priority
            || (other.hot_cue_priority == self.hot_cue_priority
                && other.hot_cues.len() > self.hot_cues.len())
        {
            self.hot_cues = other.hot_cues;
            self.hot_cue_priority = other.hot_cue_priority;
        }
        if other.waveform_priority > self.waveform_priority
            || (other.waveform_priority == self.waveform_priority
                && other.waveform.len() > self.waveform.len())
        {
            self.waveform = other.waveform;
            self.waveform_priority = other.waveform_priority;
        }
        self.waveform_coverage.merge(other.waveform_coverage);
    }
}

fn parse_analysis_file(bytes: &[u8]) -> Result<ParsedAnalysisFile, AnalysisError> {
    if bytes.len() < TAG_HEADER_BYTES || &bytes[..4] != ANALYSIS_MAGIC {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    let header_length = usize_from_u32(be_u32(bytes, 4)?)?;
    let declared_length = usize_from_u32(be_u32(bytes, 8)?)?;
    if header_length < TAG_HEADER_BYTES
        || declared_length < header_length
        || declared_length > bytes.len()
    {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    let mut parsed = ParsedAnalysisFile::default();
    let mut offset = header_length;
    while offset < declared_length {
        if declared_length - offset < TAG_HEADER_BYTES {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        let tag = &bytes[offset..declared_length];
        let tag_id = &tag[..4];
        let header_bytes = usize_from_u32(be_u32(tag, 4)?)?;
        let tag_bytes = usize_from_u32(be_u32(tag, 8)?)?;
        if header_bytes < TAG_HEADER_BYTES || tag_bytes < header_bytes || tag_bytes > tag.len() {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        // Rekordbox counts tag-specific metadata (for example PPTH's path
        // length and PQTZ's entry count) as part of `len_header`. Those
        // fields still belong to the parseable tag payload, so only the
        // common 12-byte tag prefix is skipped here.
        let body = &tag[TAG_HEADER_BYTES..tag_bytes];
        match tag_id {
            b"PPTH" => parsed.audio_location = parse_path_tag(body)?,
            b"PQTZ" => parsed.beat_grid = parse_beat_grid(body)?,
            b"PWAV" => {
                parsed.waveform_coverage.preview_points = parse_byte_entries(body, 0, 8)?;
            }
            b"PWV2" => {
                parsed.waveform_coverage.preview_points = parsed
                    .waveform_coverage
                    .preview_points
                    .max(parse_byte_entries(body, 0, 8)?);
            }
            b"PWV3" => {
                parsed.waveform_coverage.detailed_points = parse_declared_entries(body, 1)?;
            }
            b"PWV4" => {
                parsed.waveform_coverage.color_preview_points = parse_declared_entries(body, 6)?;
            }
            b"PWV5" => {
                parsed.waveform_coverage.color_detailed_points = parse_declared_entries(body, 2)?;
                let waveform = parse_color_detailed_waveform(body)?;
                if parsed.waveform_priority < 4 {
                    parsed.waveform = waveform;
                    parsed.waveform_priority = 4;
                }
            }
            b"PWV6" => {
                parsed.waveform_coverage.three_band_preview_points =
                    parse_compact_declared_entries(body, 3)?;
                let waveform = parse_three_band_waveform(body, 8)?;
                if parsed.waveform_priority < 2 {
                    parsed.waveform = waveform;
                    parsed.waveform_priority = 2;
                }
            }
            b"PWV7" => {
                parsed.waveform_coverage.three_band_detailed_points =
                    parse_declared_entries(body, 3)?;
                if parsed.waveform_priority < 3 {
                    parsed.waveform = parse_three_band_waveform(body, 12)?;
                    parsed.waveform_priority = 3;
                }
            }
            b"PSSI" => parsed.phrases = parse_phrases(body)?,
            b"PCOB" => {
                let hot_cues = parse_legacy_hot_cues(body)?;
                if parsed.hot_cue_priority < 1 && !hot_cues.is_empty() {
                    parsed.hot_cues = hot_cues;
                    parsed.hot_cue_priority = 1;
                }
            }
            b"PCO2" => {
                let hot_cues = parse_extended_hot_cues(body)?;
                if !hot_cues.is_empty() {
                    parsed.hot_cues = hot_cues;
                    parsed.hot_cue_priority = 2;
                }
            }
            _ => {}
        }
        offset = offset
            .checked_add(tag_bytes)
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
    }
    Ok(parsed)
}

fn parse_legacy_hot_cues(body: &[u8]) -> Result<Vec<AnalysisHotCue>, AnalysisError> {
    if body.len() < 12 {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    let list_type = be_u32(body, 0)?;
    let count = usize::from(be_u16(body, 6)?);
    if list_type != 1 {
        return Ok(Vec::new());
    }
    validate_entry_range(body, count, 56, 12)?;
    let mut cues = Vec::with_capacity(count);
    for entry_index in 0..count {
        let offset = 12 + entry_index * 56;
        let entry = body
            .get(offset..offset + 56)
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
        if &entry[..4] != b"PCPT" || usize_from_u32(be_u32(entry, 8)?)? != 56 {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        let hot_cue = be_u32(entry, 12)?;
        if hot_cue == 0 {
            continue;
        }
        let index = u8::try_from(hot_cue).map_err(|_| AnalysisError::MalformedAnalysisFile)?;
        let cue_type = entry[28];
        if !matches!(cue_type, 0 | 2) {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        let time_millis = be_u32(entry, 32)?;
        let loop_time = be_u32(entry, 36)?;
        cues.push(AnalysisHotCue {
            index,
            time_millis,
            loop_end_millis: (cue_type == 2 && loop_time > time_millis).then_some(loop_time),
            comment: String::new(),
            color_rgb: default_hot_cue_color(),
        });
    }
    normalize_hot_cues(cues)
}

fn parse_extended_hot_cues(body: &[u8]) -> Result<Vec<AnalysisHotCue>, AnalysisError> {
    if body.len() < 8 {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    let list_type = be_u32(body, 0)?;
    let count = usize::from(be_u16(body, 4)?);
    if list_type != 1 {
        return Ok(Vec::new());
    }
    let mut offset = 8_usize;
    let mut cues = Vec::with_capacity(count);
    for _ in 0..count {
        let header = body
            .get(offset..offset + 12)
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
        if &header[..4] != b"PCP2" {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        let entry_bytes = usize_from_u32(be_u32(header, 8)?)?;
        if entry_bytes < 28 {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        let entry = body
            .get(offset..offset + entry_bytes)
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
        let hot_cue = be_u32(entry, 12)?;
        if hot_cue != 0 {
            let index = u8::try_from(hot_cue).map_err(|_| AnalysisError::MalformedAnalysisFile)?;
            let cue_type = entry[16];
            if !matches!(cue_type, 0 | 2) {
                return Err(AnalysisError::MalformedAnalysisFile);
            }
            let time_millis = be_u32(entry, 20)?;
            let loop_time = be_u32(entry, 24)?;
            let (comment, color_rgb) = if entry_bytes >= 44 {
                let comment_bytes = usize_from_u32(be_u32(entry, 40)?)?;
                let comment_end = 44_usize
                    .checked_add(comment_bytes)
                    .ok_or(AnalysisError::MalformedAnalysisFile)?;
                if comment_end > entry_bytes || !comment_bytes.is_multiple_of(2) {
                    return Err(AnalysisError::MalformedAnalysisFile);
                }
                let comment = parse_utf16be(&entry[44..comment_end])?;
                let color = entry.get(comment_end..comment_end + 4).map_or_else(
                    default_hot_cue_color,
                    |bytes| {
                        let rgb = [bytes[1], bytes[2], bytes[3]];
                        if rgb == [0, 0, 0] {
                            default_hot_cue_color()
                        } else {
                            rgb
                        }
                    },
                );
                (comment, color)
            } else {
                (String::new(), default_hot_cue_color())
            };
            cues.push(AnalysisHotCue {
                index,
                time_millis,
                loop_end_millis: (cue_type == 2 && loop_time > time_millis).then_some(loop_time),
                comment,
                color_rgb,
            });
        }
        offset = offset
            .checked_add(entry_bytes)
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
    }
    normalize_hot_cues(cues)
}

fn parse_utf16be(bytes: &[u8]) -> Result<String, AnalysisError> {
    let words = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&words).map_err(|_| AnalysisError::MalformedAnalysisFile)
}

fn normalize_hot_cues(mut cues: Vec<AnalysisHotCue>) -> Result<Vec<AnalysisHotCue>, AnalysisError> {
    cues.sort_by_key(|cue| cue.index);
    if cues.windows(2).any(|pair| pair[0].index == pair[1].index) {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    Ok(cues)
}

const fn default_hot_cue_color() -> [u8; 3] {
    [0x00, 0xd8, 0x7f]
}

fn parse_path_tag(body: &[u8]) -> Result<Option<String>, AnalysisError> {
    let byte_count = usize_from_u32(be_u32(body, 0)?)?;
    if byte_count < 2 || byte_count > body.len().saturating_sub(4) || !byte_count.is_multiple_of(2)
    {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    let path_bytes = &body[4..4 + byte_count - 2];
    let words = path_bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let path = String::from_utf16(&words).map_err(|_| AnalysisError::MalformedAnalysisFile)?;
    Ok(normalize_location(&path))
}

fn parse_byte_entries(
    body: &[u8],
    count_offset: usize,
    data_offset: usize,
) -> Result<usize, AnalysisError> {
    let count = usize_from_u32(be_u32(body, count_offset)?)?;
    if count > body.len().saturating_sub(data_offset) {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    Ok(count)
}

fn parse_fixed_entries(
    body: &[u8],
    entry_bytes: usize,
    data_offset: usize,
) -> Result<usize, AnalysisError> {
    let count = usize_from_u32(be_u32(body, 8)?)?;
    validate_entry_range(body, count, entry_bytes, data_offset)?;
    Ok(count)
}

fn parse_beat_grid(body: &[u8]) -> Result<Vec<AnalysisBeat>, AnalysisError> {
    let count = parse_fixed_entries(body, 8, 12)?;
    (0..count)
        .map(|index| {
            let offset = 12 + index * 8;
            let beat_number = be_u16(body, offset)?;
            if !(1..=4).contains(&beat_number) {
                return Err(AnalysisError::MalformedAnalysisFile);
            }
            Ok(AnalysisBeat {
                beat_number,
                tempo_centi_bpm: be_u16(body, offset + 2)?,
                time_millis: be_u32(body, offset + 4)?,
            })
        })
        .collect()
}

fn parse_declared_entries(
    body: &[u8],
    expected_entry_bytes: usize,
) -> Result<usize, AnalysisError> {
    let entry_bytes = usize_from_u32(be_u32(body, 0)?)?;
    let count = usize_from_u32(be_u32(body, 4)?)?;
    if entry_bytes != expected_entry_bytes {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    validate_entry_range(body, count, entry_bytes, 12)?;
    Ok(count)
}

fn parse_compact_declared_entries(
    body: &[u8],
    expected_entry_bytes: usize,
) -> Result<usize, AnalysisError> {
    let entry_bytes = usize_from_u32(be_u32(body, 0)?)?;
    let count = usize_from_u32(be_u32(body, 4)?)?;
    if entry_bytes != expected_entry_bytes {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    validate_entry_range(body, count, entry_bytes, 8)?;
    Ok(count)
}

fn parse_color_detailed_waveform(body: &[u8]) -> Result<Vec<AnalysisWaveformPoint>, AnalysisError> {
    let count = parse_declared_entries(body, 2)?;
    (0..count)
        .map(|index| {
            let value = be_u16(body, 12 + index * 2)?;
            let red = u32::from((value & 0xE000) >> 13);
            let green = u32::from((value & 0x1C00) >> 10);
            let blue = u32::from((value & 0x0380) >> 7);
            let height = u32::from((value & 0x007C) >> 2);
            let scale = |component: u32| -> Result<u8, AnalysisError> {
                u8::try_from(component * height * 255 / (7 * 31))
                    .map_err(|_| AnalysisError::MalformedAnalysisFile)
            };
            Ok(AnalysisWaveformPoint {
                low: scale(blue)?,
                mid: scale(green)?,
                high: scale(red)?,
            })
        })
        .collect()
}

fn parse_three_band_waveform(
    body: &[u8],
    data_offset: usize,
) -> Result<Vec<AnalysisWaveformPoint>, AnalysisError> {
    let count = if data_offset == 8 {
        parse_compact_declared_entries(body, 3)?
    } else {
        parse_declared_entries(body, 3)?
    };
    (0..count)
        .map(|index| {
            let offset = data_offset + index * 3;
            let bytes = body
                .get(offset..offset + 3)
                .ok_or(AnalysisError::MalformedAnalysisFile)?;
            Ok(AnalysisWaveformPoint {
                low: bytes[2],
                mid: bytes[0],
                high: bytes[1],
            })
        })
        .collect()
}

fn validate_entry_range(
    body: &[u8],
    count: usize,
    entry_bytes: usize,
    data_offset: usize,
) -> Result<(), AnalysisError> {
    let data_bytes = count
        .checked_mul(entry_bytes)
        .ok_or(AnalysisError::MalformedAnalysisFile)?;
    let end = data_offset
        .checked_add(data_bytes)
        .ok_or(AnalysisError::MalformedAnalysisFile)?;
    if end > body.len() {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    Ok(())
}

fn parse_phrases(body: &[u8]) -> Result<Vec<AnalysisPhrase>, AnalysisError> {
    if body.len() < 20 {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    let count = usize::from(be_u16(body, 4)?);
    let mut decoded = body.to_vec();
    if be_u16(body, 6)? > 20 {
        let increment = u8::try_from(count).map_err(|_| AnalysisError::MalformedAnalysisFile)?;
        let base = [
            0xCB_u8, 0xE1, 0xEE, 0xFA, 0xE5, 0xEE, 0xAD, 0xEE, 0xE9, 0xD2, 0xE9, 0xEB, 0xE1, 0xE9,
            0xF3, 0xE8, 0xE9, 0xF4, 0xE1,
        ];
        for (index, byte) in decoded[6..].iter_mut().enumerate() {
            *byte ^= base[index % base.len()].wrapping_add(increment);
        }
    }
    let mood = be_u16(&decoded, 6)?;
    if !(1..=3).contains(&mood) {
        return Err(AnalysisError::MalformedAnalysisFile);
    }
    validate_entry_range(&decoded, count, 24, 20)?;
    let final_end_beat = u32::from(be_u16(&decoded, 14)?);
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let entry_offset = 20 + index * 24;
        let beat = u32::from(be_u16(&decoded, entry_offset + 2)?);
        let kind = be_u16(&decoded, entry_offset + 4)?;
        if beat == 0 {
            return Err(AnalysisError::MalformedAnalysisFile);
        }
        entries.push((
            beat,
            kind,
            decoded[entry_offset + 6],
            decoded[entry_offset + 7],
            decoded[entry_offset + 19],
        ));
    }
    entries
        .iter()
        .enumerate()
        .map(|(index, &(beat, kind, k1, k2, k3))| {
            let next = entries
                .get(index + 1)
                .map_or(final_end_beat.saturating_add(1), |entry| entry.0);
            let start_beat = beat.saturating_sub(1);
            let end_beat = next.saturating_sub(1);
            if end_beat <= start_beat {
                return Err(AnalysisError::MalformedAnalysisFile);
            }
            Ok(AnalysisPhrase {
                start_beat,
                end_beat,
                source_label: phrase_label(mood, kind, k1, k2, k3),
            })
        })
        .collect()
}

fn phrase_label(mood: u16, kind: u16, k1: u8, k2: u8, k3: u8) -> String {
    match mood {
        1 => match kind {
            1 => format!("Intro {}", if k1 == 1 { 1 } else { 2 }),
            2 => {
                let variant = match (k2, k3) {
                    (0, 0) => 1,
                    (0, 1) => 2,
                    (1, 0) => 3,
                    _ => 1,
                };
                format!("Up {variant}")
            }
            3 => "Down".to_owned(),
            5 => format!("Chorus {}", if k1 == 1 { 1 } else { 2 }),
            6 => format!("Outro {}", if k1 == 1 { 1 } else { 2 }),
            _ => format!("High {kind}"),
        },
        2 => match kind {
            1 => "Intro".to_owned(),
            2..=7 => format!("Verse {}", kind - 1),
            8 => "Bridge".to_owned(),
            9 => "Chorus".to_owned(),
            10 => "Outro".to_owned(),
            _ => format!("Mid {kind}"),
        },
        3 => match kind {
            1 => "Intro".to_owned(),
            2..=4 => "Verse 1".to_owned(),
            5..=7 => "Verse 2".to_owned(),
            8 => "Bridge".to_owned(),
            9 => "Chorus".to_owned(),
            10 => "Outro".to_owned(),
            _ => format!("Low {kind}"),
        },
        _ => format!("Phrase {kind}"),
    }
}

fn read_bounded(path: &Path, maximum_file_bytes: u64) -> Result<Vec<u8>, AnalysisError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() < TAG_HEADER_BYTES as u64 || metadata.len() > maximum_file_bytes {
        return Err(AnalysisError::InvalidFileSize(metadata.len()));
    }
    Ok(fs::read(path)?)
}

fn normalize_location(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('\0');
    let without_scheme = trimmed
        .strip_prefix("file://localhost")
        .or_else(|| trimmed.strip_prefix("file://"))
        .unwrap_or(trimmed);
    let decoded = percent_decode(without_scheme.as_bytes())?;
    let decoded = String::from_utf8(decoded).ok()?;
    let path = PathBuf::from(&decoded);
    fs::canonicalize(&path).ok().map_or_else(
        || Some(decoded),
        |canonical| canonical.to_str().map(str::to_owned),
    )
}

fn percent_decode(value: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut index = 0_usize;
    while index < value.len() {
        if value[index] == b'%' {
            let high = *value.get(index + 1)?;
            let low = *value.get(index + 2)?;
            decoded.push(
                hex_nibble(high)?
                    .checked_mul(16)?
                    .checked_add(hex_nibble(low)?)?,
            );
            index += 3;
        } else {
            decoded.push(value[index]);
            index += 1;
        }
    }
    Some(decoded)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn canonical_existing_ancestor(path: &Path) -> Result<PathBuf, AnalysisError> {
    let mut candidate = path;
    loop {
        if candidate.exists() {
            return Ok(fs::canonicalize(candidate)?);
        }
        candidate = candidate
            .parent()
            .ok_or(AnalysisError::InvalidSnapshotRoot)?;
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn be_u16(bytes: &[u8], offset: usize) -> Result<u16, AnalysisError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(AnalysisError::MalformedAnalysisFile)?;
    Ok(u16::from_be_bytes([value[0], value[1]]))
}

fn be_u32(bytes: &[u8], offset: usize) -> Result<u32, AnalysisError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(AnalysisError::MalformedAnalysisFile)?;
    Ok(u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
}

fn usize_from_u32(value: u32) -> Result<usize, AnalysisError> {
    usize::try_from(value).map_err(|_| AnalysisError::MalformedAnalysisFile)
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("Rekordbox analysis I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("analysis root must be an existing directory")]
    InvalidAnalysisRoot,
    #[error("analysis request contains an invalid track count: {0}")]
    InvalidRequestedTrackCount(usize),
    #[error("snapshot root must be empty")]
    SnapshotRootNotEmpty,
    #[error("snapshot root cannot be inside the Rekordbox analysis root")]
    SnapshotInsideAnalysisRoot,
    #[error("snapshot root has no existing ancestor")]
    InvalidSnapshotRoot,
    #[error("analysis traversal exceeded its configured bounds")]
    TraversalLimitExceeded,
    #[error("analysis file has an invalid size: {0}")]
    InvalidFileSize(u64),
    #[error("analysis file is malformed or unsupported")]
    MalformedAnalysisFile,
    #[error("analysis source changed while its snapshot was copied")]
    SourceChangedDuringSnapshot,
    #[error("resolved analysis identity is empty")]
    InvalidResolvedIdentity,
    #[error("resolved analysis identities must be unique")]
    DuplicateResolvedIdentity,
    #[error("resolved analysis file is outside the approved root or is not ANLZ DAT")]
    ResolvedFileOutsideAnalysisRoot,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_resolved_set_is_snapshotted_without_path_matching()
    -> Result<(), Box<dyn std::error::Error>> {
        let test_root = unique_test_root("resolved")?;
        let analysis_root = test_root.join("source");
        let set_root = analysis_root.join("P001");
        let snapshot_root = test_root.join("snapshot");
        fs::create_dir_all(&set_root)?;
        let dat_path = set_root.join("ANLZ0000.DAT");
        let ext_path = set_root.join("ANLZ0000.EXT");
        fs::write(&dat_path, analysis_file(&[tag(*b"PQTZ", fixed_body(8, 8))]))?;
        fs::write(
            &ext_path,
            analysis_file(&[
                tag(*b"PWV5", declared_body(2, 32)),
                tag(*b"PSSI", phrase_body(3, false)),
            ]),
        )?;

        let request = ResolvedAnalysisRequest::try_new(
            &analysis_root,
            &snapshot_root,
            [ResolvedAnalysisTrack::try_new("83110744", &dat_path)?],
        )?;
        let result = snapshot_resolved_analysis(&request)?;

        assert_eq!(result.report.requested_tracks, 1);
        assert_eq!(result.report.matched_tracks, 1);
        assert_eq!(result.report.snapshot_files, 2);
        assert_eq!(result.report.tracks_with_beat_grid, 1);
        assert_eq!(result.report.tracks_with_phrases, 1);
        assert_eq!(result.report.tracks_with_color_waveform, 1);
        assert!(result.tracks.contains_key("83110744"));
        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn resolved_snapshot_returns_typed_beats_waveform_and_phrases()
    -> Result<(), Box<dyn std::error::Error>> {
        let test_root = unique_test_root("typed")?;
        let analysis_root = test_root.join("source");
        let set_root = analysis_root.join("P001");
        fs::create_dir_all(&set_root)?;
        let dat_path = set_root.join("ANLZ0000.DAT");
        fs::write(
            &dat_path,
            analysis_file(&[
                tag(*b"PQTZ", fixed_body(8, 8)),
                tag(*b"PWV7", declared_body(3, 16)),
                tag(*b"PSSI", phrase_body(2, false)),
            ]),
        )?;
        let request = ResolvedAnalysisRequest::try_new(
            &analysis_root,
            test_root.join("snapshot"),
            [ResolvedAnalysisTrack::try_new("42", &dat_path)?],
        )?;

        let result = snapshot_resolved_analysis_data(&request)?;
        let track = result
            .tracks
            .get("42")
            .ok_or_else(|| io::Error::other("typed track missing"))?;
        assert_eq!(track.beat_grid.len(), 8);
        assert_eq!(track.beat_grid[4].beat_number, 1);
        assert_eq!(track.waveform.len(), 16);
        assert_eq!(track.phrases[0].source_label, "Intro");
        assert_eq!(
            (track.phrases[0].start_beat, track.phrases[0].end_beat),
            (0, 1)
        );
        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn rgb_detail_is_preferred_over_three_band_for_lumi_waveforms()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut rgb = declared_body(2, 1);
        rgb[12..14].copy_from_slice(&0xE07C_u16.to_be_bytes());
        let mut three_band = declared_body(3, 1);
        three_band[12..15].copy_from_slice(&[127, 0, 0]);

        let parsed = parse_analysis_file(&analysis_file(&[
            tag(*b"PWV5", rgb),
            tag(*b"PWV7", three_band),
        ]))?;

        assert_eq!(parsed.waveform.len(), 1);
        assert_eq!(parsed.waveform[0].high, 255);
        assert_eq!(parsed.waveform[0].mid, 0);
        assert_eq!(parsed.waveform[0].low, 0);
        Ok(())
    }

    #[test]
    fn extended_hot_cues_preserve_letter_timing_name_and_rgb_over_legacy_data()
    -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_analysis_file(&analysis_file(&[
            tag(*b"PCOB", legacy_hot_cue_body(1, 1_000)),
            tag(
                *b"PCO2",
                extended_hot_cue_body(3, 32_000, Some(34_000), "DROP", [0xe6, 0x28, 0x28]),
            ),
        ]))?;

        assert_eq!(parsed.hot_cue_priority, 2);
        assert_eq!(parsed.hot_cues.len(), 1);
        assert_eq!(parsed.hot_cues[0].index, 3);
        assert_eq!(parsed.hot_cues[0].time_millis, 32_000);
        assert_eq!(parsed.hot_cues[0].loop_end_millis, Some(34_000));
        assert_eq!(parsed.hot_cues[0].comment, "DROP");
        assert_eq!(parsed.hot_cues[0].color_rgb, [0xe6, 0x28, 0x28]);
        assert_eq!(parsed.coverage().hot_cue_entries, 1);
        Ok(())
    }

    #[test]
    fn matched_set_is_copied_then_parsed_without_mutating_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let test_root = unique_test_root("matched")?;
        let analysis_root = test_root.join("source");
        let set_root = analysis_root.join("P001");
        let snapshot_root = test_root.join("snapshot");
        fs::create_dir_all(&set_root)?;
        let audio_path = test_root.join("Audio File.aiff");
        fs::write(&audio_path, b"audio")?;
        let dat_path = set_root.join("ANLZ0000.DAT");
        let ext_path = set_root.join("ANLZ0000.EXT");
        let two_ex_path = set_root.join("ANLZ0000.2EX");
        let dat = analysis_file(&[
            tag(*b"PPTH", path_body(audio_path.to_string_lossy().as_ref())?),
            tag(*b"PQTZ", fixed_body(4, 8)),
        ]);
        let ext = analysis_file(&[
            tag(*b"PWV5", declared_body(2, 16)),
            tag(*b"PSSI", phrase_body(2, true)),
        ]);
        fs::write(&dat_path, &dat)?;
        fs::write(&ext_path, &ext)?;
        fs::write(
            &two_ex_path,
            analysis_file(&[
                tag(*b"PWV6", compact_declared_body(3, 8)),
                tag(*b"PWV7", declared_body(3, 16)),
            ]),
        )?;
        let source_dat_before = fs::read(&dat_path)?;

        let request = AnalysisScanRequest::try_new(
            &analysis_root,
            &snapshot_root,
            [format!(
                "file://localhost{}",
                audio_path.to_string_lossy().replace(' ', "%20")
            )],
        )?;
        let result = scan_and_snapshot(&request)?;

        assert_eq!(result.report.matched_tracks, 1);
        assert_eq!(result.report.snapshot_files, 3);
        assert_eq!(result.report.tracks_with_beat_grid, 1);
        assert_eq!(result.report.tracks_with_phrases, 1);
        assert_eq!(result.report.tracks_with_color_waveform, 1);
        assert_eq!(result.report.tracks_with_three_band_waveform, 1);
        assert_eq!(result.report.total_beat_grid_entries, 4);
        assert_eq!(result.report.total_phrase_entries, 2);
        assert_eq!(fs::read(&dat_path)?, source_dat_before);
        fs::remove_dir_all(&test_root)?;
        Ok(())
    }

    #[test]
    fn filename_fallback_is_explicit_and_rejects_ambiguous_analysis_sets()
    -> Result<(), Box<dyn std::error::Error>> {
        let test_root = unique_test_root("provisional")?;
        let analysis_root = test_root.join("source");
        let requested_root = test_root.join("current");
        fs::create_dir_all(&requested_root)?;
        let requested = requested_root.join("Unique.aiff");
        fs::write(&requested, b"audio")?;
        write_analysis_dat(
            &analysis_root.join("A/ANLZ0000.DAT"),
            "/retired/library/Unique.aiff",
        )?;

        let disabled = AnalysisScanRequest::try_new(
            &analysis_root,
            test_root.join("disabled-snapshot"),
            [requested.to_string_lossy().into_owned()],
        )?;
        let disabled_result = scan_and_snapshot(&disabled)?;
        assert_eq!(disabled_result.report.matched_tracks, 0);

        let enabled = AnalysisScanRequest::try_new(
            &analysis_root,
            test_root.join("enabled-snapshot"),
            [requested.to_string_lossy().into_owned()],
        )?
        .allow_provisional_unique_filename_matches();
        let enabled_result = scan_and_snapshot(&enabled)?;
        assert_eq!(enabled_result.report.provisional_filename_matches, 1);

        let ambiguous_requested = requested_root.join("Ambiguous.aiff");
        fs::write(&ambiguous_requested, b"audio")?;
        write_analysis_dat(
            &analysis_root.join("B/ANLZ0000.DAT"),
            "/retired/one/Ambiguous.aiff",
        )?;
        write_analysis_dat(
            &analysis_root.join("C/ANLZ0000.DAT"),
            "/retired/two/Ambiguous.aiff",
        )?;
        let ambiguous = AnalysisScanRequest::try_new(
            &analysis_root,
            test_root.join("ambiguous-snapshot"),
            [ambiguous_requested.to_string_lossy().into_owned()],
        )?
        .allow_provisional_unique_filename_matches();
        let ambiguous_result = scan_and_snapshot(&ambiguous)?;
        assert_eq!(ambiguous_result.report.matched_tracks, 0);
        assert_eq!(ambiguous_result.report.ambiguous_filename_candidates, 1);
        fs::remove_dir_all(&test_root)?;
        Ok(())
    }

    #[test]
    fn malformed_unmatched_set_fails_closed_without_aborting_coverage()
    -> Result<(), Box<dyn std::error::Error>> {
        let test_root = unique_test_root("malformed")?;
        let analysis_root = test_root.join("source");
        let snapshot_root = test_root.join("snapshot");
        fs::create_dir_all(analysis_root.join("A"))?;
        fs::write(
            analysis_root.join("A/ANLZ0000.DAT"),
            b"not-an-analysis-file",
        )?;
        let requested = test_root.join("missing.wav");
        let request = AnalysisScanRequest::try_new(
            &analysis_root,
            &snapshot_root,
            [requested.to_string_lossy().into_owned()],
        )?;
        let result = scan_and_snapshot(&request)?;
        assert_eq!(result.report.malformed_analysis_sets, 1);
        assert_eq!(result.report.matched_tracks, 0);
        assert_eq!(result.report.missing_tracks, 1);
        fs::remove_dir_all(&test_root)?;
        Ok(())
    }

    #[test]
    fn snapshot_cannot_be_placed_below_the_production_analysis_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let test_root = unique_test_root("boundary")?;
        let analysis_root = test_root.join("source");
        fs::create_dir_all(&analysis_root)?;
        let result = AnalysisScanRequest::try_new(
            &analysis_root,
            analysis_root.join("lumi-snapshot"),
            ["/track.wav".to_owned()],
        );
        assert!(matches!(
            result,
            Err(AnalysisError::SnapshotInsideAnalysisRoot)
        ));
        fs::remove_dir_all(&test_root)?;
        Ok(())
    }

    fn unique_test_root(label: &str) -> Result<PathBuf, io::Error> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!(
            "lumi-rekordbox-analysis-{label}-{}-{nanos}",
            std::process::id()
        )))
    }

    fn analysis_file(tags: &[Vec<u8>]) -> Vec<u8> {
        let header_bytes = 28_usize;
        let total_bytes = header_bytes + tags.iter().map(Vec::len).sum::<usize>();
        let mut bytes = vec![0_u8; header_bytes];
        bytes[..4].copy_from_slice(ANALYSIS_MAGIC);
        bytes[4..8].copy_from_slice(&(header_bytes as u32).to_be_bytes());
        bytes[8..12].copy_from_slice(&(total_bytes as u32).to_be_bytes());
        for tag in tags {
            bytes.extend_from_slice(tag);
        }
        bytes
    }

    fn write_analysis_dat(path: &Path, audio_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing analysis parent"))?;
        fs::create_dir_all(parent)?;
        fs::write(
            path,
            analysis_file(&[
                tag(*b"PPTH", path_body(audio_path)?),
                tag(*b"PQTZ", fixed_body(4, 8)),
            ]),
        )?;
        Ok(())
    }

    fn tag(id: [u8; 4], body: Vec<u8>) -> Vec<u8> {
        let total_bytes = TAG_HEADER_BYTES + body.len();
        let mut bytes = Vec::with_capacity(total_bytes);
        bytes.extend_from_slice(&id);
        bytes.extend_from_slice(&(TAG_HEADER_BYTES as u32).to_be_bytes());
        bytes.extend_from_slice(&(total_bytes as u32).to_be_bytes());
        bytes.extend_from_slice(&body);
        bytes
    }

    fn path_body(path: &str) -> Result<Vec<u8>, AnalysisError> {
        let mut encoded = path.encode_utf16().collect::<Vec<_>>();
        encoded.push(0);
        let byte_count = encoded
            .len()
            .checked_mul(2)
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(AnalysisError::MalformedAnalysisFile)?;
        let mut body = Vec::new();
        body.extend_from_slice(&byte_count.to_be_bytes());
        for word in encoded {
            body.extend_from_slice(&word.to_be_bytes());
        }
        Ok(body)
    }

    fn fixed_body(count: u32, entry_bytes: usize) -> Vec<u8> {
        let mut body = vec![0_u8; 12 + count as usize * entry_bytes];
        body[8..12].copy_from_slice(&count.to_be_bytes());
        if entry_bytes == 8 {
            for index in 0..count as usize {
                let offset = 12 + index * entry_bytes;
                let beat_number = u16::try_from(index % 4 + 1).unwrap_or(1);
                body[offset..offset + 2].copy_from_slice(&beat_number.to_be_bytes());
                body[offset + 2..offset + 4].copy_from_slice(&12_800_u16.to_be_bytes());
                body[offset + 4..offset + 8].copy_from_slice(&(index as u32 * 469).to_be_bytes());
            }
        }
        body
    }

    fn declared_body(entry_bytes: u32, count: u32) -> Vec<u8> {
        let mut body = vec![0_u8; 12 + entry_bytes as usize * count as usize];
        body[..4].copy_from_slice(&entry_bytes.to_be_bytes());
        body[4..8].copy_from_slice(&count.to_be_bytes());
        body
    }

    fn compact_declared_body(entry_bytes: u32, count: u32) -> Vec<u8> {
        let mut body = vec![0_u8; 8 + entry_bytes as usize * count as usize];
        body[..4].copy_from_slice(&entry_bytes.to_be_bytes());
        body[4..8].copy_from_slice(&count.to_be_bytes());
        body
    }

    fn phrase_body(count: u16, masked: bool) -> Vec<u8> {
        let mut body = vec![0_u8; 20 + usize::from(count) * 24];
        body[..4].copy_from_slice(&24_u32.to_be_bytes());
        body[4..6].copy_from_slice(&count.to_be_bytes());
        body[6..8].copy_from_slice(&2_u16.to_be_bytes());
        body[14..16].copy_from_slice(&count.to_be_bytes());
        for index in 0..usize::from(count) {
            let offset = 20 + index * 24;
            body[offset..offset + 2].copy_from_slice(&(index as u16).to_be_bytes());
            body[offset + 2..offset + 4].copy_from_slice(&(1 + index as u16).to_be_bytes());
            body[offset + 4..offset + 6].copy_from_slice(&1_u16.to_be_bytes());
        }
        if masked {
            let increment = count as u8;
            let base = [
                0xCB_u8, 0xE1, 0xEE, 0xFA, 0xE5, 0xEE, 0xAD, 0xEE, 0xE9, 0xD2, 0xE9, 0xEB, 0xE1,
                0xE9, 0xF3, 0xE8, 0xE9, 0xF4, 0xE1,
            ];
            for (index, byte) in body[6..].iter_mut().enumerate() {
                *byte ^= base[index % base.len()].wrapping_add(increment);
            }
        }
        body
    }

    fn legacy_hot_cue_body(index: u32, time_millis: u32) -> Vec<u8> {
        let mut body = vec![0_u8; 12 + 56];
        body[..4].copy_from_slice(&1_u32.to_be_bytes());
        body[6..8].copy_from_slice(&1_u16.to_be_bytes());
        let entry = &mut body[12..];
        entry[..4].copy_from_slice(b"PCPT");
        entry[4..8].copy_from_slice(&28_u32.to_be_bytes());
        entry[8..12].copy_from_slice(&56_u32.to_be_bytes());
        entry[12..16].copy_from_slice(&index.to_be_bytes());
        entry[28] = 0;
        entry[32..36].copy_from_slice(&time_millis.to_be_bytes());
        body
    }

    fn extended_hot_cue_body(
        index: u32,
        time_millis: u32,
        loop_end_millis: Option<u32>,
        comment: &str,
        color_rgb: [u8; 3],
    ) -> Vec<u8> {
        let mut comment_bytes = comment
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>();
        comment_bytes.extend_from_slice(&[0, 0]);
        let entry_bytes = 44 + comment_bytes.len() + 4;
        let mut body = vec![0_u8; 8 + entry_bytes];
        body[..4].copy_from_slice(&1_u32.to_be_bytes());
        body[4..6].copy_from_slice(&1_u16.to_be_bytes());
        let entry = &mut body[8..];
        entry[..4].copy_from_slice(b"PCP2");
        entry[4..8].copy_from_slice(&16_u32.to_be_bytes());
        entry[8..12].copy_from_slice(&(entry_bytes as u32).to_be_bytes());
        entry[12..16].copy_from_slice(&index.to_be_bytes());
        entry[16] = if loop_end_millis.is_some() { 2 } else { 0 };
        entry[20..24].copy_from_slice(&time_millis.to_be_bytes());
        entry[24..28].copy_from_slice(&loop_end_millis.unwrap_or_default().to_be_bytes());
        entry[40..44].copy_from_slice(&(comment_bytes.len() as u32).to_be_bytes());
        entry[44..44 + comment_bytes.len()].copy_from_slice(&comment_bytes);
        let color_offset = 44 + comment_bytes.len();
        entry[color_offset] = 0x2a;
        entry[color_offset + 1..color_offset + 4].copy_from_slice(&color_rgb);
        body
    }
}
