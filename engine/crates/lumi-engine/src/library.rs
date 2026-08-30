use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumi_domain::{
    DeckId, KeyMode, PhraseKind, PitchClass, ThemeId, TrackId, TrackIdentityFacts, TrackLoadId,
    TrackMetadata, TrackPhrase,
};
use lumi_library::{
    AutoloopCatalog, AutoloopCatalogError, AutoloopResolutionReason, AutoloopVariantMove, BeatGrid,
    BeatMarker, HotCue, ImportedLibraryBaseline, ImportedPlaylist, ImportedTrackAnalysis,
    LibraryRepository, LibraryTrackQuery, LibraryTrackSort, LibraryTrackSortDirection,
    LibraryTrackSortField, LumiPhraseTimeline, PhraseInstance, PhraseLoopStrategy, PhraseRole,
    PhraseRoleCatalogError, PhraseRoleId, PhraseRoleMove, PlaylistId, RawPhraseObservation,
    ReconcileError, ReconcilePreview, ReconcileStrategy, SourceChangeClass, SourceMirrorDiff,
    SourceMirrorPlaylist, SourceMirrorSnapshot, SourceMirrorTrack, SourcePhraseMapping,
    SourcePlaylistId, SourceRevision, SourceTrackDiff, TimelineEditCommand, TimelineEditError,
    TimelineRevision, TimelineRevisionOrigin, TimelineRevisionReason, TimelineRevisionSummary,
    TrackColor, TrackPageRequest, TrackPreparationStatus, TrackSummary, TrackWorkflowFilter,
    TrackWorkflowState, VariantId, WaveformPoint, WorkflowStepDefinition, reconcile_timeline,
};
use lumi_library_demo::{DemoLibraryError, DemoLibraryRevision, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider as _;
use lumi_library_sqlite::{
    DeviceAliasUpsert, DeviceAnalysisUpsert, DeviceHotCueUpsert, DeviceMatchCandidate,
    DevicePlaylistUpsert, DeviceTrackImport, LibraryResetImpact,
};
use lumi_library_sqlite::{SqliteLibraryError, SqliteLibraryRepository};
use lumi_light_plans::{
    Candidate as LightPlanCandidate, CompiledLightPlan, LightPlanError, LightPlanningPolicy,
    PhraseRequest as LightPlanPhraseRequest, PhraseSelection as LightPlanPhraseSelection,
    VariationHistory,
};
use lumi_planner::{PlannerTrack, PlanningInput, ThemeSelectionContext};
use lumi_rekordbox_analysis::{
    AnalysisError, AnalysisWaveformPoint, ResolvedAnalysisRequest, ResolvedAnalysisTrack,
    ResolvedTrackAnalysis, snapshot_resolved_analysis_data,
};
use lumi_rekordbox_device::{
    DeviceError, DeviceLibrarySnapshot, DeviceTrack, REKORDBOX_TRACK_COLORS,
    audio_content_signature, read_device_library,
};
use lumi_rekordbox_resolver::{
    DatabaseKey, RequestedTrack, ResolverError, SqlCipherResolver, create_database_snapshot,
};
use lumi_rekordbox_xml::{
    RekordboxXmlError, RekordboxXmlMirrorSnapshot, RekordboxXmlSyncRequest, load_latest_mirror,
};
use serde_json::{Value, json};
use thiserror::Error;

use crate::autoloop_defaults::{AutoloopDefaultsError, seeded_autoloop_catalog};
use crate::phrase_role_defaults::{
    PhraseRoleDefaultsError, provider_display_name, seeded_phrase_role_catalog,
};

const DEFAULT_PAGE_LIMIT: u16 = 50;
const REKORDBOX_XML_SOURCE_ID: &str = "rekordbox-xml-local";
const REKORDBOX_XML_SOURCE_KIND: &str = "rekordbox-xml";
const REKORDBOX_CANONICAL_SOURCE_ID: &str = "rekordbox7-local";
const REKORDBOX_CANONICAL_SOURCE_KIND: &str = "rekordbox7";
const MAX_IMPORTED_WAVEFORM_POINTS: usize = 16_384;
const MAX_DECK_WAVEFORM_PREVIEW_POINTS: usize = 1_024;
const MAX_DECK_WAVEFORM_DETAIL_POINTS: usize = 16_384;

pub(crate) fn library_sort_field_name(field: LibraryTrackSortField) -> &'static str {
    match field {
        LibraryTrackSortField::Playlist => "playlist",
        LibraryTrackSortField::Title => "title",
        LibraryTrackSortField::Artist => "artist",
        LibraryTrackSortField::Bpm => "bpm",
        LibraryTrackSortField::Key => "key",
        LibraryTrackSortField::Duration => "duration",
        LibraryTrackSortField::UsbSources => "usbSources",
        LibraryTrackSortField::TimelineRevision => "timelineRevision",
        LibraryTrackSortField::Readiness => "readiness",
        LibraryTrackSortField::PreparationStatus => "preparationStatus",
        LibraryTrackSortField::Attention => "attention",
        LibraryTrackSortField::SourceTrackId => "sourceTrackID",
        LibraryTrackSortField::AnalysisRevision => "analysisRevision",
    }
}

pub(crate) fn library_sort_direction_name(direction: LibraryTrackSortDirection) -> &'static str {
    match direction {
        LibraryTrackSortDirection::Ascending => "ascending",
        LibraryTrackSortDirection::Descending => "descending",
    }
}

pub struct LibraryWorker {
    repository: SqliteLibraryRepository,
    database_path: Option<std::path::PathBuf>,
    source_id: String,
    source_kind: String,
    source_name: String,
    source_revision: String,
    search: String,
    playlist_id: Option<PlaylistId>,
    workflow_filter: Option<TrackWorkflowFilter>,
    workflow_step_id: Option<String>,
    offset: u32,
    limit: u16,
    sort: LibraryTrackSort,
    editor_track_id: Option<TrackId>,
    pending_source_refresh: Option<ImportedLibraryBaseline>,
    pending_rekordbox_preview: Option<RekordboxXmlMirrorSnapshot>,
    pending_rekordbox_diff: Option<SourceMirrorDiff>,
    last_rekordbox_apply: Option<SourceMirrorDiff>,
    pending_device_inspection: Option<DeviceInspection>,
    device_review_comparisons_by_source: BTreeMap<String, BTreeMap<u32, DeviceReviewComparison>>,
    pending_library_reset: Option<LibraryResetPreviewState>,
    pending_light_plan_preview: Option<Value>,
}

pub(crate) struct LibraryQueryUpdate {
    pub search: String,
    pub playlist_id: Option<u64>,
    pub workflow_filter: Option<TrackWorkflowFilter>,
    pub workflow_step_id: Option<String>,
    pub offset: u32,
    pub limit: u16,
    pub sort: LibraryTrackSort,
}

#[derive(Clone, Debug)]
struct LibraryResetPreviewState {
    token: String,
    preserve_track_ids: Vec<TrackId>,
    impact: LibraryResetImpact,
    authored_heads: Vec<(TrackId, u64)>,
}

#[derive(Clone, Debug)]
struct DeviceInspection {
    snapshot: DeviceLibrarySnapshot,
    selected_playlist_ids: Vec<u32>,
    tracks: BTreeMap<u32, DeviceInspectionTrack>,
    review_comparisons: BTreeMap<u32, DeviceReviewComparison>,
}

#[derive(Clone, Debug)]
struct DeviceInspectionTrack {
    status: &'static str,
    detail: String,
}

#[derive(Clone, Debug)]
struct DeviceReviewComparison {
    beat_grid_changed: bool,
    hot_cues_changed: bool,
    file_data_changed: bool,
    raw_phrases_changed: bool,
    waveform_changed: bool,
    beat_grid_detail: String,
    hot_cues_detail: String,
    raw_phrases_detail: String,
    waveform_detail: String,
    file_detail: String,
}

#[derive(Clone, Debug)]
pub struct LibraryLocalPlaybackTrack {
    metadata: TrackMetadata,
    context: LibraryPlanContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RekordboxDeviceSyncResult {
    pub source_id: String,
    pub display_name: String,
    pub database_revision: String,
    pub tracks: usize,
    pub matched: usize,
    pub unmatched: usize,
    pub refreshed_analyses: usize,
    pub protected_older: usize,
    pub held_conflicts: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceReviewChoice {
    KeepLumi,
    UseUsb,
}

#[derive(Clone, Debug)]
pub struct ConnectedLibraryTrack {
    pub prepared: LibraryLocalPlaybackTrack,
}

impl LibraryLocalPlaybackTrack {
    #[must_use]
    pub fn into_parts(self) -> (TrackMetadata, LibraryPlanContext) {
        (self.metadata, self.context)
    }
}

#[derive(Clone, Debug)]
pub struct LibraryPlanContext {
    provider_kind: String,
    source_id: String,
    source_name: String,
    source_track_id: String,
    analysis_revision: String,
    timeline_revision: u64,
    audio_uri: String,
    duration_millis: u64,
    beat_grid: lumi_library::BeatGrid,
    waveform: Vec<lumi_library::WaveformPoint>,
    hot_cues: Vec<lumi_library::HotCue>,
    track_color: Option<TrackColor>,
    catalog: AutoloopCatalog,
    phrases: Vec<LibraryPhrasePlanContext>,
    autoloop_overrides: BTreeMap<u16, VariantId>,
}

#[derive(Clone, Debug)]
struct LibraryPhrasePlanContext {
    phrase_index: u16,
    start_beat: u32,
    end_beat: u32,
    role_id: PhraseRoleId,
    role_name: String,
    strategy: PhraseLoopStrategy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLibraryCue {
    pub phrase_index: u16,
    pub role_id: String,
    pub role_name: String,
    pub strategy: &'static str,
    pub variant_id: String,
    pub entry_id: String,
    pub entry_name: String,
    pub bank_number: u64,
    pub autoloop_number: Option<u16>,
    pub catalog_revision: u64,
    pub resolution_reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalPlaybackClockAnchor {
    pub bpm_milli: u32,
    pub beat_index: u32,
    pub song_position_16th: u16,
    pub delay_to_next_tick: Duration,
}

impl LibraryPlanContext {
    #[must_use]
    pub const fn catalog_revision(&self) -> u64 {
        self.catalog.revision()
    }

    /// Returns every Theme that can safely start the track. Policy filtering
    /// happens before coverage ranking so an ineligible complete Theme cannot
    /// hide an eligible Theme with one optional mapping gap.
    #[must_use]
    pub fn startable_themes(&self) -> Vec<(ThemeId, String)> {
        self.catalog
            .themes()
            .iter()
            .filter_map(|theme| {
                let can_start = self
                    .phrases
                    .first()
                    .is_some_and(|phrase| self.resolve_phrase(theme.id(), phrase).is_ok());
                if !can_start {
                    return None;
                }
                Some((theme.id(), theme.display_name().to_owned()))
            })
            .collect()
    }

    /// Keeps only the supplied Themes with the best exact phrase coverage.
    /// Complete mappings win; when every eligible Theme has an optional gap,
    /// the most complete eligible Theme remains plannable.
    #[must_use]
    pub fn best_covered_themes(&self, themes: Vec<(ThemeId, String)>) -> Vec<(ThemeId, String)> {
        let coverage = themes
            .into_iter()
            .map(|(theme_id, name)| {
                let mapped = self
                    .phrases
                    .iter()
                    .filter(|phrase| self.resolve_phrase(theme_id, phrase).is_ok())
                    .count();
                (theme_id, name, mapped)
            })
            .collect::<Vec<_>>();
        let Some(maximum) = coverage.iter().map(|(_, _, mapped)| *mapped).max() else {
            return Vec::new();
        };
        coverage
            .into_iter()
            .filter(|(_, _, mapped)| *mapped == maximum)
            .map(|(id, name, _)| (id, name))
            .collect()
    }

    /// Applies hard Theme eligibility first and coverage ranking second. Keep
    /// this order centralized: an excluded complete Theme must never mask a
    /// slightly less complete but safe Theme.
    #[must_use]
    pub fn eligible_best_covered_themes(
        &self,
        policy: &LightPlanningPolicy,
    ) -> Vec<(ThemeId, String)> {
        let eligible = crate::session::policy_eligible_executable_themes(
            self.startable_themes(),
            self.track_color_rgb(),
            policy,
        );
        self.best_covered_themes(eligible)
    }

    #[must_use]
    pub const fn track_color_rgb(&self) -> Option<u32> {
        match self.track_color {
            Some(color) => Some(color.rgb_u32()),
            None => None,
        }
    }

    #[must_use]
    pub fn missing_role_names(&self, theme_id: ThemeId) -> Vec<String> {
        let mut missing = Vec::new();
        for phrase in &self.phrases {
            if missing.contains(&phrase.role_name) {
                continue;
            }
            if self.resolve_phrase(theme_id, phrase).is_err() {
                missing.push(phrase.role_name.clone());
            }
        }
        missing
    }

    #[must_use]
    pub fn theme_coverage(&self) -> Vec<(ThemeId, String, Vec<String>)> {
        self.catalog
            .themes()
            .iter()
            .map(|theme| {
                (
                    theme.id(),
                    theme.display_name().to_owned(),
                    self.missing_role_names(theme.id()),
                )
            })
            .collect()
    }

    #[must_use]
    pub fn identity_json(&self) -> Value {
        json!({
            "matchStatus": "exact",
            "providerKind": self.provider_kind,
            "sourceId": self.source_id,
            "sourceName": self.source_name,
            "sourceTrackId": self.source_track_id,
            "analysisRevision": self.analysis_revision,
            "timelineRevision": self.timeline_revision,
        })
    }

    #[must_use]
    pub fn local_playback_json(&self) -> Value {
        json!({
            "audioUri": self.audio_uri,
            "durationMillis": self.duration_millis,
        })
    }

    #[must_use]
    pub fn waveform_preview_json(&self) -> Value {
        let points = deck_waveform_preview_points(&self.waveform, MAX_DECK_WAVEFORM_PREVIEW_POINTS);
        json!({
            "source": "localLibrary",
            "style": "rgb",
            "points": points.iter().map(|point| json!({
                "low": point[0] / 8,
                "mid": point[1] / 8,
                "high": point[2] / 8,
            })).collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn beat_grid_json(&self) -> Value {
        json!({
            "beatsPerBar": self.beat_grid.beats_per_bar(),
            "durationMillis": self.duration_millis,
            "timesMillis": self.beat_grid.markers().iter()
                .map(|marker| marker.time_millis())
                .collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn hot_cues_json(&self) -> Value {
        json!(
            self.hot_cues
                .iter()
                .map(|cue| json!({
                    "index": cue.index(),
                    "timeMillis": cue.time_millis(),
                    "loopEndMillis": cue.loop_end_millis(),
                    "name": cue.name(),
                    "colorRgb": cue.color_rgb(),
                }))
                .collect::<Vec<_>>()
        )
    }

    #[must_use]
    pub fn beat_at_millis(&self, position_millis: u64) -> u32 {
        let index = self
            .beat_grid
            .markers()
            .partition_point(|marker| marker.time_millis() <= position_millis);
        u32::try_from(index.saturating_sub(1)).unwrap_or(u32::MAX)
    }

    #[must_use]
    #[cfg_attr(test, allow(dead_code))]
    pub fn is_hot_cue_beat(&self, beat: u32) -> bool {
        self.hot_cues
            .iter()
            .any(|cue| self.beat_at_millis(cue.time_millis()).abs_diff(beat) <= 1)
    }

    #[must_use]
    pub fn millis_at_beat(&self, beat: u32) -> Option<u64> {
        self.beat_grid
            .markers()
            .get(usize::try_from(beat).ok()?)
            .map(|marker| marker.time_millis())
    }

    #[must_use]
    pub fn clock_anchor_at_millis(
        &self,
        position_millis: u64,
        fallback_bpm_milli: u32,
    ) -> Option<LocalPlaybackClockAnchor> {
        let markers = self.beat_grid.markers();
        let first = markers.first()?;
        let insertion = markers.partition_point(|marker| marker.time_millis() <= position_millis);
        let current_index = insertion.saturating_sub(1);
        let current = markers.get(current_index).unwrap_or(first);
        let song_position = u16::try_from(current.beat_index().checked_mul(4)?).ok()?;

        let last_index = markers.len().saturating_sub(1);
        let window_start = current_index.saturating_sub(8);
        let window_end = current_index.saturating_add(8).min(last_index);
        let bpm_milli = if window_end > window_start {
            let elapsed = markers[window_end]
                .time_millis()
                .saturating_sub(markers[window_start].time_millis());
            let beat_count = u64::try_from(window_end - window_start).ok()?;
            if let Some(measured_bpm) = beat_count.saturating_mul(60_000_000).checked_div(elapsed) {
                u32::try_from(measured_bpm)
                    .ok()
                    .filter(|bpm| (20_000..=300_000).contains(bpm))
                    .unwrap_or(fallback_bpm_milli)
            } else {
                fallback_bpm_milli
            }
        } else {
            fallback_bpm_milli
        };

        let delay_to_next_tick = if position_millis < first.time_millis() {
            Duration::from_micros(
                first
                    .time_millis()
                    .saturating_sub(position_millis)
                    .saturating_mul(1_000),
            )
        } else if position_millis == current.time_millis() {
            Duration::ZERO
        } else if let Some(next) = markers.get(current_index.saturating_add(1)) {
            let beat_micros = next
                .time_millis()
                .saturating_sub(current.time_millis())
                .saturating_mul(1_000);
            let elapsed_micros = position_millis
                .saturating_sub(current.time_millis())
                .saturating_mul(1_000)
                .min(beat_micros);
            let elapsed_ticks = elapsed_micros.saturating_mul(24) / beat_micros.max(1);
            let next_tick_offset = elapsed_ticks.saturating_add(1).saturating_mul(beat_micros) / 24;
            Duration::from_micros(next_tick_offset.saturating_sub(elapsed_micros))
        } else {
            Duration::from_nanos(60_000_000_000_000_u64 / (u64::from(bpm_milli).saturating_mul(24)))
        };

        Some(LocalPlaybackClockAnchor {
            bpm_milli,
            beat_index: current.beat_index(),
            song_position_16th: song_position,
            delay_to_next_tick,
        })
    }

    #[must_use]
    pub fn phrase_role_json(&self, phrase_index: u16) -> Value {
        self.phrases
            .iter()
            .find(|phrase| phrase.phrase_index == phrase_index)
            .map_or(Value::Null, |phrase| {
                json!({
                    "roleId": phrase.role_id.as_str(),
                    "roleName": phrase.role_name,
                })
            })
    }

    #[cfg(test)]
    pub fn resolve(
        &self,
        theme_id: ThemeId,
    ) -> Result<Vec<ResolvedLibraryCue>, AutoloopCatalogError> {
        self.phrases
            .iter()
            .map(|phrase| {
                let override_variant = self.autoloop_overrides.get(&phrase.phrase_index);
                let resolution = self.resolve_phrase(theme_id, phrase)?;
                Ok(ResolvedLibraryCue {
                    phrase_index: phrase.phrase_index,
                    role_id: phrase.role_id.as_str().to_owned(),
                    role_name: phrase.role_name.clone(),
                    strategy: if override_variant.is_some() {
                        "planOverride"
                    } else {
                        loop_strategy_name(&phrase.strategy)
                    },
                    variant_id: resolution.variant_id().as_str().to_owned(),
                    entry_id: resolution.entry_id().as_str().to_owned(),
                    entry_name: resolution.display_name().to_owned(),
                    bank_number: theme_id.value(),
                    autoloop_number: resolution
                        .variant_id()
                        .as_str()
                        .strip_prefix("mapping-")
                        .and_then(|value| value.parse::<u16>().ok()),
                    catalog_revision: resolution.catalog_revision(),
                    resolution_reason: autoloop_resolution_reason_name(resolution.reason()),
                })
            })
            .collect()
    }

    fn resolve_phrase(
        &self,
        theme_id: ThemeId,
        phrase: &LibraryPhrasePlanContext,
    ) -> Result<lumi_library::AutoloopResolution, AutoloopCatalogError> {
        if let Some(variant_id) = self.autoloop_overrides.get(&phrase.phrase_index) {
            self.catalog.resolve(
                theme_id,
                &phrase.role_id,
                Some(variant_id),
                self.catalog.revision(),
            )
        } else {
            self.catalog.resolve_loop_strategy(
                theme_id,
                &phrase.role_id,
                &phrase.strategy,
                self.catalog.revision(),
            )
        }
    }

    /// Compiles all automatic variation before playback. The returned values are
    /// immutable physical AutoLoop addresses; the realtime executor never sees
    /// the policy, history or weighted-selection algorithm.
    pub fn compile_light_plan(
        &self,
        theme_id: ThemeId,
        policy: &LightPlanningPolicy,
        variation_seed: u64,
        history: &VariationHistory,
    ) -> Result<CompiledLightPlan, LightPlanError> {
        self.compile_light_plan_for_phrases(theme_id, &[], policy, variation_seed, history)
    }

    /// Compiles only the phrases that actually use `theme_id` in a mixed live
    /// plan. An empty selection means the complete track and preserves the
    /// original pre-playback compilation contract.
    pub fn compile_light_plan_for_phrases(
        &self,
        theme_id: ThemeId,
        phrase_indices: &[u16],
        policy: &LightPlanningPolicy,
        variation_seed: u64,
        history: &VariationHistory,
    ) -> Result<CompiledLightPlan, LightPlanError> {
        let phrases = self
            .phrases
            .iter()
            .filter(|phrase| {
                phrase_indices.is_empty() || phrase_indices.contains(&phrase.phrase_index)
            })
            .map(|phrase| {
                let selection =
                    if let Some(variant) = self.autoloop_overrides.get(&phrase.phrase_index) {
                        LightPlanPhraseSelection::PlanOverride(variant.as_str().to_owned())
                    } else {
                        match &phrase.strategy {
                            PhraseLoopStrategy::Auto => LightPlanPhraseSelection::Automatic,
                            PhraseLoopStrategy::FixedVariant(variant) => {
                                LightPlanPhraseSelection::FixedVariant(variant.as_str().to_owned())
                            }
                            PhraseLoopStrategy::ThemeSpecificExact(overrides) => overrides
                                .iter()
                                .find(|value| value.theme_id() == theme_id)
                                .map_or(LightPlanPhraseSelection::Automatic, |value| {
                                    LightPlanPhraseSelection::FixedVariant(
                                        value.variant_id().as_str().to_owned(),
                                    )
                                }),
                        }
                    };
                LightPlanPhraseRequest {
                    phrase_index: phrase.phrase_index,
                    role_id: phrase.role_id.as_str().to_owned(),
                    selection,
                }
            })
            .collect::<Vec<_>>();
        let candidates = self
            .catalog
            .cells()
            .iter()
            .filter_map(|cell| {
                Some(LightPlanCandidate {
                    theme_id: cell.theme_id().value(),
                    role_id: cell.role_id().as_str().to_owned(),
                    variant_id: cell.variant_id().as_str().to_owned(),
                    entry_id: cell.entry_id().as_str().to_owned(),
                    display_name: cell.display_name().to_owned(),
                    autoloop_number: mapping_number(cell.variant_id())?,
                })
            })
            .collect::<Vec<_>>();
        lumi_light_plans::compile(
            policy,
            theme_id.value(),
            self.track_color.map(TrackColor::rgb_u32),
            variation_seed,
            &phrases,
            &candidates,
            history,
        )
    }

    /// Resolves the configured selection for one phrase only. Missing coverage
    /// is represented as `None` so a sparse Theme safely keeps the current
    /// SoundSwitch AutoLoop running instead of invalidating the whole plan.
    #[must_use]
    pub fn resolved_autoloop(
        &self,
        theme_id: ThemeId,
        phrase_index: u16,
    ) -> Option<ResolvedLibraryCue> {
        let phrase = self
            .phrases
            .iter()
            .find(|phrase| phrase.phrase_index == phrase_index)?;
        let override_variant = self.autoloop_overrides.get(&phrase_index);
        let resolution = self.resolve_phrase(theme_id, phrase).ok()?;
        Some(ResolvedLibraryCue {
            phrase_index,
            role_id: phrase.role_id.as_str().to_owned(),
            role_name: phrase.role_name.clone(),
            strategy: if override_variant.is_some() {
                "planOverride"
            } else {
                loop_strategy_name(&phrase.strategy)
            },
            variant_id: resolution.variant_id().as_str().to_owned(),
            entry_id: resolution.entry_id().as_str().to_owned(),
            entry_name: resolution.display_name().to_owned(),
            bank_number: theme_id.value(),
            autoloop_number: mapping_number(resolution.variant_id()),
            catalog_revision: resolution.catalog_revision(),
            resolution_reason: autoloop_resolution_reason_name(resolution.reason()),
        })
    }

    pub fn autoloop_choices(
        &self,
        theme_id: ThemeId,
        phrase_index: u16,
    ) -> Vec<ResolvedLibraryCue> {
        let Some(phrase) = self
            .phrases
            .iter()
            .find(|phrase| phrase.phrase_index == phrase_index)
        else {
            return Vec::new();
        };
        let mut choices = self
            .catalog
            .cells()
            .iter()
            .filter(|cell| cell.theme_id() == theme_id && cell.role_id() == &phrase.role_id)
            .filter_map(|cell| {
                let autoloop_number = mapping_number(cell.variant_id())?;
                Some(ResolvedLibraryCue {
                    phrase_index,
                    role_id: phrase.role_id.as_str().to_owned(),
                    role_name: phrase.role_name.clone(),
                    strategy: if self.autoloop_overrides.contains_key(&phrase_index) {
                        "planOverride"
                    } else {
                        loop_strategy_name(&phrase.strategy)
                    },
                    variant_id: cell.variant_id().as_str().to_owned(),
                    entry_id: cell.entry_id().as_str().to_owned(),
                    entry_name: cell.display_name().to_owned(),
                    bank_number: theme_id.value(),
                    autoloop_number: Some(autoloop_number),
                    catalog_revision: self.catalog.revision(),
                    resolution_reason: if self.autoloop_overrides.contains_key(&phrase_index) {
                        "planOverride"
                    } else {
                        loop_strategy_name(&phrase.strategy)
                    }
                    .to_owned(),
                })
            })
            .collect::<Vec<_>>();
        choices.sort_by_key(|choice| choice.autoloop_number);
        choices
    }

    pub fn set_autoloop_override(
        &mut self,
        theme_id: ThemeId,
        phrase_index: u16,
        autoloop_number: u16,
    ) -> Result<(), AutoloopCatalogError> {
        let Some(phrase) = self
            .phrases
            .iter()
            .find(|phrase| phrase.phrase_index == phrase_index)
        else {
            return Err(AutoloopCatalogError::UnknownPhraseRole);
        };
        let variant_id = self
            .catalog
            .cells()
            .iter()
            .find(|cell| {
                cell.theme_id() == theme_id
                    && cell.role_id() == &phrase.role_id
                    && mapping_number(cell.variant_id()) == Some(autoloop_number)
            })
            .map(|cell| cell.variant_id().clone())
            .ok_or(AutoloopCatalogError::MissingExactCell)?;
        self.autoloop_overrides.insert(phrase_index, variant_id);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn remap_phrase_for_test(
        &mut self,
        theme_id: ThemeId,
        phrase_index: u16,
        button_number: u16,
    ) -> Result<(), AutoloopCatalogError> {
        let role_id = self
            .phrases
            .iter()
            .find(|phrase| phrase.phrase_index == phrase_index)
            .map(|phrase| phrase.role_id.clone())
            .ok_or(AutoloopCatalogError::UnknownPhraseRole)?;
        let mappings = self
            .catalog
            .cells()
            .iter()
            .filter(|cell| cell.theme_id() == theme_id && cell.role_id() == &role_id)
            .map(|cell| cell.variant_id().clone())
            .collect::<Vec<_>>();
        let mut catalog = self.catalog.clone();
        for mapping in mappings {
            catalog = catalog.clear_mapping(theme_id, &mapping)?;
        }
        catalog = catalog.set_mapping(
            theme_id,
            VariantId::try_new(format!("mapping-{button_number}"))
                .map_err(|_| AutoloopCatalogError::IdentifierOverflow)?,
            role_id,
            Some(format!("Regression AutoLoop {button_number}")),
        )?;
        self.catalog = catalog;
        Ok(())
    }
}

fn deck_waveform_preview_points(points: &[WaveformPoint], maximum: usize) -> Vec<[u8; 3]> {
    if points.is_empty() || maximum == 0 {
        return Vec::new();
    }
    let chunk_size = points.len().div_ceil(maximum);
    points
        .chunks(chunk_size)
        .map(|chunk| {
            chunk.iter().fold([0_u8; 3], |peak, point| {
                [
                    peak[0].max(point.low()),
                    peak[1].max(point.mid()),
                    peak[2].max(point.high()),
                ]
            })
        })
        .collect()
}

fn mapping_number(variant_id: &VariantId) -> Option<u16> {
    variant_id
        .as_str()
        .strip_prefix("mapping-")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| (1..=32).contains(value))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhraseRoleCatalogMutation {
    Add {
        display_name: String,
    },
    Rename {
        role_id: PhraseRoleId,
        display_name: String,
    },
    Move {
        role_id: PhraseRoleId,
        direction: PhraseRoleMove,
    },
    SetArchived {
        role_id: PhraseRoleId,
        archived: bool,
    },
    SetColor {
        role_id: PhraseRoleId,
        color_rgb: u32,
    },
    SetSourceMapping {
        provider_kind: String,
        raw_label: String,
        role_id: PhraseRoleId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AutoloopCatalogMutation {
    RenameTheme {
        theme_id: ThemeId,
        display_name: String,
    },
    AddVariant {
        role_id: PhraseRoleId,
        display_name: String,
    },
    RenameVariant {
        role_id: PhraseRoleId,
        variant_id: VariantId,
        display_name: String,
    },
    MoveVariant {
        role_id: PhraseRoleId,
        variant_id: VariantId,
        direction: AutoloopVariantMove,
    },
    SetVariantArchived {
        role_id: PhraseRoleId,
        variant_id: VariantId,
        archived: bool,
    },
    SetCell {
        theme_id: ThemeId,
        role_id: PhraseRoleId,
        variant_id: VariantId,
        display_name: Option<String>,
    },
    SetButton {
        theme_id: ThemeId,
        button_number: u16,
        role_id: PhraseRoleId,
        display_name: Option<String>,
    },
    ClearButton {
        theme_id: ThemeId,
        button_number: u16,
    },
}

impl LibraryWorker {
    pub fn demo() -> Result<Self, LibraryWorkerError> {
        let database_path = crate::service::configured_database_path()
            .map_err(|error| LibraryWorkerError::Configuration(error.to_string()))?;
        let repository = match database_path.as_ref() {
            Some(path) => SqliteLibraryRepository::open(path)?,
            None => SqliteLibraryRepository::in_memory()?,
        };
        Self::demo_with_repository(repository, database_path)
    }

    pub fn autoloop_catalog(&self) -> Result<AutoloopCatalog, LibraryWorkerError> {
        Ok(self.repository.autoloop_catalog()?)
    }

    pub fn light_planning_policy(&self) -> Result<LightPlanningPolicy, LibraryWorkerError> {
        Ok(self.repository.light_planning_policy()?)
    }

    pub fn replace_light_planning_policy(
        &mut self,
        expected_revision: u64,
        policy: LightPlanningPolicy,
    ) -> Result<LightPlanningPolicy, LibraryWorkerError> {
        Ok(self
            .repository
            .replace_light_planning_policy(expected_revision, policy)?)
    }

    /// Builds an inspectable Light Plan through the exact compiler used by
    /// playback. This transient result never enters an output or timing lane.
    pub fn preview_light_plan(
        &mut self,
        track_id: u64,
        expected_timeline_revision: u64,
        theme_id: Option<u64>,
        variation_seed: u64,
        policy: &LightPlanningPolicy,
    ) -> Result<(), LibraryWorkerError> {
        let prepared = self.local_playback_track(track_id, expected_timeline_revision)?;
        let (metadata, context) = prepared.into_parts();
        let (theme_id, theme_reason) = if let Some(theme_id) = theme_id {
            (theme_id, "manualPreviewOverride")
        } else {
            let themes = context.eligible_best_covered_themes(policy);
            let planner = crate::session::planner_for_executable_themes(
                context.catalog_revision(),
                themes,
                policy,
            )
            .map_err(|error| LibraryWorkerError::Configuration(error.to_string()))?
            .ok_or_else(|| {
                LibraryWorkerError::Configuration(crate::session::library_plan_hold_reason(
                    &context, policy,
                ))
            })?;
            let planning_input = PlanningInput {
                deck_id: DeckId::new(1),
                track_load_id: TrackLoadId::new(track_id),
                track: PlannerTrack::analyzed(&metadata),
            };
            let plan = planner
                .generate_with_context(&planning_input, &ThemeSelectionContext::default())
                .map_err(|error| LibraryWorkerError::Configuration(error.to_string()))?;
            let decision = plan.theme_decision().ok_or_else(|| {
                LibraryWorkerError::Configuration(
                    "automatic Light Plan preview has no Theme decision".to_owned(),
                )
            })?;
            (
                decision.theme_id().value(),
                crate::session::theme_selection_reason_name(decision.reason()),
            )
        };
        let compiled = context.compile_light_plan(
            ThemeId::new(theme_id),
            policy,
            variation_seed,
            &VariationHistory::default(),
        )?;
        self.pending_light_plan_preview = Some(json!({
            "trackId": track_id,
            "trackTitle": metadata.title(),
            "themeId": theme_id,
            "themeReason": theme_reason,
            "policyRevision": compiled.policy_revision,
            "variationSeed": compiled.variation_seed.to_string(),
            "signature": compiled.signature.to_string(),
            "phrases": compiled.choices.iter().filter_map(|choice| {
                let phrase = context.phrases.iter()
                    .find(|phrase| phrase.phrase_index == choice.phrase_index)?;
                Some(json!({
                    "phraseIndex": choice.phrase_index,
                    "startBeat": phrase.start_beat,
                    "endBeat": phrase.end_beat,
                    "roleId": choice.role_id,
                    "roleName": phrase.role_name,
                    "variantId": choice.variant_id,
                    "entryId": choice.entry_id,
                    "autoloopName": choice.display_name,
                    "autoloopNumber": choice.autoloop_number,
                    "reason": choice.evidence.reason,
                    "effectiveWeight": choice.evidence.effective_weight,
                    "colorInfluence": choice.evidence.color_influence,
                    "repeatProtection": choice.evidence.repeat_protection,
                    "modifiers": compiled.modifier_choices.iter()
                        .filter(|modifier| modifier.phrase_index == choice.phrase_index)
                        .map(|modifier| json!({
                            "id": modifier.modifier_id,
                            "name": modifier.display_name,
                            "kind": modifier.kind,
                            "scope": modifier.scope,
                            "providerKind": modifier.provider_kind,
                            "midiChannel": modifier.midi_channel,
                            "midiNote": modifier.midi_note,
                            "reason": modifier.evidence.reason,
                            "effectiveWeight": modifier.evidence.effective_weight,
                            "colorInfluence": modifier.evidence.color_influence,
                            "repeatProtection": modifier.evidence.repeat_protection,
                        })).collect::<Vec<_>>(),
                }))
            }).collect::<Vec<_>>(),
            "availableModifiers": policy.modifiers.iter().map(|modifier| json!({
                "id": modifier.id,
                "name": modifier.display_name,
                "kind": modifier.kind,
                "automaticExecutionReady": modifier.automatic_execution_ready(),
                "execution": if modifier.automatic_execution_ready() { "eligible" } else { "pocRequired" },
            })).collect::<Vec<_>>(),
        }));
        Ok(())
    }

    #[cfg(test)]
    fn demo_at(path: &std::path::Path) -> Result<Self, LibraryWorkerError> {
        Self::demo_with_repository(
            SqliteLibraryRepository::open(path)?,
            Some(path.to_path_buf()),
        )
    }

    fn demo_with_repository(
        mut repository: SqliteLibraryRepository,
        database_path: Option<std::path::PathBuf>,
    ) -> Result<Self, LibraryWorkerError> {
        let provider = DemoLibrarySourceProvider::curated();
        let baseline = provider.load_baseline()?;
        if repository
            .page_tracks(TrackPageRequest::try_new(0, 1)?)?
            .total()
            == 0
            && !repository.suppress_demo_seed()?
        {
            repository.import_baseline(&baseline)?;
        }
        let persisted_source = match repository.library_source(
            &lumi_library::LibrarySourceId::try_new(REKORDBOX_CANONICAL_SOURCE_ID)?,
        )? {
            Some(source) => source,
            None => {
                let latest_baseline =
                    DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2)
                        .load_baseline()?;
                let persisted_before_recovery = repository
                    .library_source(baseline.source_id())?
                    .ok_or(LibraryWorkerError::MissingLibrarySource)?;
                match repository.complete_source_refresh(&latest_baseline) {
                    Ok(()) => {}
                    Err(SqliteLibraryError::IncompleteSourceRefresh(_))
                        if persisted_before_recovery.revision()
                            == latest_baseline.source_revision() =>
                    {
                        repository.restore_source_checkpoint(&baseline)?;
                    }
                    Err(SqliteLibraryError::IncompleteSourceRefresh(_)) => {}
                    Err(error) => return Err(error.into()),
                }
                repository
                    .library_source(baseline.source_id())?
                    .ok_or(LibraryWorkerError::MissingLibrarySource)?
            }
        };
        let phrase_mapping_defaults_upgraded = seed_default_role_catalog(&mut repository)?;
        seed_default_autoloop_catalog(&mut repository)?;
        let mut worker = Self {
            repository,
            database_path,
            source_id: persisted_source.id().as_str().to_owned(),
            source_kind: persisted_source.kind().to_owned(),
            source_name: persisted_source.display_name().to_owned(),
            source_revision: persisted_source.revision().as_str().to_owned(),
            search: String::new(),
            playlist_id: None,
            workflow_filter: None,
            workflow_step_id: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
            sort: LibraryTrackSort::default(),
            editor_track_id: None,
            pending_source_refresh: None,
            pending_rekordbox_preview: None,
            pending_rekordbox_diff: None,
            last_rekordbox_apply: None,
            pending_device_inspection: None,
            device_review_comparisons_by_source: BTreeMap::new(),
            pending_library_reset: None,
            pending_light_plan_preview: None,
        };
        worker.ensure_imported_timelines()?;
        if phrase_mapping_defaults_upgraded {
            worker.remap_untouched_source_timelines()?;
        }
        Ok(worker)
    }

    pub fn create_consistent_backup(
        &self,
        destination: &std::path::Path,
    ) -> Result<(), LibraryWorkerError> {
        self.validate_backup_location(destination)?;
        self.repository.create_consistent_backup(destination)?;
        Ok(())
    }

    pub fn restore_consistent_backup(
        &mut self,
        source: &std::path::Path,
        rollback: &std::path::Path,
    ) -> Result<(), LibraryWorkerError> {
        self.validate_backup_location(source)?;
        self.validate_backup_location(rollback)?;
        self.repository
            .restore_consistent_backup(source, rollback)?;
        self.pending_source_refresh = None;
        self.pending_rekordbox_preview = None;
        self.pending_rekordbox_diff = None;
        self.last_rekordbox_apply = None;
        self.pending_device_inspection = None;
        self.device_review_comparisons_by_source.clear();
        self.pending_library_reset = None;
        self.pending_light_plan_preview = None;
        let persisted = self
            .repository
            .library_source(&lumi_library::LibrarySourceId::try_new(&self.source_id)?)?
            .ok_or(LibraryWorkerError::MissingLibrarySource)?;
        self.source_id = persisted.id().as_str().to_owned();
        self.source_kind = persisted.kind().to_owned();
        self.source_name = persisted.display_name().to_owned();
        self.source_revision = persisted.revision().as_str().to_owned();
        seed_default_role_catalog(&mut self.repository)?;
        seed_default_autoloop_catalog(&mut self.repository)?;
        self.ensure_imported_timelines()?;
        Ok(())
    }

    fn validate_backup_location(&self, path: &std::path::Path) -> Result<(), LibraryWorkerError> {
        let database = self
            .database_path
            .as_ref()
            .ok_or(LibraryWorkerError::BackupUnavailable)?;
        let root = database
            .parent()
            .ok_or(LibraryWorkerError::BackupUnavailable)?
            .join("Backups");
        let parent = path
            .parent()
            .ok_or(LibraryWorkerError::UntrustedBackupPath)?;
        let package_parent = parent
            .parent()
            .ok_or(LibraryWorkerError::UntrustedBackupPath)?;
        if package_parent != root
            || path.file_name().and_then(|value| value.to_str()) != Some("library.sqlite")
        {
            return Err(LibraryWorkerError::UntrustedBackupPath);
        }
        Ok(())
    }

    pub fn query(&mut self, update: LibraryQueryUpdate) {
        self.search = update.search;
        self.playlist_id = update.playlist_id.map(PlaylistId::new);
        self.workflow_filter = update.workflow_filter;
        self.workflow_step_id = update.workflow_step_id;
        self.offset = update.offset;
        self.limit = update.limit;
        self.sort = update.sort;
    }

    pub fn assign_track_workflow_step(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        step_id: &str,
    ) -> Result<(), LibraryWorkerError> {
        self.repository.assign_track_workflow_step(
            TrackId::new(track_id),
            expected_revision,
            step_id,
        )?;
        Ok(())
    }

    pub fn replace_track_workflow_catalog(
        &mut self,
        expected_revision: u64,
        steps: Vec<WorkflowStepDefinition>,
    ) -> Result<(), LibraryWorkerError> {
        self.repository
            .replace_track_workflow_catalog(expected_revision, steps)?;
        Ok(())
    }

    pub fn set_track_preparation_status(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        status: TrackPreparationStatus,
    ) -> Result<(), LibraryWorkerError> {
        self.repository.set_track_preparation_status(
            TrackId::new(track_id),
            expected_revision,
            status,
        )?;
        Ok(())
    }

    pub fn resolve_track_workflow_attention(
        &mut self,
        track_id: u64,
        expected_revision: u64,
    ) -> Result<(), LibraryWorkerError> {
        self.repository
            .resolve_track_workflow_attention(TrackId::new(track_id), expected_revision)?;
        Ok(())
    }

    pub fn preview_library_reset(
        &mut self,
        preserve_track_ids: &[u64],
    ) -> Result<(), LibraryWorkerError> {
        let mut preserved = preserve_track_ids
            .iter()
            .copied()
            .map(TrackId::new)
            .collect::<Vec<_>>();
        preserved.sort_by_key(|track_id| track_id.value());
        preserved.dedup();
        let impact = self.repository.preview_library_reset(&preserved)?;
        if impact.preserved_track_count
            != u64::try_from(preserved.len())
                .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?
        {
            return Err(LibraryWorkerError::UnknownResetPreservedTrack);
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        self.pending_library_reset = Some(LibraryResetPreviewState {
            token: format!("library-reset-{nonce}-{}", impact.track_count),
            preserve_track_ids: preserved,
            impact,
            authored_heads: self
                .repository
                .reset_preservable_tracks()?
                .into_iter()
                .map(|track| (track.track_id, track.timeline_revision))
                .collect(),
        });
        Ok(())
    }

    pub fn apply_library_reset(
        &mut self,
        expected_token: &str,
        backup_database_path: &str,
    ) -> Result<(), LibraryWorkerError> {
        let preview = self
            .pending_library_reset
            .as_ref()
            .ok_or(LibraryWorkerError::NoPendingLibraryReset)?
            .clone();
        if preview.token != expected_token {
            return Err(LibraryWorkerError::LibraryResetPreviewChanged);
        }
        let current_impact = self
            .repository
            .preview_library_reset(&preview.preserve_track_ids)?;
        let current_authored_heads = self
            .repository
            .reset_preservable_tracks()?
            .into_iter()
            .map(|track| (track.track_id, track.timeline_revision))
            .collect::<Vec<_>>();
        if current_impact != preview.impact || current_authored_heads != preview.authored_heads {
            self.pending_library_reset = None;
            return Err(LibraryWorkerError::LibraryResetPreviewChanged);
        }
        let backup_path = Path::new(backup_database_path);
        let backup_metadata =
            fs::metadata(backup_path).map_err(|_| LibraryWorkerError::MissingLibraryResetBackup)?;
        if !backup_metadata.is_file() || backup_metadata.len() < 4_096 {
            return Err(LibraryWorkerError::MissingLibraryResetBackup);
        }
        let mut header = [0_u8; 16];
        fs::File::open(backup_path)?.read_exact(&mut header)?;
        if &header != b"SQLite format 3\0" {
            return Err(LibraryWorkerError::InvalidLibraryResetBackup);
        }
        self.repository
            .reset_library_content(&preview.preserve_track_ids)?;
        self.pending_library_reset = None;
        self.pending_rekordbox_preview = None;
        self.pending_rekordbox_diff = None;
        self.last_rekordbox_apply = None;
        self.pending_device_inspection = None;
        self.device_review_comparisons_by_source.clear();
        self.editor_track_id = None;
        self.playlist_id = None;
        self.search.clear();
        self.offset = 0;
        Ok(())
    }

    pub fn preview_rekordbox_xml_sync(
        &mut self,
        folder: String,
        followed_paths: Vec<String>,
        include_future_child_playlists: bool,
    ) -> Result<(), LibraryWorkerError> {
        let request = RekordboxXmlSyncRequest::try_new(
            folder,
            followed_paths,
            include_future_child_playlists,
        )?;
        let preview = load_latest_mirror(&request)?;
        let mirror = rekordbox_mirror_snapshot(&preview)?;
        self.pending_rekordbox_diff = Some(self.repository.preview_source_mirror(&mirror)?);
        self.pending_rekordbox_preview = Some(preview);
        self.last_rekordbox_apply = None;
        Ok(())
    }

    pub fn apply_rekordbox_xml_sync(
        &mut self,
        folder: String,
        followed_paths: Vec<String>,
        include_future_child_playlists: bool,
        expected_content_sha256: &str,
    ) -> Result<(), LibraryWorkerError> {
        let request = RekordboxXmlSyncRequest::try_new(
            folder,
            followed_paths,
            include_future_child_playlists,
        )?;
        let fresh = load_latest_mirror(&request)?;
        let pending = self
            .pending_rekordbox_preview
            .as_ref()
            .ok_or(LibraryWorkerError::NoPendingRekordboxPreview)?;
        if fresh.content_sha256() != expected_content_sha256
            || pending.content_sha256() != expected_content_sha256
            || pending.selection_paths() != fresh.selection_paths()
            || pending.include_future_child_playlists() != fresh.include_future_child_playlists()
        {
            return Err(LibraryWorkerError::RekordboxPreviewChanged);
        }
        let mirror = rekordbox_mirror_snapshot(&fresh)?;
        let applied = self.repository.apply_source_mirror(&mirror)?;
        self.pending_rekordbox_diff = Some(self.repository.preview_source_mirror(&mirror)?);
        self.pending_rekordbox_preview = Some(fresh);
        self.last_rekordbox_apply = Some(applied);
        Ok(())
    }

    pub fn import_rekordbox_analysis(
        &mut self,
        folder: String,
        followed_paths: Vec<String>,
        include_future_child_playlists: bool,
        expected_content_sha256: &str,
    ) -> Result<(), LibraryWorkerError> {
        let request = RekordboxXmlSyncRequest::try_new(
            folder,
            followed_paths,
            include_future_child_playlists,
        )?;
        let snapshot = load_latest_mirror(&request)?;
        if snapshot.content_sha256() != expected_content_sha256 {
            return Err(LibraryWorkerError::RekordboxPreviewChanged);
        }
        let paths = RekordboxInstallationPaths::discover()?;
        let temporary = RekordboxImportTemporaryRoot::create()?;
        let database_snapshot =
            create_database_snapshot(&paths.database, temporary.path().join("master.snapshot.db"))?;
        let requested = snapshot
            .tracks()
            .iter()
            .map(|track| RequestedTrack::try_new(track.source_track_id(), track.location()))
            .collect::<Result<Vec<_>, _>>()?;
        let resolver = SqlCipherResolver::try_new(paths.sqlcipher)?;
        let key = DatabaseKey::rekordbox7_bundled()?;
        let resolved =
            resolver.resolve(&database_snapshot, &key, &paths.analysis_root, requested)?;
        if resolved.report.resolved_tracks != snapshot.tracks().len()
            || resolved.report.missing_database_rows != 0
            || resolved.report.missing_analysis_paths != 0
            || resolved.report.audio_path_mismatches != 0
        {
            return Err(LibraryWorkerError::IncompleteRekordboxResolution {
                requested: snapshot.tracks().len(),
                resolved: resolved.report.resolved_tracks,
                path_mismatches: resolved.report.audio_path_mismatches,
            });
        }
        let analysis_request = ResolvedAnalysisRequest::try_new(
            &paths.analysis_root,
            temporary.path().join("analysis"),
            resolved
                .tracks
                .values()
                .map(|track| {
                    ResolvedAnalysisTrack::try_new(track.source_track_id(), track.analysis_file())
                })
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let analysis = snapshot_resolved_analysis_data(&analysis_request)?;
        if analysis.tracks.len() != snapshot.tracks().len() {
            return Err(LibraryWorkerError::IncompleteRekordboxAnalysis {
                requested: snapshot.tracks().len(),
                parsed: analysis.tracks.len(),
            });
        }
        let baseline =
            rekordbox_canonical_baseline(&snapshot, &analysis.tracks, database_snapshot.sha256())?;
        self.repository.import_baseline(&baseline)?;
        self.source_id = baseline.source_id().as_str().to_owned();
        self.source_kind = baseline.source_kind().to_owned();
        self.source_name = baseline.display_name().to_owned();
        self.source_revision = baseline.source_revision().as_str().to_owned();
        self.search.clear();
        self.playlist_id = None;
        self.offset = 0;
        self.editor_track_id = None;
        self.pending_source_refresh = None;
        self.ensure_imported_timelines()?;
        Ok(())
    }

    /// Imports a mounted Rekordbox Device Library as a performance identity
    /// source. Device rows never replace Lumi-owned phrases or AutoLoop
    /// choices. Matched beatgrids and waveforms are refreshed atomically with
    /// the aliases, so an interrupted sync leaves the previous snapshot live.
    pub fn inspect_rekordbox_device(
        &mut self,
        root: impl AsRef<Path>,
        source_id: Option<&str>,
    ) -> Result<(), LibraryWorkerError> {
        let mut snapshot = read_device_library(root)?;
        if let Some(source_id) = source_id.filter(|value| !value.trim().is_empty()) {
            snapshot.source_id = source_id.to_owned();
        }
        let inspection = self.prepare_device_inspection(snapshot)?;
        self.remember_device_inspection(inspection);
        Ok(())
    }

    pub fn sync_rekordbox_device(
        &mut self,
        root: impl AsRef<Path>,
        source_id: Option<&str>,
        playlist_ids: &[u32],
    ) -> Result<RekordboxDeviceSyncResult, LibraryWorkerError> {
        let mut snapshot = read_device_library(root)?;
        if let Some(source_id) = source_id.filter(|value| !value.trim().is_empty()) {
            snapshot.source_id = source_id.to_owned();
        }
        let inspection = snapshot.clone();
        let requested = playlist_ids.iter().copied().collect::<BTreeSet<_>>();
        if requested.is_empty() || requested.len() != playlist_ids.len() {
            return Err(LibraryWorkerError::InvalidDevicePlaylistSelection);
        }
        let selected_playlists = snapshot
            .playlists
            .iter()
            .filter(|playlist| requested.contains(&playlist.device_playlist_id))
            .cloned()
            .collect::<Vec<_>>();
        if selected_playlists.len() != requested.len() {
            return Err(LibraryWorkerError::InvalidDevicePlaylistSelection);
        }
        let selected_track_ids = selected_playlists
            .iter()
            .flat_map(|playlist| playlist.track_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        if selected_track_ids.is_empty() {
            return Err(LibraryWorkerError::EmptyDevicePlaylistSelection);
        }
        snapshot
            .tracks
            .retain(|device_track_id, _| selected_track_ids.contains(device_track_id));
        snapshot.playlists = selected_playlists;
        for track in snapshot.tracks.values_mut() {
            track.audio_signature = audio_content_signature(&track.audio_path)?;
        }
        let stored_aliases = self.repository.device_alias_states(&snapshot.source_id)?;
        let candidates = self.repository.device_match_candidates()?;
        let mut audio_candidates = BTreeMap::<String, Vec<TrackId>>::new();
        for candidate in &candidates {
            let Some(path) = file_uri_path(&candidate.audio_uri) else {
                continue;
            };
            if let Ok(signature) = audio_content_signature(path) {
                audio_candidates
                    .entry(signature)
                    .or_default()
                    .push(candidate.track_id);
            }
        }
        let preliminary_matches = snapshot
            .tracks
            .values()
            .map(|device_track| {
                let strict_matches = candidates
                    .iter()
                    .filter(|candidate| device_track_matches(candidate, device_track))
                    .collect::<Vec<_>>();
                let metadata_matches = candidates
                    .iter()
                    .filter(|candidate| device_metadata_matches(candidate, device_track))
                    .collect::<Vec<_>>();
                let stored_canonical = stored_aliases
                    .get(&device_track.device_track_id)
                    .and_then(|state| state.canonical_track_id);
                let stored_is_device_import = stored_canonical.is_some_and(|track_id| {
                    candidates.iter().any(|candidate| {
                        candidate.track_id == track_id
                            && candidate.source_kind == "rekordbox-device"
                            && !candidate.has_user_timeline_edits
                    })
                });
                let canonical_repair = stored_is_device_import.then(|| {
                    metadata_matches
                        .iter()
                        .filter(|candidate| candidate.source_kind != "rekordbox-device")
                        .copied()
                        .collect::<Vec<_>>()
                });
                let (canonical_track_id, candidate_count, match_kind) = if canonical_repair
                    .as_ref()
                    .is_some_and(|matches| matches.len() == 1)
                {
                    (
                        canonical_repair
                            .as_ref()
                            .and_then(|matches| matches.first())
                            .map(|candidate| candidate.track_id),
                        1,
                        "metadata-canonical-repair",
                    )
                } else if strict_matches.len() == 1 {
                    (Some(strict_matches[0].track_id), 1, "metadata+file-size")
                } else if strict_matches.is_empty() {
                    let audio_matches = audio_candidates
                        .get(&device_track.audio_signature)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    if audio_matches.len() == 1 {
                        (audio_matches.first().copied(), 1, "audio-signature")
                    } else if audio_matches.is_empty() && metadata_matches.len() == 1 {
                        (Some(metadata_matches[0].track_id), 1, "metadata-exact")
                    } else {
                        let candidate_count = audio_matches.len().max(metadata_matches.len());
                        (
                            None,
                            candidate_count,
                            if candidate_count == 0 {
                                "unmatched"
                            } else {
                                "ambiguous"
                            },
                        )
                    }
                } else {
                    (None, strict_matches.len(), "ambiguous")
                };
                (
                    device_track,
                    canonical_track_id,
                    candidate_count,
                    match_kind,
                )
            })
            .collect::<Vec<_>>();
        let mut canonical_match_counts = BTreeMap::<TrackId, usize>::new();
        for (_, canonical_track_id, _, _) in &preliminary_matches {
            if let Some(track_id) = canonical_track_id {
                *canonical_match_counts.entry(*track_id).or_default() += 1;
            }
        }
        let mut aliases = Vec::with_capacity(snapshot.tracks.len());
        let mut matched_tracks =
            BTreeMap::<TrackId, (&DeviceTrack, lumi_library_sqlite::DeviceAnalysisDecision)>::new();
        let mut matched_hot_cues =
            BTreeMap::<TrackId, (&DeviceTrack, lumi_library_sqlite::DeviceAnalysisDecision)>::new();
        for (device_track, preliminary_track_id, candidate_count, match_kind) in preliminary_matches
        {
            let canonical_track_id = preliminary_track_id
                .filter(|track_id| canonical_match_counts.get(track_id).copied() == Some(1));
            if let Some(track_id) = canonical_track_id {
                let mut decision = self.repository.device_analysis_decision(
                    track_id,
                    &snapshot.source_id,
                    &device_track.analysis_revision,
                    &device_track.analyzed_at,
                )?;
                let keeps_exact_revision = stored_aliases
                    .get(&device_track.device_track_id)
                    .is_some_and(|previous| {
                        is_kept_active_revision(
                            &previous.sync_disposition,
                            &previous.analysis_revision,
                            &device_track.analysis_revision,
                        )
                    });
                if keeps_exact_revision {
                    decision = lumi_library_sqlite::DeviceAnalysisDecision::KeepActive;
                }
                matched_tracks.insert(track_id, (device_track, decision));
                let mut hot_cue_decision = self.repository.device_hot_cue_decision(
                    track_id,
                    &snapshot.source_id,
                    &device_track.analysis_revision,
                    &device_track.analyzed_at,
                )?;
                if keeps_exact_revision {
                    hot_cue_decision = lumi_library_sqlite::DeviceAnalysisDecision::KeepActive;
                }
                matched_hot_cues.insert(track_id, (device_track, hot_cue_decision));
            }
            let sync_disposition = canonical_track_id
                .and_then(|track_id| {
                    matched_tracks
                        .get(&track_id)
                        .map(|(_, decision)| decision.disposition())
                })
                .unwrap_or("unmatched")
                .to_owned();
            aliases.push(DeviceAliasUpsert {
                device_track_id: device_track.device_track_id,
                simulator_signature: device_track.simulator_signature,
                canonical_track_id,
                match_kind: if canonical_track_id.is_some() {
                    match_kind.to_owned()
                } else if candidate_count == 0 {
                    "unmatched".to_owned()
                } else {
                    "ambiguous".to_owned()
                },
                title: device_track.title.clone(),
                artist: device_track.artist.clone(),
                bpm_milli: device_track.bpm_milli,
                duration_millis: u64::from(device_track.duration_millis),
                file_size: device_track.file_size,
                audio_uri: device_audio_uri(&device_track.audio_path),
                metadata_revision: device_track.metadata_revision.clone(),
                color_rgb: device_track.color_rgb,
                master_database_id: device_track.master_database_id,
                master_content_id: device_track.master_content_id,
                information_update_count: device_track.information_update_count,
                analysis_revision: device_track.analysis_revision.clone(),
                audio_signature: device_track.audio_signature.clone(),
                analyzed_at: device_track.analyzed_at.clone(),
                sync_disposition,
            });
        }

        let temporary = RekordboxImportTemporaryRoot::create()?;
        let analysis_root = snapshot
            .database_path
            .parent()
            .and_then(Path::parent)
            .ok_or(LibraryWorkerError::InvalidRekordboxDeviceRoot)?
            .join("USBANLZ");
        let promotable_tracks = matched_tracks
            .iter()
            .filter(|(_, (_, decision))| decision.promotes())
            .map(|(track_id, (track, _))| (*track_id, *track))
            .collect::<BTreeMap<_, _>>();
        let promotable_hot_cues = matched_hot_cues
            .iter()
            .filter(|(track_id, (_, decision))| {
                decision.promotes() && !promotable_tracks.contains_key(track_id)
            })
            .map(|(track_id, (track, _))| (*track_id, *track))
            .collect::<BTreeMap<_, _>>();
        let analysis_tracks = promotable_tracks
            .iter()
            .chain(promotable_hot_cues.iter())
            .map(|(track_id, track)| (*track_id, *track))
            .collect::<BTreeMap<_, _>>();
        let parsed_analyses = if analysis_tracks.is_empty() {
            BTreeMap::new()
        } else {
            let request = ResolvedAnalysisRequest::try_new(
                &analysis_root,
                temporary.path().join("device-analysis"),
                analysis_tracks
                    .iter()
                    .map(|(track_id, device_track)| {
                        ResolvedAnalysisTrack::try_new(
                            track_id.value().to_string(),
                            &device_track.analysis_dat_path,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            snapshot_resolved_analysis_data(&request)?.tracks
        };
        let analyses = promotable_tracks
            .iter()
            .map(|(track_id, device_track)| {
                let analysis = parsed_analyses
                    .get(&track_id.value().to_string())
                    .ok_or_else(|| {
                        LibraryWorkerError::MissingRekordboxTrackAnalysis(
                            device_track.device_track_id.to_string(),
                        )
                    })?;
                device_analysis_upsert(&snapshot.source_id, *track_id, device_track, analysis)
            })
            .collect::<Result<Vec<_>, LibraryWorkerError>>()?;
        let hot_cue_updates = promotable_hot_cues
            .iter()
            .map(|(track_id, device_track)| {
                let analysis = parsed_analyses
                    .get(&track_id.value().to_string())
                    .ok_or_else(|| {
                        LibraryWorkerError::MissingRekordboxTrackAnalysis(
                            device_track.device_track_id.to_string(),
                        )
                    })?;
                Ok(DeviceHotCueUpsert {
                    track_id: *track_id,
                    source_id: snapshot.source_id.clone(),
                    device_track_id: device_track.device_track_id,
                    source_analysis_revision: device_track.analysis_revision.clone(),
                    analyzed_at: device_track.analyzed_at.clone(),
                    hot_cues: canonical_hot_cues(
                        analysis,
                        u64::from(device_track.duration_millis),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, LibraryWorkerError>>()?;

        let new_device_tracks = aliases
            .iter()
            .filter(|alias| alias.match_kind == "unmatched")
            .filter_map(|alias| snapshot.tracks.get(&alias.device_track_id))
            .collect::<Vec<_>>();
        let new_tracks = if new_device_tracks.is_empty() {
            Vec::new()
        } else {
            let request = ResolvedAnalysisRequest::try_new(
                &analysis_root,
                temporary.path().join("device-new-track-analysis"),
                new_device_tracks
                    .iter()
                    .map(|device_track| {
                        ResolvedAnalysisTrack::try_new(
                            device_track.device_track_id.to_string(),
                            &device_track.analysis_dat_path,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            let parsed = snapshot_resolved_analysis_data(&request)?;
            new_device_tracks
                .iter()
                .map(|device_track| {
                    let analysis = parsed
                        .tracks
                        .get(&device_track.device_track_id.to_string())
                        .ok_or_else(|| {
                            LibraryWorkerError::MissingRekordboxTrackAnalysis(
                                device_track.device_track_id.to_string(),
                            )
                        })?;
                    let canonical_grid = canonical_beat_grid(analysis)?;
                    let beat_grid = canonical_grid.beat_grid;
                    let total_beats = u32::try_from(beat_grid.markers().len())
                        .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?;
                    let bpm_milli = (20_000..=300_000)
                        .contains(&device_track.bpm_milli)
                        .then_some(device_track.bpm_milli)
                        .or_else(|| median_analysis_bpm_milli(analysis))
                        .ok_or_else(|| LibraryWorkerError::InvalidRekordboxMetadata {
                            track_id: device_track.device_track_id.to_string(),
                            field: "BPM",
                        })?;
                    let musical_key = parse_musical_key(Some(&device_track.musical_key))
                        .ok_or_else(|| LibraryWorkerError::InvalidRekordboxMetadata {
                            track_id: device_track.device_track_id.to_string(),
                            field: "key",
                        })?;
                    let duration_millis = waveform_duration_millis(analysis)
                        .or_else(|| {
                            (device_track.duration_millis > 0)
                                .then_some(u64::from(device_track.duration_millis))
                        })
                        .or_else(|| inferred_duration_millis(&beat_grid, bpm_milli))
                        .ok_or(LibraryWorkerError::RekordboxImportOverflow)?;
                    let imported = ImportedTrackAnalysis::try_new(
                        lumi_library::SourceTrackId::try_new(format!(
                            "onelibrary:{}",
                            device_track.device_track_id
                        ))?,
                        SourceRevision::try_new(format!(
                            "device:{}:{}",
                            snapshot.source_id, device_track.analysis_revision
                        ))?,
                        nonempty_device_metadata(&device_track.title, "Unknown Track"),
                        nonempty_device_metadata(&device_track.artist, "Unknown Artist"),
                        bpm_milli,
                        musical_key,
                        duration_millis,
                        device_track.color_rgb.map(TrackColor::from_rgb_u32),
                        device_audio_uri(&device_track.audio_path),
                        beat_grid,
                        downsample_waveform(&analysis.waveform, MAX_IMPORTED_WAVEFORM_POINTS),
                        canonical_phrases(
                            analysis,
                            total_beats,
                            canonical_grid.source_beat_offset,
                        )?,
                    )?
                    .with_hot_cues(canonical_hot_cues(analysis, duration_millis)?)?;
                    Ok(DeviceTrackImport {
                        device_track_id: device_track.device_track_id,
                        source_analysis_revision: device_track.analysis_revision.clone(),
                        analyzed_at: device_track.analyzed_at.clone(),
                        analysis: imported,
                    })
                })
                .collect::<Result<Vec<_>, LibraryWorkerError>>()?
        };
        self.repository.sync_device_aliases(
            &snapshot.source_id,
            &snapshot.display_name,
            &snapshot.database_revision,
            &mut aliases,
            &new_tracks,
            &analyses,
            &hot_cue_updates,
            &snapshot
                .playlists
                .iter()
                .map(|playlist| DevicePlaylistUpsert {
                    device_playlist_id: playlist.device_playlist_id,
                    path: playlist.path.clone(),
                    device_track_ids: playlist.track_ids.clone(),
                })
                .collect::<Vec<_>>(),
        )?;
        self.ensure_imported_timelines()?;
        let _ = self.repository.relink_creative_archives()?;
        let inspection = self.prepare_device_inspection(inspection)?;
        self.remember_device_inspection(inspection);
        let matched = aliases
            .iter()
            .filter(|alias| alias.canonical_track_id.is_some())
            .count();
        let protected_older = matched_tracks
            .values()
            .filter(|(_, decision)| {
                matches!(
                    decision,
                    lumi_library_sqlite::DeviceAnalysisDecision::ProtectOlder
                )
            })
            .count();
        let held_conflicts = matched_tracks
            .values()
            .filter(|(_, decision)| {
                matches!(
                    decision,
                    lumi_library_sqlite::DeviceAnalysisDecision::HoldConflict
                )
            })
            .count();
        Ok(RekordboxDeviceSyncResult {
            source_id: snapshot.source_id,
            display_name: snapshot.display_name,
            database_revision: snapshot.database_revision,
            tracks: aliases.len(),
            matched,
            unmatched: aliases.len().saturating_sub(matched),
            refreshed_analyses: analyses.len(),
            protected_older,
            held_conflicts,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn resolve_rekordbox_device_conflict(
        &mut self,
        root: impl AsRef<Path>,
        source_id: &str,
        device_track_id: u32,
        expected_incoming_revision: &str,
        expected_active_revision: &str,
        choice: DeviceReviewChoice,
    ) -> Result<(), LibraryWorkerError> {
        match choice {
            DeviceReviewChoice::KeepLumi => {
                self.repository.keep_active_device_analysis(
                    source_id,
                    device_track_id,
                    expected_incoming_revision,
                    expected_active_revision,
                )?;
                let mut snapshot = read_device_library(root.as_ref())?;
                snapshot.source_id = source_id.to_owned();
                let inspection = self.prepare_device_inspection(snapshot)?;
                self.remember_device_inspection(inspection);
                return Ok(());
            }
            DeviceReviewChoice::UseUsb => {}
        }
        let mut snapshot = read_device_library(root.as_ref())?;
        snapshot.source_id = source_id.to_owned();
        let device_track = snapshot
            .tracks
            .get(&device_track_id)
            .ok_or(LibraryWorkerError::DeviceReviewChanged)?;
        if device_track.analysis_revision != expected_incoming_revision {
            return Err(LibraryWorkerError::DeviceReviewChanged);
        }
        let review = self
            .repository
            .device_review_tracks()?
            .remove(source_id)
            .and_then(|tracks| {
                tracks
                    .into_iter()
                    .find(|track| track.device_track_id == device_track_id)
            })
            .ok_or(LibraryWorkerError::DeviceReviewChanged)?;
        let canonical_track_id = review
            .canonical_track_id
            .ok_or(LibraryWorkerError::DeviceReviewChanged)?;
        let analysis_root = snapshot
            .database_path
            .parent()
            .and_then(Path::parent)
            .ok_or(LibraryWorkerError::InvalidRekordboxDeviceRoot)?
            .join("USBANLZ");
        let temporary = RekordboxImportTemporaryRoot::create()?;
        let request = ResolvedAnalysisRequest::try_new(
            &analysis_root,
            temporary.path().join("device-reviewed-analysis"),
            vec![ResolvedAnalysisTrack::try_new(
                device_track_id.to_string(),
                &device_track.analysis_dat_path,
            )?],
        )?;
        let parsed = snapshot_resolved_analysis_data(&request)?;
        let resolved = parsed
            .tracks
            .get(&device_track_id.to_string())
            .ok_or_else(|| {
                LibraryWorkerError::MissingRekordboxTrackAnalysis(device_track_id.to_string())
            })?;
        let analysis =
            device_analysis_upsert(source_id, canonical_track_id, device_track, resolved)?;
        self.repository
            .promote_reviewed_device_analysis(&analysis, expected_active_revision)?;
        self.ensure_imported_timelines()?;
        let inspection = self.prepare_device_inspection(snapshot)?;
        self.remember_device_inspection(inspection);
        Ok(())
    }

    fn remember_device_inspection(&mut self, inspection: DeviceInspection) {
        self.device_review_comparisons_by_source.insert(
            inspection.snapshot.source_id.clone(),
            inspection.review_comparisons.clone(),
        );
        self.pending_device_inspection = Some(inspection);
    }

    fn prepare_device_inspection(
        &self,
        snapshot: DeviceLibrarySnapshot,
    ) -> Result<DeviceInspection, LibraryWorkerError> {
        let stored = self.repository.device_alias_states(&snapshot.source_id)?;
        let candidates = self.repository.device_match_candidates()?;
        let mut tracks = BTreeMap::new();
        for track in snapshot.tracks.values() {
            let state = if let Some(previous) = stored.get(&track.device_track_id) {
                if previous.canonical_track_id.is_none() {
                    DeviceInspectionTrack {
                        status: "not-in-lumi",
                        detail: "This USB track was not matched to a Lumi track during the previous sync."
                            .to_owned(),
                    }
                } else if kept_active_track_is_current(
                    &previous.sync_disposition,
                    &previous.analysis_revision,
                    &track.analysis_revision,
                    &previous.metadata_revision,
                    &track.metadata_revision,
                ) {
                    DeviceInspectionTrack {
                        status: "current",
                        detail:
                            "You chose to keep the active Lumi version for this exact USB revision."
                                .to_owned(),
                    }
                } else if previous.sync_disposition == "held-conflict" {
                    DeviceInspectionTrack {
                        status: "conflict",
                        detail: "This USB analysis differs from the active Lumi analysis and their source dates cannot order them safely. Lumi kept the active analysis for review."
                            .to_owned(),
                    }
                } else if previous.sync_disposition == "protected-older" {
                    DeviceInspectionTrack {
                        status: "usb-outdated",
                        detail: "Lumi has a newer protected analysis; sync did not downgrade it."
                            .to_owned(),
                    }
                } else if previous.metadata_revision == track.metadata_revision
                    && previous.analysis_revision == track.analysis_revision
                {
                    DeviceInspectionTrack {
                        status: "current",
                        detail: "USB and Lumi use the same synchronized track revision.".to_owned(),
                    }
                } else {
                    DeviceInspectionTrack {
                        status: "usb-newer",
                        detail: "This trusted USB changed after its previous Lumi sync.".to_owned(),
                    }
                }
            } else {
                let matches = candidates
                    .iter()
                    .filter(|candidate| device_track_matches(candidate, track))
                    .collect::<Vec<_>>();
                if matches.len() == 1 {
                    let decision = self.repository.device_analysis_decision(
                        matches[0].track_id,
                        &snapshot.source_id,
                        &track.analysis_revision,
                        &track.analyzed_at,
                    )?;
                    match decision {
                        lumi_library_sqlite::DeviceAnalysisDecision::Current => {
                            DeviceInspectionTrack {
                                status: "current",
                                detail: "The OneLibrary analysis already matches Lumi.".to_owned(),
                            }
                        }
                        lumi_library_sqlite::DeviceAnalysisDecision::KeepActive => {
                            DeviceInspectionTrack {
                                status: "current",
                                detail: "You chose to keep the active Lumi version for this exact USB revision."
                                    .to_owned(),
                            }
                        }
                        lumi_library_sqlite::DeviceAnalysisDecision::PromoteInitial
                        | lumi_library_sqlite::DeviceAnalysisDecision::PromoteNewer => {
                            DeviceInspectionTrack {
                                status: "usb-newer",
                                detail: "The USB contains a newer analysis revision for this Lumi track."
                                    .to_owned(),
                            }
                        }
                        lumi_library_sqlite::DeviceAnalysisDecision::ProtectOlder => {
                            DeviceInspectionTrack {
                                status: "usb-outdated",
                                detail: "Lumi has a newer protected analysis; sync will not downgrade it."
                                    .to_owned(),
                            }
                        }
                        lumi_library_sqlite::DeviceAnalysisDecision::HoldConflict => {
                            DeviceInspectionTrack {
                                status: "conflict",
                                detail: "The revisions differ but cannot be ordered safely."
                                    .to_owned(),
                            }
                        }
                    }
                } else if matches.is_empty() {
                    DeviceInspectionTrack {
                        status: "not-in-lumi",
                        detail: "No canonical Lumi track match was found.".to_owned(),
                    }
                } else {
                    DeviceInspectionTrack {
                        status: "conflict",
                        detail: "Multiple Lumi tracks match this USB identity.".to_owned(),
                    }
                }
            };
            tracks.insert(track.device_track_id, state);
        }
        let review_comparisons = self.device_review_comparisons(&snapshot, &stored)?;
        let mut selected_playlist_ids = self
            .repository
            .device_selected_playlist_ids(&snapshot.source_id)?;
        let selected_paths = self
            .repository
            .device_selected_playlist_paths(&snapshot.source_id)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        selected_playlist_ids.extend(
            snapshot
                .playlists
                .iter()
                .filter(|playlist| selected_paths.contains(&playlist.path))
                .map(|playlist| playlist.device_playlist_id),
        );
        selected_playlist_ids.sort_unstable();
        selected_playlist_ids.dedup();
        Ok(DeviceInspection {
            snapshot,
            selected_playlist_ids,
            tracks,
            review_comparisons,
        })
    }

    fn device_review_comparisons(
        &self,
        snapshot: &DeviceLibrarySnapshot,
        stored: &BTreeMap<u32, lumi_library_sqlite::StoredDeviceAliasState>,
    ) -> Result<BTreeMap<u32, DeviceReviewComparison>, LibraryWorkerError> {
        let conflicts = stored
            .iter()
            .filter_map(|(device_track_id, alias)| {
                if alias.sync_disposition != "held-conflict" {
                    return None;
                }
                Some((
                    *device_track_id,
                    alias.canonical_track_id?,
                    snapshot.tracks.get(device_track_id)?,
                ))
            })
            .collect::<Vec<_>>();
        if conflicts.is_empty() {
            return Ok(BTreeMap::new());
        }
        let analysis_root = snapshot
            .database_path
            .parent()
            .and_then(Path::parent)
            .ok_or(LibraryWorkerError::InvalidRekordboxDeviceRoot)?
            .join("USBANLZ");
        let temporary = RekordboxImportTemporaryRoot::create()?;
        let request = ResolvedAnalysisRequest::try_new(
            &analysis_root,
            temporary.path().join("device-review-analysis"),
            conflicts
                .iter()
                .map(|(device_track_id, _, track)| {
                    ResolvedAnalysisTrack::try_new(
                        device_track_id.to_string(),
                        &track.analysis_dat_path,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let parsed = snapshot_resolved_analysis_data(&request)?.tracks;
        let mut comparisons = BTreeMap::new();
        for (device_track_id, canonical_track_id, device_track) in conflicts {
            let Some(resolved) = parsed.get(&device_track_id.to_string()) else {
                continue;
            };
            let incoming = device_analysis_upsert(
                &snapshot.source_id,
                canonical_track_id,
                device_track,
                resolved,
            )?;
            let Some(active) = self.repository.track(canonical_track_id)? else {
                continue;
            };
            let summary = active.summary();
            let mut file_changes = Vec::new();
            if summary.title() != device_track.title {
                file_changes.push(format!(
                    "title: USB ‘{}’ · Lumi ‘{}’",
                    device_track.title,
                    summary.title()
                ));
            }
            if summary.artist() != device_track.artist {
                file_changes.push(format!(
                    "artist: USB ‘{}’ · Lumi ‘{}’",
                    device_track.artist,
                    summary.artist()
                ));
            }
            if summary.bpm_milli() != device_track.bpm_milli {
                file_changes.push(format!(
                    "BPM: USB {:.3} · Lumi {:.3}",
                    f64::from(device_track.bpm_milli) / 1_000.0,
                    f64::from(summary.bpm_milli()) / 1_000.0
                ));
            }
            if summary.duration_millis() != incoming.duration_millis {
                file_changes.push(format!(
                    "duration: USB {:.3}s · Lumi {:.3}s",
                    incoming.duration_millis as f64 / 1_000.0,
                    summary.duration_millis() as f64 / 1_000.0
                ));
            }
            if summary.color().map(TrackColor::rgb_u32) != device_track.color_rgb {
                file_changes.push(format!(
                    "track color: USB {} · Lumi {}",
                    optional_rgb(device_track.color_rgb),
                    optional_rgb(summary.color().map(TrackColor::rgb_u32))
                ));
            }
            let file_data_changed = !file_changes.is_empty();
            let file_detail = if file_changes.is_empty() {
                format!(
                    "Track metadata is unchanged · USB file size {} bytes",
                    device_track.file_size
                )
            } else {
                format!(
                    "{} · USB file size {} bytes",
                    file_changes.join("; "),
                    device_track.file_size
                )
            };
            let beat_grid_changed = active.beat_grid() != &incoming.beat_grid;
            let hot_cues_changed = active.hot_cues() != incoming.hot_cues.as_slice();
            let raw_phrases_changed = active.raw_phrases() != incoming.raw_phrases.as_slice();
            let waveform_changed = active.waveform() != incoming.waveform.as_slice();
            comparisons.insert(
                device_track_id,
                DeviceReviewComparison {
                    beat_grid_changed,
                    hot_cues_changed,
                    file_data_changed,
                    raw_phrases_changed,
                    waveform_changed,
                    beat_grid_detail: beat_grid_review_detail(
                        &incoming.beat_grid,
                        active.beat_grid(),
                    ),
                    hot_cues_detail: hot_cue_review_detail(&incoming.hot_cues, active.hot_cues()),
                    raw_phrases_detail: raw_phrases_review_detail(
                        &incoming.raw_phrases,
                        active.raw_phrases(),
                    ),
                    waveform_detail: waveform_review_detail(&incoming.waveform, active.waveform()),
                    file_detail,
                },
            );
        }
        Ok(comparisons)
    }

    pub fn connected_track(
        &mut self,
        device_track_id: u32,
        simulator_signature: u32,
    ) -> Result<Option<ConnectedLibraryTrack>, LibraryWorkerError> {
        let Some(alias) = self
            .repository
            .resolve_device_alias(device_track_id, simulator_signature)?
        else {
            return Ok(None);
        };
        self.ensure_timeline(alias.canonical_track_id)?;
        let timeline = self
            .repository
            .timeline_head(alias.canonical_track_id)?
            .ok_or(LibraryWorkerError::MissingTimeline)?;
        let prepared = self.local_playback_track(
            alias.canonical_track_id.value(),
            timeline.revision().value(),
        )?;
        Ok(Some(ConnectedLibraryTrack { prepared }))
    }

    pub fn open_editor(&mut self, track_id: u64) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.ensure_timeline(track_id)?;
        self.editor_track_id = Some(track_id);
        Ok(())
    }

    pub fn waveform_detail_json(&self, track_id: u64) -> Result<Value, LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        let track = self
            .repository
            .track(track_id)?
            .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
        let points =
            deck_waveform_preview_points(track.waveform(), MAX_DECK_WAVEFORM_DETAIL_POINTS);
        Ok(json!({
            "trackId": track_id.value(),
            "source": "localLibraryDetail",
            "style": "rgb",
            "points": points.iter().map(|point| json!([
                point[0],
                point[1],
                point[2],
            ])).collect::<Vec<_>>(),
        }))
    }

    /// Builds a Local Playback load exclusively from the stored track identity and
    /// the exact current Lumi-owned timeline. Raw source phrases are never used
    /// after the initial timeline import.
    pub fn local_playback_track(
        &mut self,
        track_id: u64,
        expected_timeline_revision: u64,
    ) -> Result<LibraryLocalPlaybackTrack, LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.ensure_timeline(track_id)?;
        let track = self
            .repository
            .track(track_id)?
            .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
        let timeline = self.require_expected_head(track_id, expected_timeline_revision)?;
        let role_catalog = self.repository.phrase_role_catalog()?;
        let catalog = self.repository.autoloop_catalog()?;
        let duration_beats = timeline.total_beats();
        let phrases = timeline
            .phrases()
            .iter()
            .map(|phrase| {
                Ok(TrackPhrase::new(
                    phrase.index(),
                    phrase.start_beat(),
                    phrase.end_beat(),
                    planner_phrase_kind(phrase, timeline.phrases().len()),
                ))
            })
            .collect::<Result<Vec<_>, LibraryWorkerError>>()?;
        let identity = TrackIdentityFacts::try_new(
            self.source_kind.clone(),
            self.source_id.clone(),
            track.summary().source_track_id().as_str(),
            track.summary().source_revision().as_str(),
            timeline.revision().value(),
        )?;
        let metadata = TrackMetadata::try_new_with_color(
            track_id,
            track.summary().title().to_owned(),
            track.summary().artist().to_owned(),
            track.summary().bpm_milli(),
            track.summary().musical_key(),
            track.summary().color(),
            duration_beats,
            phrases,
        )?
        .with_identity_facts(identity);
        let context = LibraryPlanContext {
            provider_kind: self.source_kind.clone(),
            source_id: self.source_id.clone(),
            source_name: self.source_name.clone(),
            source_track_id: track.summary().source_track_id().as_str().to_owned(),
            analysis_revision: track.summary().source_revision().as_str().to_owned(),
            timeline_revision: timeline.revision().value(),
            audio_uri: self.resolved_audio_uri(&track)?,
            duration_millis: track.summary().duration_millis(),
            beat_grid: track.beat_grid().clone(),
            waveform: track.waveform().to_vec(),
            hot_cues: track.hot_cues().to_vec(),
            track_color: track.summary().color(),
            catalog,
            autoloop_overrides: BTreeMap::new(),
            phrases: timeline
                .phrases()
                .iter()
                .map(|phrase| LibraryPhrasePlanContext {
                    phrase_index: phrase.index(),
                    start_beat: phrase.start_beat(),
                    end_beat: phrase.end_beat(),
                    role_id: phrase.role_id().clone(),
                    role_name: role_display_name(role_catalog.roles(), phrase.role_id()),
                    strategy: phrase.loop_strategy().clone(),
                })
                .collect(),
        };

        // A physical 8-button bank is intentionally allowed to be sparse. The
        // selected Theme is therefore validated when the plan is resolved,
        // instead of rejecting a track because an unrelated bank lacks one of
        // its Phrase Types.
        Ok(LibraryLocalPlaybackTrack { metadata, context })
    }

    #[cfg(test)]
    pub fn simulator_track(
        &mut self,
        track_id: u64,
        expected_timeline_revision: u64,
    ) -> Result<LibraryLocalPlaybackTrack, LibraryWorkerError> {
        self.local_playback_track(track_id, expected_timeline_revision)
    }

    pub fn edit_timeline(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        command: TimelineEditCommand,
    ) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.require_open_track(track_id)?;
        self.require_phrases_unlocked(track_id)?;
        if let Some(role_id) = command.assigned_role_id() {
            self.require_active_role(role_id)?;
        }
        let head = self.require_expected_head(track_id, expected_revision)?;
        let edited = head.edit(command)?;
        self.repository
            .append_timeline_revision(&edited, Some(head.revision()))?;
        Ok(())
    }

    pub fn set_phrase_loop_strategy(
        &mut self,
        track_id: u64,
        expected_timeline_revision: u64,
        expected_catalog_revision: u64,
        phrase_index: u16,
        strategy: PhraseLoopStrategy,
    ) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.require_open_track(track_id)?;
        self.require_phrases_unlocked(track_id)?;
        let head = self.require_expected_head(track_id, expected_timeline_revision)?;
        let phrase = head
            .phrases()
            .get(usize::from(phrase_index))
            .ok_or(TimelineEditError::UnknownPhrase)?;
        let catalog = self.repository.autoloop_catalog()?;
        if catalog.revision() != expected_catalog_revision {
            return Err(LibraryWorkerError::AutoloopCatalogRevisionConflict {
                expected: expected_catalog_revision,
                actual: catalog.revision(),
            });
        }
        catalog.validate_loop_strategy(phrase.role_id(), &strategy)?;
        let edited = head.edit(TimelineEditCommand::SetLoopStrategy {
            phrase_index,
            strategy,
        })?;
        self.repository
            .append_timeline_revision(&edited, Some(head.revision()))?;
        Ok(())
    }

    pub fn undo_timeline(
        &mut self,
        track_id: u64,
        expected_revision: u64,
    ) -> Result<(), LibraryWorkerError> {
        self.restore_from_history(track_id, expected_revision, HistoryAction::Undo)
    }

    pub fn redo_timeline(
        &mut self,
        track_id: u64,
        expected_revision: u64,
    ) -> Result<(), LibraryWorkerError> {
        self.restore_from_history(track_id, expected_revision, HistoryAction::Redo)
    }

    pub fn restore_timeline_revision(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        target_revision: u64,
    ) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.require_open_track(track_id)?;
        self.require_phrases_unlocked(track_id)?;
        let head = self.require_expected_head(track_id, expected_revision)?;
        let target_revision = TimelineRevision::try_new(target_revision)
            .map_err(|_| LibraryWorkerError::InvalidTimelineRevision(target_revision))?;
        let target = self
            .repository
            .timeline_revision(track_id, target_revision)?
            .ok_or(LibraryWorkerError::UnknownTimelineRevision(
                target_revision.value(),
            ))?;
        let restored =
            LumiPhraseTimeline::restore(&head, &target, TimelineRevisionReason::RestoreRevision)?;
        self.repository
            .append_timeline_revision(&restored, Some(head.revision()))?;
        Ok(())
    }

    pub fn reuse_creative_timeline(
        &mut self,
        source_track_id: u64,
        target_track_id: u64,
        expected_target_revision: u64,
    ) -> Result<(), LibraryWorkerError> {
        let source_track_id = TrackId::new(source_track_id);
        let target_track_id = TrackId::new(target_track_id);
        self.require_open_track(target_track_id)?;
        self.require_phrases_unlocked(target_track_id)?;
        let target = self.require_expected_head(target_track_id, expected_target_revision)?;
        let source = self
            .repository
            .timeline_head(source_track_id)?
            .ok_or(LibraryWorkerError::CreativeTimelineSourceUnavailable)?;
        let eligible = self
            .repository
            .creative_timeline_candidates(target_track_id)?
            .iter()
            .any(|candidate| candidate.track_id == source_track_id);
        if !eligible {
            return Err(LibraryWorkerError::CreativeTimelineSourceUnavailable);
        }
        if source.total_beats() != target.total_beats() {
            return Err(LibraryWorkerError::CreativeTimelineIncompatible {
                source_beats: source.total_beats(),
                target_beats: target.total_beats(),
            });
        }
        let revision = target
            .revision()
            .checked_next()
            .ok_or(LibraryWorkerError::HistoryOverflow)?;
        let phrases = source
            .phrases()
            .iter()
            .enumerate()
            .map(|(index, phrase)| {
                PhraseInstance::new(
                    u16::try_from(index).unwrap_or(u16::MAX),
                    phrase.start_beat(),
                    phrase.end_beat(),
                    phrase.role_id().clone(),
                )
                .with_loop_strategy(phrase.loop_strategy().clone())
            })
            .collect::<Vec<_>>();
        let copied = LumiPhraseTimeline::try_new_with_history(
            target_track_id,
            revision,
            target.baseline_revision().clone(),
            target.total_beats(),
            TimelineRevisionOrigin::RevisionRestore,
            TimelineRevisionReason::RestoreRevision,
            Some(target.revision()),
            None,
            phrases,
        )?;
        self.repository
            .append_timeline_revision(&copied, Some(target.revision()))?;
        self.repository.resolve_track_version_candidate(
            target_track_id,
            source_track_id,
            expected_target_revision,
            "reused",
            Some(copied.revision().value()),
        )?;
        Ok(())
    }

    pub fn keep_track_version_separate(
        &mut self,
        source_track_id: u64,
        target_track_id: u64,
        expected_target_revision: u64,
    ) -> Result<(), LibraryWorkerError> {
        let target_track_id = TrackId::new(target_track_id);
        self.require_open_track(target_track_id)?;
        let _ = self.require_expected_head(target_track_id, expected_target_revision)?;
        let source_track_id = TrackId::new(source_track_id);
        let eligible = self
            .repository
            .creative_timeline_candidates(target_track_id)?
            .iter()
            .any(|candidate| candidate.track_id == source_track_id && candidate.likely_version);
        if !eligible {
            return Err(LibraryWorkerError::CreativeTimelineSourceUnavailable);
        }
        self.repository.resolve_track_version_candidate(
            target_track_id,
            source_track_id,
            expected_target_revision,
            "kept-separate",
            None,
        )?;
        Ok(())
    }

    pub const fn close_editor(&mut self) {
        self.editor_track_id = None;
    }

    pub fn preview_demo_source_refresh(&mut self) -> Result<(), LibraryWorkerError> {
        let baseline =
            DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2).load_baseline()?;
        if baseline.source_id().as_str() != self.source_id {
            return Err(LibraryWorkerError::SourceRefreshIdentityMismatch);
        }
        self.pending_source_refresh = Some(baseline);
        if self.pending_source_change_count()? == 0 {
            let baseline = self
                .pending_source_refresh
                .take()
                .ok_or(LibraryWorkerError::NoPendingSourceRefresh)?;
            self.repository.complete_source_refresh(&baseline)?;
            self.source_revision = baseline.source_revision().as_str().to_owned();
            self.source_name = baseline.display_name().to_owned();
        }
        Ok(())
    }

    pub fn reconcile_source_refresh(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        strategy: ReconcileStrategy,
    ) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.require_open_track(track_id)?;
        self.require_phrases_unlocked(track_id)?;
        let head = self.require_expected_head(track_id, expected_revision)?;
        let stored = self
            .repository
            .track(track_id)?
            .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
        let baseline = self
            .pending_source_refresh
            .as_ref()
            .ok_or(LibraryWorkerError::NoPendingSourceRefresh)?
            .clone();
        let incoming = baseline
            .tracks()
            .iter()
            .find(|track| track.source_track_id() == stored.summary().source_track_id())
            .ok_or(LibraryWorkerError::MissingIncomingTrack)?;
        let diff = SourceTrackDiff::between(&stored, incoming);
        if diff.is_metadata_only() {
            if !matches!(strategy, ReconcileStrategy::KeepLumi) {
                return Err(LibraryWorkerError::MetadataRefreshRequiresKeepLumi);
            }
            self.repository.refresh_track_without_timeline(
                &baseline,
                incoming,
                stored.summary().source_revision(),
            )?;
            if self.pending_source_change_count()? == 0 {
                self.repository.complete_source_refresh(&baseline)?;
                self.source_revision = baseline.source_revision().as_str().to_owned();
                self.source_name = baseline.display_name().to_owned();
                self.pending_source_refresh = None;
            }
            return Ok(());
        }
        let (source_total_beats, source_phrases) = self.map_source_phrases(incoming)?;
        let reconciled = reconcile_timeline(
            &head,
            incoming.analysis_revision().clone(),
            source_total_beats,
            &source_phrases,
            &strategy,
        )?;
        self.repository
            .reconcile_track(&baseline, incoming, &reconciled, head.revision())?;

        if self.pending_source_change_count()? == 0 {
            self.repository.complete_source_refresh(&baseline)?;
            self.source_revision = baseline.source_revision().as_str().to_owned();
            self.source_name = baseline.display_name().to_owned();
            self.pending_source_refresh = None;
        }
        Ok(())
    }

    pub fn mutate_phrase_role_catalog(
        &mut self,
        expected_revision: u64,
        mutation: PhraseRoleCatalogMutation,
    ) -> Result<(), LibraryWorkerError> {
        let catalog = self.repository.phrase_role_catalog()?;
        if catalog.revision() != expected_revision {
            return Err(LibraryWorkerError::PhraseRoleCatalogRevisionConflict {
                expected: expected_revision,
                actual: catalog.revision(),
            });
        }
        let updated = match mutation {
            PhraseRoleCatalogMutation::Add { display_name } => catalog.add_role(display_name)?,
            PhraseRoleCatalogMutation::Rename {
                role_id,
                display_name,
            } => catalog.rename_role(&role_id, display_name)?,
            PhraseRoleCatalogMutation::Move { role_id, direction } => {
                catalog.move_role(&role_id, direction)?
            }
            PhraseRoleCatalogMutation::SetArchived { role_id, archived } => {
                catalog.set_archived(&role_id, archived)?
            }
            PhraseRoleCatalogMutation::SetColor { role_id, color_rgb } => {
                catalog.set_color_rgb(&role_id, color_rgb)?
            }
            PhraseRoleCatalogMutation::SetSourceMapping {
                provider_kind,
                raw_label,
                role_id,
            } => {
                let role = catalog
                    .roles()
                    .iter()
                    .find(|role| role.id() == &role_id)
                    .ok_or(LibraryWorkerError::UnknownPhraseRole)?;
                if role.is_archived() {
                    return Err(LibraryWorkerError::ArchivedPhraseRole);
                }
                catalog.upsert_mapping(SourcePhraseMapping::try_new(
                    provider_kind,
                    raw_label,
                    role_id,
                )?)?
            }
        };
        self.repository
            .replace_phrase_role_catalog(&updated, expected_revision)?;
        Ok(())
    }

    pub fn mutate_autoloop_catalog(
        &mut self,
        expected_revision: u64,
        mutation: AutoloopCatalogMutation,
    ) -> Result<(), LibraryWorkerError> {
        let catalog = self.repository.autoloop_catalog()?;
        if catalog.revision() != expected_revision {
            return Err(LibraryWorkerError::AutoloopCatalogRevisionConflict {
                expected: expected_revision,
                actual: catalog.revision(),
            });
        }
        let updated = match mutation {
            AutoloopCatalogMutation::RenameTheme {
                theme_id,
                display_name,
            } => catalog.rename_theme(theme_id, display_name)?,
            AutoloopCatalogMutation::AddVariant {
                role_id,
                display_name,
            } => {
                self.require_active_role(&role_id)?;
                catalog.add_variant(role_id, display_name)?
            }
            AutoloopCatalogMutation::RenameVariant {
                role_id,
                variant_id,
                display_name,
            } => catalog.rename_variant(&role_id, &variant_id, display_name)?,
            AutoloopCatalogMutation::MoveVariant {
                role_id,
                variant_id,
                direction,
            } => catalog.move_variant(&role_id, &variant_id, direction)?,
            AutoloopCatalogMutation::SetVariantArchived {
                role_id,
                variant_id,
                archived,
            } => catalog.set_variant_archived(&role_id, &variant_id, archived)?,
            AutoloopCatalogMutation::SetCell {
                theme_id,
                role_id,
                variant_id,
                display_name,
            } => catalog.set_cell(theme_id, &role_id, &variant_id, display_name)?,
            AutoloopCatalogMutation::SetButton {
                theme_id,
                button_number,
                role_id,
                display_name,
            } => {
                self.require_active_role(&role_id)?;
                if !(1..=32).contains(&button_number) {
                    return Err(AutoloopCatalogError::IdentifierOverflow.into());
                }
                catalog.set_mapping(
                    theme_id,
                    VariantId::try_new(format!("mapping-{button_number}"))?,
                    role_id,
                    display_name,
                )?
            }
            AutoloopCatalogMutation::ClearButton {
                theme_id,
                button_number,
            } => {
                if !(1..=32).contains(&button_number) {
                    return Err(AutoloopCatalogError::IdentifierOverflow.into());
                }
                catalog.clear_mapping(
                    theme_id,
                    &VariantId::try_new(format!("mapping-{button_number}"))?,
                )?
            }
        };
        let roles = self.repository.phrase_role_catalog()?;
        updated.validate_roles(&roles)?;
        self.repository
            .replace_autoloop_catalog(&updated, expected_revision)?;
        Ok(())
    }

    fn require_active_role(&self, role_id: &PhraseRoleId) -> Result<(), LibraryWorkerError> {
        let catalog = self.repository.phrase_role_catalog()?;
        let role = catalog
            .roles()
            .iter()
            .find(|role| role.id() == role_id)
            .ok_or(LibraryWorkerError::UnknownPhraseRole)?;
        if role.is_archived() {
            return Err(LibraryWorkerError::ArchivedPhraseRole);
        }
        Ok(())
    }

    fn require_open_track(&self, track_id: TrackId) -> Result<(), LibraryWorkerError> {
        if self.editor_track_id != Some(track_id) {
            return Err(LibraryWorkerError::EditorTrackMismatch);
        }
        Ok(())
    }

    fn require_phrases_unlocked(&self, track_id: TrackId) -> Result<(), LibraryWorkerError> {
        let protection = self
            .repository
            .track_phrase_protections(&[track_id])?
            .remove(&track_id)
            .unwrap_or_default();
        if protection.locked {
            return Err(LibraryWorkerError::TrackPhrasesProtected);
        }
        Ok(())
    }

    pub fn set_track_phrase_protection(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        locked: bool,
    ) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.require_open_track(track_id)?;
        self.repository
            .set_track_phrase_protection(track_id, expected_revision, locked)?;
        Ok(())
    }

    fn require_expected_head(
        &self,
        track_id: TrackId,
        expected_revision: u64,
    ) -> Result<LumiPhraseTimeline, LibraryWorkerError> {
        let expected = TimelineRevision::try_new(expected_revision)
            .map_err(|_| LibraryWorkerError::InvalidTimelineRevision(expected_revision))?;
        let head = self
            .repository
            .timeline_head(track_id)?
            .ok_or(LibraryWorkerError::MissingTimeline)?;
        if head.revision() != expected {
            return Err(LibraryWorkerError::TimelineRevisionConflict {
                expected,
                actual: head.revision(),
            });
        }
        Ok(head)
    }

    fn ensure_timeline(&mut self, track_id: TrackId) -> Result<(), LibraryWorkerError> {
        let track = self
            .repository
            .track(track_id)?
            .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
        if self.repository.timeline_head(track_id)?.is_some() {
            return Ok(());
        }
        let (total_beats, phrases) = self.map_source_phrases_from_parts(
            track.beat_grid().beats_per_bar(),
            track.beat_grid().markers().len(),
            track.raw_phrases(),
        )?;
        let timeline = LumiPhraseTimeline::try_new_with_history(
            track_id,
            TimelineRevision::initial(),
            SourceRevision::try_new(track.summary().source_revision().as_str())?,
            total_beats,
            TimelineRevisionOrigin::SourceImport,
            TimelineRevisionReason::InitialSourceMapping,
            None,
            None,
            phrases,
        )?;
        self.repository.append_timeline_revision(&timeline, None)?;
        Ok(())
    }

    fn ensure_imported_timelines(&mut self) -> Result<(), LibraryWorkerError> {
        loop {
            let track_ids = self.repository.track_ids_missing_timelines(200)?;
            if track_ids.is_empty() {
                break;
            }
            for track_id in track_ids {
                self.ensure_timeline(track_id)?;
            }
        }
        Ok(())
    }

    /// Applies newly added provider defaults only to pristine source-import
    /// timelines. Any track with a user or reconciliation revision is left
    /// untouched, so a defaults upgrade can never overwrite editing work.
    fn remap_untouched_source_timelines(&mut self) -> Result<(), LibraryWorkerError> {
        let mut offset = 0_u32;
        loop {
            let page = self
                .repository
                .page_tracks(TrackPageRequest::try_new(offset, 200)?)?;
            if page.tracks().is_empty() {
                break;
            }
            let track_ids = page
                .tracks()
                .iter()
                .map(TrackSummary::id)
                .collect::<Vec<_>>();
            for track_id in track_ids {
                let Some(head) = self.repository.timeline_head(track_id)? else {
                    continue;
                };
                if head.revision() != TimelineRevision::initial()
                    || head.origin() != TimelineRevisionOrigin::SourceImport
                    || head.reason() != TimelineRevisionReason::InitialSourceMapping
                {
                    continue;
                }
                let track = self
                    .repository
                    .track(track_id)?
                    .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
                let (total_beats, phrases) = self.map_source_phrases_from_parts(
                    track.beat_grid().beats_per_bar(),
                    track.beat_grid().markers().len(),
                    track.raw_phrases(),
                )?;
                if head.phrases() == phrases {
                    continue;
                }
                let revision = head
                    .revision()
                    .checked_next()
                    .ok_or(LibraryWorkerError::HistoryOverflow)?;
                let remapped = LumiPhraseTimeline::try_new_with_history(
                    track_id,
                    revision,
                    head.baseline_revision().clone(),
                    total_beats,
                    TimelineRevisionOrigin::SourceReconcile,
                    TimelineRevisionReason::SourceReconcile,
                    Some(head.revision()),
                    None,
                    phrases,
                )?;
                self.repository
                    .append_timeline_revision(&remapped, Some(head.revision()))?;
            }
            let consumed = u32::try_from(page.tracks().len())
                .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?;
            offset = offset
                .checked_add(consumed)
                .ok_or(LibraryWorkerError::RekordboxImportOverflow)?;
            if u64::from(offset) >= page.total() {
                break;
            }
        }
        Ok(())
    }

    fn map_source_phrases(
        &self,
        track: &ImportedTrackAnalysis,
    ) -> Result<(u32, Vec<PhraseInstance>), LibraryWorkerError> {
        self.map_source_phrases_from_parts(
            track.beat_grid().beats_per_bar(),
            track.beat_grid().markers().len(),
            track.raw_phrases(),
        )
    }

    fn map_source_phrases_from_parts(
        &self,
        beats_per_bar: u8,
        marker_count: usize,
        raw_phrases: &[lumi_library::RawPhraseObservation],
    ) -> Result<(u32, Vec<PhraseInstance>), LibraryWorkerError> {
        let beats_per_bar = u32::from(beats_per_bar);
        let total_beats =
            u32::try_from(marker_count).map_err(|_| LibraryWorkerError::InvalidSourceTimeline)?;
        if total_beats == 0 || !total_beats.is_multiple_of(beats_per_bar) {
            return Err(LibraryWorkerError::InvalidSourceTimeline);
        }
        let role_catalog = self.repository.phrase_role_catalog()?;
        let mut phrases = Vec::with_capacity(raw_phrases.len());
        for (index, phrase) in raw_phrases.iter().enumerate() {
            let role_id = role_catalog
                .resolve(&self.source_kind, phrase.source_label())
                .cloned()
                .ok_or_else(|| LibraryWorkerError::UnmappedSourcePhrase {
                    provider_kind: self.source_kind.clone(),
                    raw_label: phrase.source_label().to_owned(),
                })?;
            if !role_catalog
                .roles()
                .iter()
                .any(|role| role.id() == &role_id && !role.is_archived())
            {
                return Err(LibraryWorkerError::ArchivedSourcePhraseMapping {
                    provider_kind: self.source_kind.clone(),
                    raw_label: phrase.source_label().to_owned(),
                    role_id,
                });
            }
            phrases.push(PhraseInstance::new(
                u16::try_from(index).map_err(|_| LibraryWorkerError::InvalidSourceTimeline)?,
                phrase.start_beat(),
                phrase.end_beat(),
                role_id,
            ));
        }
        Ok((total_beats, phrases))
    }

    fn pending_source_change_count(&self) -> Result<usize, LibraryWorkerError> {
        let Some(baseline) = &self.pending_source_refresh else {
            return Ok(0);
        };
        let page = self
            .repository
            .page_tracks(TrackPageRequest::try_new(0, 200)?)?;
        let mut count = 0;
        for stored_summary in page.tracks() {
            let Some(incoming) = baseline
                .tracks()
                .iter()
                .find(|track| track.source_track_id() == stored_summary.source_track_id())
            else {
                continue;
            };
            let stored = self.repository.track(stored_summary.id())?.ok_or(
                LibraryWorkerError::UnknownTrack(stored_summary.id().value()),
            )?;
            if !SourceTrackDiff::between(&stored, incoming)
                .changes()
                .is_empty()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    fn restore_from_history(
        &mut self,
        track_id: u64,
        expected_revision: u64,
        action: HistoryAction,
    ) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.require_open_track(track_id)?;
        self.require_phrases_unlocked(track_id)?;
        let head = self.require_expected_head(track_id, expected_revision)?;
        let history = self.rebuild_history(track_id)?;
        let target_revision = match action {
            HistoryAction::Undo => history.undo.last().copied(),
            HistoryAction::Redo => history.redo.last().copied(),
        }
        .ok_or(match action {
            HistoryAction::Undo => LibraryWorkerError::NothingToUndo,
            HistoryAction::Redo => LibraryWorkerError::NothingToRedo,
        })?;
        let target = self
            .repository
            .timeline_revision(track_id, target_revision)?
            .ok_or(LibraryWorkerError::UnknownTimelineRevision(
                target_revision.value(),
            ))?;
        let reason = match action {
            HistoryAction::Undo => TimelineRevisionReason::Undo,
            HistoryAction::Redo => TimelineRevisionReason::Redo,
        };
        let restored = LumiPhraseTimeline::restore(&head, &target, reason)?;
        self.repository
            .append_timeline_revision(&restored, Some(head.revision()))?;
        Ok(())
    }

    fn rebuild_history(&self, track_id: TrackId) -> Result<TimelineHistory, LibraryWorkerError> {
        let mut offset = 0_u32;
        let mut summaries = Vec::new();
        loop {
            let page = self
                .repository
                .timeline_revisions(track_id, TrackPageRequest::try_new(offset, 200)?)?;
            summaries.extend_from_slice(page.revisions());
            let consumed = u32::try_from(page.revisions().len())
                .map_err(|_| LibraryWorkerError::HistoryOverflow)?;
            offset = offset
                .checked_add(consumed)
                .ok_or(LibraryWorkerError::HistoryOverflow)?;
            if u64::from(offset) >= page.total() || consumed == 0 {
                break;
            }
        }
        summaries.reverse();
        TimelineHistory::replay(&summaries)
    }

    pub fn snapshot_json(&self) -> Result<Value, LibraryWorkerError> {
        self.snapshot_json_with_device_inspection(true)
    }

    pub fn status_snapshot_json(&self) -> Result<Value, LibraryWorkerError> {
        self.snapshot_json_with_device_inspection(false)
    }

    fn snapshot_json_with_device_inspection(
        &self,
        include_device_inspection: bool,
    ) -> Result<Value, LibraryWorkerError> {
        let request = TrackPageRequest::try_new(self.offset, self.limit)?;
        let query = LibraryTrackQuery::try_new_sorted(
            self.search.clone(),
            self.playlist_id,
            request,
            self.sort,
        )?
        .with_workflow_filter(self.workflow_filter)
        .with_workflow_step_id(self.workflow_step_id.clone());
        let page = self.repository.query_tracks(&query)?;
        let workflow_states = self.repository.track_workflow_states(
            &page
                .tracks()
                .iter()
                .map(TrackSummary::id)
                .collect::<Vec<_>>(),
        )?;
        let phrase_protections = self.repository.track_phrase_protections(
            &page
                .tracks()
                .iter()
                .map(TrackSummary::id)
                .collect::<Vec<_>>(),
        )?;
        let workflow_summary = self.repository.track_workflow_summary()?;
        let workflow_catalog = self.repository.track_workflow_catalog()?;
        let device_source_relations = self.repository.device_track_source_relations(
            &page
                .tracks()
                .iter()
                .map(TrackSummary::id)
                .collect::<Vec<_>>(),
        )?;
        let collection_total = self
            .repository
            .query_tracks(&LibraryTrackQuery::try_new(
                "",
                None,
                TrackPageRequest::try_new(0, 1)?,
            )?)?
            .total();
        let playlist_page = self
            .repository
            .page_playlists(TrackPageRequest::try_new(0, 200)?)?;
        let device_sources = self.repository.device_source_summaries()?;
        let device_review_tracks = self.repository.device_review_tracks()?;
        let stored_device_playlists = self.repository.stored_device_playlists()?;
        let data_summary = self.repository.data_summary()?;
        let reset_candidates = self.repository.reset_preservable_tracks()?;
        let creative_archives = self.repository.creative_archives()?;
        let light_planning_policy = self.repository.light_planning_policy()?;
        let track_color_counts = self
            .repository
            .track_color_summaries()?
            .into_iter()
            .map(|color| (color.color_rgb, color.track_count))
            .collect::<BTreeMap<_, _>>();
        let source_refresh = match &self.pending_source_refresh {
            Some(baseline) => json!({
                "revision": baseline.source_revision().as_str(),
                "changeCount": self.pending_source_change_count()?,
            }),
            None => Value::Null,
        };
        Ok(json!({
            "condition": if collection_total == 0 {
                "empty"
            } else {
                "ready"
            },
            "providerKind": self.source_kind,
            "source": {
                "id": self.source_id,
                "name": self.source_name,
                "revision": self.source_revision,
                "status": if self.pending_source_refresh.is_some() { "changesAvailable" } else { "current" },
            },
            "sourceRefresh": source_refresh,
            "rekordboxSyncPreview": self.rekordbox_sync_preview_json(),
            "rekordboxMirror": self.rekordbox_mirror_json()?,
            "rekordboxDeviceInspection": if include_device_inspection {
                self.rekordbox_device_inspection_json()
            } else {
                Value::Null
            },
            "rekordboxDevices": device_sources.iter().map(|source| {
                let playlists = stored_device_playlists
                    .get(&source.source_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let review_tracks = device_review_tracks
                    .get(&source.source_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let review_comparisons = self
                    .device_review_comparisons_by_source
                    .get(&source.source_id);
                json!({
                    "sourceId": source.source_id,
                    "displayName": source.display_name,
                    "databaseRevision": source.database_revision,
                    "activeTracks": source.active_tracks,
                    "matchedTracks": source.matched_tracks,
                    "unmatchedTracks": source.active_tracks.saturating_sub(source.matched_tracks),
                    "syncedAt": source.synced_at,
                    "trustState": "trusted",
                    "currentTracks": source.current_tracks,
                    "promotedTracks": source.promoted_tracks,
                    "protectedTracks": source.protected_tracks,
                    "conflictTracks": source.conflict_tracks,
                    "beatGridRefresh": true,
                    "cueRevisionTracked": true,
                    "reviewTracks": review_tracks.iter().take(200).map(|track| {
                        let active_source = track.active_source_name
                            .as_deref()
                            .unwrap_or("the active Lumi analysis");
                        let reason = match track.active_analyzed_at.as_deref() {
                            Some(active_date) if active_date == track.incoming_analyzed_at => format!(
                                "This USB analysis differs from {active_source}. Both are dated {active_date}, so Lumi cannot safely determine which is newer and kept the active analysis."
                            ),
                            Some(active_date) => format!(
                                "This USB analysis ({}) differs from {active_source} ({active_date}), but the revisions could not be ordered safely. Lumi kept the active analysis.",
                                track.incoming_analyzed_at
                            ),
                            None => format!(
                                "This USB analysis differs from {active_source}, but comparable provenance is unavailable. Lumi kept the active analysis."
                            ),
                        };
                        let comparison = review_comparisons
                            .and_then(|items| items.get(&track.device_track_id));
                        json!({
                            "deviceTrackId": track.device_track_id,
                            "canonicalTrackId": track.canonical_track_id.map(TrackId::value),
                            "title": track.title,
                            "artist": track.artist,
                            "bpmMilli": track.bpm_milli,
                            "durationMillis": track.duration_millis,
                            "incomingAnalyzedAt": track.incoming_analyzed_at,
                            "activeAnalyzedAt": track.active_analyzed_at,
                            "activeSourceName": track.active_source_name,
                            "incomingAnalysisRevision": track.incoming_analysis_revision,
                            "activeAnalysisRevision": track.active_analysis_revision,
                            "incomingMetadataRevision": track.incoming_metadata_revision,
                            "incomingFileSize": track.incoming_file_size,
                            "reason": reason,
                            "components": comparison.map(review_comparison_json),
                        })
                    }).collect::<Vec<_>>(),
                    "playlists": playlists.iter().map(|playlist| json!({
                        "id": playlist.device_playlist_id,
                        "libraryPlaylistId": playlist.playlist_id.value(),
                        "name": playlist.name,
                        "trackCount": playlist.track_count,
                    })).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "capabilities": {
                "playlists": true,
                "color": true,
                "beatGrid": true,
                "waveform": true,
                "rawPhrases": true,
                "localAudio": true,
            },
            "lightPlanning": {
                "policy": light_planning_policy,
                "trackColors": REKORDBOX_TRACK_COLORS.iter().map(|(name, rgb)| json!({
                    "rgb": rgb,
                    "name": name,
                    "trackCount": track_color_counts.get(rgb).copied().unwrap_or(0),
                })).collect::<Vec<_>>(),
                "preview": self.pending_light_plan_preview,
                "execution": {
                    "compiledBeforePlayback": true,
                    "realtimePolicyEvaluation": false,
                    "staticLookOutput": "verifiedAutomatic",
                    "colorOverrideOutput": "pocRequired",
                },
            },
            "dataManagement": {
                "trackCount": data_summary.track_count,
                "playlistCount": data_summary.playlist_count,
                "userEditedTrackCount": data_summary.user_edited_track_count,
                "creativeArchiveCount": data_summary.creative_archive_count,
                "pendingArchiveCount": data_summary.pending_archive_count,
                "resetCandidates": reset_candidates.iter().map(|track| json!({
                    "trackId": track.track_id.value(),
                    "title": track.title,
                    "artist": track.artist,
                    "timelineRevision": track.timeline_revision,
                })).collect::<Vec<_>>(),
                "creativeArchives": creative_archives.iter().map(|archive| json!({
                    "archiveId": archive.archive_id,
                    "title": archive.title,
                    "artist": archive.artist,
                    "phraseCount": archive.phrase_count,
                    "totalBeats": archive.total_beats,
                    "state": archive.state,
                    "restoredTrackId": archive.restored_track_id.map(|track_id| track_id.value()),
                })).collect::<Vec<_>>(),
                "resetPreview": self.pending_library_reset.as_ref().map(|preview| json!({
                    "token": preview.token,
                    "trackCount": preview.impact.track_count,
                    "playlistCount": preview.impact.playlist_count,
                    "preservedTrackCount": preview.impact.preserved_track_count,
                    "removedTrackCount": preview.impact.removed_track_count,
                    "archivedCreativeTrackCount": preview.impact.archived_creative_track_count,
                    "preserveTrackIds": preview.preserve_track_ids.iter()
                        .map(|track_id| track_id.value()).collect::<Vec<_>>(),
                })),
            },
            "collectionTotal": collection_total,
            "workflow": {
                "changedAfterUsbSync": workflow_summary.changed_after_usb_sync,
                "versionCandidates": workflow_summary.version_candidates,
                "notStarted": workflow_summary.not_started,
                "inProgress": workflow_summary.in_progress,
                "readyForShow": workflow_summary.ready_for_show,
                "catalogRevision": workflow_summary.catalog_revision,
                "stepCounts": workflow_summary.step_counts,
            },
            "workflowCatalog": {
                "revision": workflow_catalog.revision(),
                "steps": workflow_catalog.steps().iter().map(|step| json!({
                    "id": step.id(),
                    "displayName": step.display_name(),
                    "icon": step.icon(),
                    "colorRgb": step.color_rgb(),
                    "sortOrder": step.sort_order(),
                    "archived": step.archived(),
                    "rules": step.rules().iter().map(|rule| json!({
                        "field": rule.field().as_str(),
                        "operator": rule.operator().as_str(),
                        "value": rule.value(),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            },
            "query": {
                "search": self.search,
                "playlistId": self.playlist_id.map(|id| id.value()),
                "offset": page.offset(),
                "limit": self.limit,
                "sortBy": library_sort_field_name(self.sort.field()),
                "sortDirection": library_sort_direction_name(self.sort.direction()),
                "workflowFilter": self.workflow_filter.map(TrackWorkflowFilter::as_str),
                "workflowStepId": self.workflow_step_id,
            },
            "playlists": playlist_page.playlists().iter().map(|playlist| json!({
                "id": playlist.id().value(),
                "sourcePlaylistId": playlist.source_playlist_id().as_str(),
                "name": playlist.name(),
                "trackCount": playlist.track_count(),
            })).collect::<Vec<_>>(),
            "page": {
                "total": page.total(),
                "offset": page.offset(),
                "tracks": page.tracks().iter().map(|track| {
                    track_json_with_device_sources(
                        track,
                        device_source_relations.get(&track.id()).map(Vec::as_slice).unwrap_or(&[]),
                        workflow_states.get(&track.id()),
                        phrase_protections.get(&track.id()).copied().unwrap_or_default(),
                    )
                }).collect::<Vec<_>>(),
            },
            "phraseRoleSettings": self.phrase_role_settings_json()?,
            "autoloopCatalog": self.autoloop_catalog_json()?,
            "editor": self.editor_json()?,
        }))
    }

    fn rekordbox_sync_preview_json(&self) -> Value {
        let Some(preview) = &self.pending_rekordbox_preview else {
            return Value::Null;
        };
        let diagnostics = preview.diagnostics();
        let diff = self.pending_rekordbox_diff.unwrap_or_default();
        json!({
            "exportFileName": preview.export_path().file_name().and_then(|name| name.to_str()),
            "contentSha256": preview.content_sha256(),
            "productVersion": preview.product_version(),
            "collectionTrackCount": preview.collection_track_count(),
            "followedPlaylistCount": preview.playlists().len(),
            "uniqueTrackCount": preview.tracks().len(),
            "selectionPaths": preview.selection_paths(),
            "includeFutureChildPlaylists": preview.include_future_child_playlists(),
            "playlists": preview.playlists().iter().map(|playlist| json!({
                "path": playlist.path(),
                "name": playlist.name(),
                "trackCount": playlist.track_ids().len(),
            })).collect::<Vec<_>>(),
            "diagnostics": {
                "duplicatePlaylistReferences": diagnostics.duplicate_playlist_references,
                "missingArtist": diagnostics.missing_artist,
                "missingBpm": diagnostics.missing_bpm,
                "missingKey": diagnostics.missing_key,
                "missingDuration": diagnostics.missing_duration,
                "missingBeatGrid": diagnostics.missing_beat_grid,
                "missingColour": diagnostics.missing_colour,
                "missingWaveform": diagnostics.missing_waveform,
                "missingPhrases": diagnostics.missing_phrases,
            },
            "diff": {
                "inserted": diff.inserted,
                "updated": diff.updated,
                "unchanged": diff.unchanged,
                "archived": diff.archived,
                "restored": diff.restored,
            },
            "applyState": if self.last_rekordbox_apply.is_some() { "applied" } else { "ready" },
        })
    }

    fn rekordbox_device_inspection_json(&self) -> Value {
        let Some(inspection) = &self.pending_device_inspection else {
            return Value::Null;
        };
        let device = &inspection.snapshot;
        json!({
            "sourceId": device.source_id,
            "displayName": device.display_name,
            "databaseRevision": device.database_revision,
            "libraryFormat": "OneLibrary",
            "databaseVersion": device.database_version,
            "exportedAt": device.exported_at,
            "trackCount": device.tracks.len(),
            "playlistCount": device.playlists.len(),
            "selectedPlaylistIds": inspection.selected_playlist_ids,
            "playlists": device.playlists.iter().map(|playlist| json!({
                "id": playlist.device_playlist_id,
                "path": playlist.path,
                "folderNames": playlist.folder_names,
                "name": playlist.name,
                "trackCount": playlist.track_ids.len(),
                "statusCounts": device_status_counts(
                    playlist.track_ids.iter().filter_map(|track_id| inspection.tracks.get(track_id))
                ),
                "tracks": playlist.track_ids.iter().filter_map(|track_id| {
                    let track = device.tracks.get(track_id)?;
                    let inspection_track = inspection.tracks.get(track_id)?;
                    Some(json!({
                        "id": track.device_track_id,
                        "title": track.title,
                        "artist": track.artist,
                        "bpmMilli": track.bpm_milli,
                        "durationMillis": track.duration_millis,
                        "status": inspection_track.status,
                        "detail": inspection_track.detail,
                    }))
                }).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }

    fn rekordbox_mirror_json(&self) -> Result<Value, LibraryWorkerError> {
        let source_id = lumi_library::LibrarySourceId::try_new(REKORDBOX_XML_SOURCE_ID)?;
        let Some(summary) = self.repository.source_mirror_summary(&source_id)? else {
            return Ok(Value::Null);
        };
        Ok(json!({
            "sourceId": summary.source_id().as_str(),
            "sourceKind": summary.source_kind(),
            "displayName": summary.display_name(),
            "revision": summary.source_revision().as_str(),
            "activeTracks": summary.active_tracks(),
            "archivedTracks": summary.archived_tracks(),
            "playlists": summary.playlists(),
            "analysisState": "pending",
            "lastApply": self.last_rekordbox_apply.map(|diff| json!({
                "inserted": diff.inserted,
                "updated": diff.updated,
                "unchanged": diff.unchanged,
                "archived": diff.archived,
                "restored": diff.restored,
            })),
        }))
    }

    fn phrase_role_settings_json(&self) -> Result<Value, LibraryWorkerError> {
        let catalog = self.repository.phrase_role_catalog()?;
        let usages = self
            .repository
            .phrase_role_usages()?
            .into_iter()
            .map(|usage| (usage.role_id().clone(), usage))
            .collect::<BTreeMap<_, _>>();
        let catalog_rows = self.repository.autoloop_catalog()?.variants().iter().fold(
            BTreeMap::<PhraseRoleId, u64>::new(),
            |mut rows, variant| {
                *rows.entry(variant.role_id().clone()).or_default() += 1;
                rows
            },
        );
        let mut profiles = BTreeMap::<String, Vec<&SourcePhraseMapping>>::new();
        for mapping in catalog.mappings() {
            profiles
                .entry(mapping.provider_kind().to_owned())
                .or_default()
                .push(mapping);
        }
        for mappings in profiles.values_mut() {
            mappings.sort_by(|left, right| {
                (left.raw_label() == "*")
                    .cmp(&(right.raw_label() == "*"))
                    .then_with(|| left.raw_label().cmp(right.raw_label()))
            });
        }
        Ok(json!({
            "revision": catalog.revision(),
            "defaultsVersion": catalog.defaults_version(),
            "roles": catalog.roles().iter().map(|role| {
                let usage = usages.get(role.id());
                let affected_tracks = usage.map_or(&[][..], |value| value.tracks());
                json!({
                    "id": role.id().as_str(),
                    "name": role.display_name(),
                    "sortOrder": role.sort_order(),
                    "archived": role.is_archived(),
                    "colorRgb": role.color_rgb(),
                    "usage": {
                        "phraseCount": usage.map_or(0, |value| value.phrase_count()),
                        "trackCount": affected_tracks.len(),
                        "catalogRowCount": catalog_rows.get(role.id()).copied().unwrap_or(0),
                        "affectedTracks": affected_tracks.iter().take(100).map(|track| json!({
                            "trackId": track.track_id().value(),
                            "title": track.title(),
                            "phraseCount": track.phrase_count(),
                        })).collect::<Vec<_>>(),
                        "hasMoreAffectedTracks": affected_tracks.len() > 100,
                    },
                })
            }).collect::<Vec<_>>(),
            "mappingProfiles": profiles.into_iter().map(|(provider_kind, mappings)| json!({
                "providerKind": provider_kind,
                "providerName": provider_display_name(&provider_kind),
                "mappings": mappings.into_iter().map(|mapping| json!({
                    "rawLabel": mapping.raw_label(),
                    "roleId": mapping.role_id().as_str(),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "mappingPolicy": "futureInitialTimelinesOnly",
        }))
    }

    fn autoloop_catalog_json(&self) -> Result<Value, LibraryWorkerError> {
        let catalog = self.repository.autoloop_catalog()?;
        let phrase_roles = self.repository.phrase_role_catalog()?;
        let missing = catalog.missing_cells();
        let missing_roles = phrase_roles
            .roles()
            .iter()
            .filter(|role| {
                !role.is_archived()
                    && !catalog
                        .variants()
                        .iter()
                        .any(|variant| variant.role_id() == role.id() && !variant.is_archived())
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "revision": catalog.revision(),
            "defaultsVersion": catalog.defaults_version(),
            "themes": catalog.themes().iter().map(|theme| json!({
                "id": theme.id().value(),
                "name": theme.display_name(),
                "sortOrder": theme.sort_order(),
            })).collect::<Vec<_>>(),
            "roles": phrase_roles.roles().iter().map(|role| json!({
                "id": role.id().as_str(),
                "name": role.display_name(),
                "archived": role.is_archived(),
                "colorRgb": role.color_rgb(),
                "variants": catalog.variants().iter()
                    .filter(|variant| variant.role_id() == role.id())
                    .map(|variant| json!({
                        "id": variant.id().as_str(),
                        "name": variant.display_name(),
                        "sortOrder": variant.sort_order(),
                        "archived": variant.is_archived(),
                        "cells": catalog.themes().iter().map(|theme| {
                            let cell = catalog.cells().iter().find(|cell| {
                                cell.theme_id() == theme.id()
                                    && cell.role_id() == role.id()
                                    && cell.variant_id() == variant.id()
                            });
                            json!({
                                "themeId": theme.id().value(),
                                "buttonNumber": cell.and_then(|value| {
                                    value.variant_id().as_str().strip_prefix("mapping-")?.parse::<u16>().ok()
                                }),
                                "entryId": cell.map(|value| value.entry_id().as_str()),
                                "name": cell.map(|value| value.display_name()),
                                "status": if cell.is_some() { "ready" } else { "missing" },
                            })
                        }).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "preflight": {
                "status": if missing.is_empty() && missing_roles.is_empty() { "ready" } else { "incomplete" },
                "missingCellCount": missing.len(),
                "missingCells": missing.iter().take(200).map(|cell| json!({
                    "themeId": cell.theme_id().value(),
                    "roleId": cell.role_id().as_str(),
                    "variantId": cell.variant_id().as_str(),
                })).collect::<Vec<_>>(),
                "hasMoreMissingCells": missing.len() > 200,
                "missingRoleCount": missing_roles.len(),
                "missingRoleIds": missing_roles.iter().take(200).map(|role| role.id().as_str()).collect::<Vec<_>>(),
                "hasMoreMissingRoles": missing_roles.len() > 200,
            },
            "targetCapabilities": {
                "validationOwner": "targetAdapter",
                "hardCodedPhysicalCapacity": false,
            },
        }))
    }

    fn editor_json(&self) -> Result<Value, LibraryWorkerError> {
        let Some(track_id) = self.editor_track_id else {
            return Ok(Value::Null);
        };
        let track = self
            .repository
            .track(track_id)?
            .ok_or(LibraryWorkerError::UnknownTrack(track_id.value()))?;
        let timeline = self
            .repository
            .timeline_head(track_id)?
            .ok_or(LibraryWorkerError::MissingTimeline)?;
        let role_catalog = self.repository.phrase_role_catalog()?;
        let roles = role_catalog.roles();
        let autoloop_catalog = self.repository.autoloop_catalog()?;
        let history = self.rebuild_history(track_id)?;
        let revisions = self
            .repository
            .timeline_revisions(track_id, TrackPageRequest::try_new(0, 200)?)?;
        let audio_uri = self.resolved_audio_uri(&track)?;
        let creative_reuse_candidates = self.repository.creative_timeline_candidates(track_id)?;
        Ok(json!({
            "track": track_json_with_device_sources(
                track.summary(),
                &[],
                self.repository.track_workflow_states(&[track_id])?.get(&track_id),
                self.repository
                    .track_phrase_protections(&[track_id])?
                    .get(&track_id)
                    .copied()
                    .unwrap_or_default(),
            ),
            "audioUri": audio_uri,
            "beatGrid": {
                "beatsPerBar": track.beat_grid().beats_per_bar(),
                "markers": track.beat_grid().markers().iter().map(|marker| json!({
                    "beatIndex": marker.beat_index(),
                    "timeMillis": marker.time_millis(),
                    "barIndex": marker.bar_index(),
                    "beatInBar": marker.beat_in_bar(),
                })).collect::<Vec<_>>(),
            },
            "waveform": track.waveform().iter().map(|point| json!({
                "low": point.low(),
                "mid": point.mid(),
                "high": point.high(),
            })).collect::<Vec<_>>(),
            "hotCues": track.hot_cues().iter().map(|cue| json!({
                "index": cue.index(),
                "timeMillis": cue.time_millis(),
                "loopEndMillis": cue.loop_end_millis(),
                "name": cue.name(),
                "colorRgb": cue.color_rgb(),
            })).collect::<Vec<_>>(),
            "timeline": {
                "revision": timeline.revision().value(),
                "baselineRevision": timeline.baseline_revision().as_str(),
                "origin": origin_name(timeline.origin()),
                "reason": reason_name(timeline.reason()),
                "canUndo": !history.undo.is_empty(),
                "canRedo": !history.redo.is_empty(),
                "revisions": revisions.revisions().iter().map(|revision| json!({
                    "revision": revision.revision().value(),
                    "origin": origin_name(revision.origin()),
                    "reason": reason_name(revision.reason()),
                    "phraseCount": revision.phrase_count(),
                    "restoredFrom": revision.restored_from().map(|value| value.value()),
                })).collect::<Vec<_>>(),
            },
            "roles": roles.iter().map(|role| json!({
                "id": role.id().as_str(),
                "name": role.display_name(),
                "archived": role.is_archived(),
            })).collect::<Vec<_>>(),
            "sourcePhrases": track.raw_phrases().iter().map(|phrase| json!({
                "startBeat": phrase.start_beat(),
                "endBeat": phrase.end_beat(),
                "rawLabel": phrase.source_label(),
                "providerKind": self.source_kind,
            })).collect::<Vec<_>>(),
            "phrases": timeline.phrases().iter().map(|phrase| json!({
                "id": phrase.index(),
                "startBeat": phrase.start_beat(),
                "endBeat": phrase.end_beat(),
                "roleId": phrase.role_id().as_str(),
                "role": role_display_name(roles, phrase.role_id()),
                "origin": origin_name(timeline.origin()),
                "loopStrategy": loop_strategy_json(&autoloop_catalog, phrase),
            })).collect::<Vec<_>>(),
            "sourceReconciliation": self.source_reconciliation_json(&track, &timeline)?,
            "creativeReuseCandidates": creative_reuse_candidates.iter().map(|candidate| json!({
                "trackId": candidate.track_id.value(),
                "title": candidate.title,
                "artist": candidate.artist,
                "phraseCount": candidate.phrase_count,
                "totalBeats": candidate.total_beats,
                "exactBeatCompatibility": candidate.exact_beat_compatibility,
                "likelyVersion": candidate.likely_version,
                "timelineRevision": candidate.timeline_revision,
                "bpmMilli": candidate.bpm_milli,
                "durationMillis": candidate.duration_millis,
                "bpmDeltaMilli": candidate.bpm_delta_milli,
                "durationDeltaMillis": candidate.duration_delta_millis,
            })).collect::<Vec<_>>(),
        }))
    }

    fn resolved_audio_uri(
        &self,
        track: &lumi_library::StoredTrack,
    ) -> Result<String, LibraryWorkerError> {
        let candidates = self.repository.device_audio_uris(track.summary().id())?;
        Ok(first_available_audio_uri(track.audio_uri(), &candidates))
    }

    fn source_reconciliation_json(
        &self,
        track: &lumi_library::StoredTrack,
        timeline: &LumiPhraseTimeline,
    ) -> Result<Value, LibraryWorkerError> {
        let Some(baseline) = &self.pending_source_refresh else {
            return Ok(Value::Null);
        };
        let Some(incoming) = baseline
            .tracks()
            .iter()
            .find(|candidate| candidate.source_track_id() == track.summary().source_track_id())
        else {
            return Ok(Value::Null);
        };
        let diff = SourceTrackDiff::between(track, incoming);
        if diff.changes().is_empty() {
            return Ok(Value::Null);
        }
        let (source_total_beats, source_phrases) = self.map_source_phrases(incoming)?;
        let preview = ReconcilePreview::between(timeline, &source_phrases, source_total_beats);
        Ok(json!({
            "fromRevision": diff.from_revision().as_str(),
            "toRevision": diff.to_revision().as_str(),
            "sourceLibraryRevision": baseline.source_revision().as_str(),
            "metadataOnly": diff.is_metadata_only(),
            "requiresTimelineDecision": diff.requires_timeline_decision(),
            "changes": diff.changes().iter().map(|change| match change {
                SourceChangeClass::Metadata => "metadata",
                SourceChangeClass::Waveform => "waveform",
                SourceChangeClass::BeatGrid => "beatGrid",
                SourceChangeClass::RawPhrases => "rawPhrases",
            }).collect::<Vec<_>>(),
            "sourceTotalBeats": source_total_beats,
            "rebaseAmbiguities": preview.rebase_ambiguities(),
            "conflicts": preview.conflicts().iter().map(|conflict| json!({
                "phraseIndex": conflict.phrase_index(),
                "lumi": conflict.lumi().map(phrase_preview_json),
                "source": conflict.source().map(phrase_preview_json),
            })).collect::<Vec<_>>(),
        }))
    }
}

#[derive(Clone, Copy)]
enum HistoryAction {
    Undo,
    Redo,
}

struct TimelineHistory {
    undo: Vec<TimelineRevision>,
    redo: Vec<TimelineRevision>,
}

impl TimelineHistory {
    fn replay(summaries: &[TimelineRevisionSummary]) -> Result<Self, LibraryWorkerError> {
        let mut current = None;
        let mut undo = Vec::new();
        let mut redo = Vec::new();
        for summary in summaries {
            match summary.reason() {
                TimelineRevisionReason::InitialSourceMapping => current = Some(summary.revision()),
                TimelineRevisionReason::Undo => {
                    let target = summary
                        .restored_from()
                        .ok_or(LibraryWorkerError::CorruptHistory)?;
                    let previous = current.ok_or(LibraryWorkerError::CorruptHistory)?;
                    if undo.pop() != Some(target) {
                        return Err(LibraryWorkerError::CorruptHistory);
                    }
                    redo.push(previous);
                    current = Some(target);
                }
                TimelineRevisionReason::Redo => {
                    let target = summary
                        .restored_from()
                        .ok_or(LibraryWorkerError::CorruptHistory)?;
                    let previous = current.ok_or(LibraryWorkerError::CorruptHistory)?;
                    if redo.pop() != Some(target) {
                        return Err(LibraryWorkerError::CorruptHistory);
                    }
                    undo.push(previous);
                    current = Some(target);
                }
                TimelineRevisionReason::RestoreRevision => {
                    if let Some(previous) = current {
                        undo.push(previous);
                    }
                    current = summary.restored_from();
                    redo.clear();
                }
                _ => {
                    if let Some(previous) = current {
                        undo.push(previous);
                    }
                    current = Some(summary.revision());
                    redo.clear();
                }
            }
        }
        if current.is_none() {
            return Err(LibraryWorkerError::CorruptHistory);
        }
        Ok(Self { undo, redo })
    }
}

fn seed_default_role_catalog(
    repository: &mut SqliteLibraryRepository,
) -> Result<bool, LibraryWorkerError> {
    let existing = repository.phrase_role_catalog()?;
    if existing.defaults_version() >= lumi_library::PHRASE_ROLE_DEFAULTS_VERSION {
        return Ok(false);
    }
    let seeded = seeded_phrase_role_catalog(&existing)?;
    if existing.revision() == 0 {
        repository.initialize_phrase_role_catalog(&seeded)?;
    } else {
        repository.replace_phrase_role_catalog(&seeded, existing.revision())?;
    }
    Ok(true)
}

fn seed_default_autoloop_catalog(
    repository: &mut SqliteLibraryRepository,
) -> Result<(), LibraryWorkerError> {
    let existing = repository.autoloop_catalog()?;
    if existing.defaults_version() >= lumi_library::AUTOLOOP_CATALOG_DEFAULTS_VERSION {
        existing.validate_roles(&repository.phrase_role_catalog()?)?;
        return Ok(());
    }
    let phrase_roles = repository.phrase_role_catalog()?;
    let seeded = seeded_autoloop_catalog(&existing, &phrase_roles)?;
    if existing.revision() == 0 {
        repository.initialize_autoloop_catalog(&seeded)?;
    } else {
        repository.replace_autoloop_catalog(&seeded, existing.revision())?;
    }
    Ok(())
}

fn role_display_name(roles: &[PhraseRole], id: &PhraseRoleId) -> String {
    roles
        .iter()
        .find(|role| role.id() == id)
        .map(PhraseRole::display_name)
        .unwrap_or_else(|| id.as_str())
        .to_owned()
}

fn planner_phrase_kind(phrase: &PhraseInstance, phrase_count: usize) -> PhraseKind {
    match phrase.role_id().as_str() {
        "intro-outro" if usize::from(phrase.index()) + 1 == phrase_count => PhraseKind::Outro,
        "intro-outro" => PhraseKind::Intro,
        "bridge" => PhraseKind::Verse,
        "breakdown-1" | "breakdown-2" | "breakdown-3" => PhraseKind::Breakdown,
        "synth" | "pre-drop" | "buildup-1" | "buildup-2" | "buildup-3" => PhraseKind::Build,
        "drop" => PhraseKind::Drop,
        // Custom roles remain authoritative for Autoloop resolution. The
        // legacy scene planner receives a neutral category until its role
        // taxonomy becomes fully configurable in a later epic.
        _ => PhraseKind::Verse,
    }
}

fn autoloop_resolution_reason_name(reason: &AutoloopResolutionReason) -> String {
    match reason {
        AutoloopResolutionReason::Automatic => "automatic".to_owned(),
        AutoloopResolutionReason::ExactVariant => "exactVariant".to_owned(),
        AutoloopResolutionReason::ThemeSpecificExact => "themeSpecificExact".to_owned(),
        AutoloopResolutionReason::SameRoleFallback {
            requested_variant_id,
        } => format!("sameRoleFallback:{}", requested_variant_id.as_str()),
    }
}

fn phrase_preview_json(phrase: &PhraseInstance) -> Value {
    json!({
        "startBeat": phrase.start_beat(),
        "endBeat": phrase.end_beat(),
        "roleId": phrase.role_id().as_str(),
    })
}

fn origin_name(origin: TimelineRevisionOrigin) -> &'static str {
    match origin {
        TimelineRevisionOrigin::SourceImport => "sourceImport",
        TimelineRevisionOrigin::UserEdit => "userEdit",
        TimelineRevisionOrigin::SourceReconcile => "sourceReconcile",
        TimelineRevisionOrigin::RevisionRestore => "revisionRestore",
    }
}

fn reason_name(reason: TimelineRevisionReason) -> &'static str {
    match reason {
        TimelineRevisionReason::InitialSourceMapping => "initialSourceMapping",
        TimelineRevisionReason::CreatePhrase => "createPhrase",
        TimelineRevisionReason::SplitPhrase => "splitPhrase",
        TimelineRevisionReason::MergePrevious => "mergePrevious",
        TimelineRevisionReason::MergeNext => "mergeNext",
        TimelineRevisionReason::MoveBoundary => "moveBoundary",
        TimelineRevisionReason::AbsorbPrevious => "absorbPrevious",
        TimelineRevisionReason::AbsorbNext => "absorbNext",
        TimelineRevisionReason::ChangeRole => "changeRole",
        TimelineRevisionReason::ChangeLoopStrategy => "changeLoopStrategy",
        TimelineRevisionReason::Undo => "undo",
        TimelineRevisionReason::Redo => "redo",
        TimelineRevisionReason::RestoreRevision => "restoreRevision",
        TimelineRevisionReason::SourceReconcile => "sourceReconcile",
    }
}

fn loop_strategy_name(strategy: &PhraseLoopStrategy) -> &'static str {
    match strategy {
        PhraseLoopStrategy::Auto => "auto",
        PhraseLoopStrategy::FixedVariant(_) => "fixedVariant",
        PhraseLoopStrategy::ThemeSpecificExact(_) => "themeSpecificExact",
    }
}

fn loop_strategy_json(catalog: &lumi_library::AutoloopCatalog, phrase: &PhraseInstance) -> Value {
    let role_id = phrase.role_id();
    let active_variants = catalog
        .variants()
        .iter()
        .filter(|variant| variant.role_id() == role_id && !variant.is_archived())
        .collect::<Vec<_>>();
    let mut issues = Vec::new();
    let mut stale = false;
    let fixed_variant_id = match phrase.loop_strategy() {
        PhraseLoopStrategy::FixedVariant(variant_id) => Some(variant_id.as_str()),
        PhraseLoopStrategy::Auto | PhraseLoopStrategy::ThemeSpecificExact(_) => None,
    };
    let overrides = match phrase.loop_strategy() {
        PhraseLoopStrategy::ThemeSpecificExact(values) => values
            .iter()
            .map(|value| {
                json!({
                    "themeId": value.theme_id().value(),
                    "variantId": value.variant_id().as_str(),
                })
            })
            .collect::<Vec<_>>(),
        PhraseLoopStrategy::Auto | PhraseLoopStrategy::FixedVariant(_) => Vec::new(),
    };
    for theme in catalog.themes() {
        let exact_variant = match phrase.loop_strategy() {
            PhraseLoopStrategy::FixedVariant(variant_id) => Some(variant_id),
            PhraseLoopStrategy::ThemeSpecificExact(values) => values
                .iter()
                .find(|value| value.theme_id() == theme.id())
                .map(lumi_library::ThemeSpecificVariant::variant_id),
            PhraseLoopStrategy::Auto => None,
        };
        if let Some(variant_id) = exact_variant {
            let is_active = active_variants
                .iter()
                .any(|variant| variant.id() == variant_id);
            if !is_active {
                stale = true;
                issues.push(json!({
                    "reason": "variantMissingOrArchived",
                    "themeId": theme.id().value(),
                    "variantId": variant_id.as_str(),
                }));
            } else if !catalog.cells().iter().any(|cell| {
                cell.theme_id() == theme.id()
                    && cell.role_id() == role_id
                    && cell.variant_id() == variant_id
            }) {
                issues.push(json!({
                    "reason": "exactCellMissing",
                    "themeId": theme.id().value(),
                    "variantId": variant_id.as_str(),
                }));
            }
        } else if !active_variants.iter().any(|variant| {
            catalog.cells().iter().any(|cell| {
                cell.theme_id() == theme.id()
                    && cell.role_id() == role_id
                    && cell.variant_id() == variant.id()
            })
        }) {
            issues.push(json!({
                "reason": "automaticCoverageMissing",
                "themeId": theme.id().value(),
                "variantId": Value::Null,
            }));
        }
    }
    let status = if stale {
        "stale"
    } else if issues.is_empty() {
        "ready"
    } else {
        "incomplete"
    };
    json!({
        "kind": loop_strategy_name(phrase.loop_strategy()),
        "locked": !matches!(phrase.loop_strategy(), PhraseLoopStrategy::Auto),
        "provenance": if matches!(phrase.loop_strategy(), PhraseLoopStrategy::Auto) {
            "automaticDefault"
        } else {
            "userSelection"
        },
        "rowRoleId": role_id.as_str(),
        "fixedVariantId": fixed_variant_id,
        "themeOverrides": overrides,
        "validatedCatalogRevision": catalog.revision(),
        "status": status,
        "issues": issues,
    })
}

fn track_json_with_device_sources(
    track: &TrackSummary,
    device_sources: &[lumi_library_sqlite::DeviceTrackSourceRelation],
    workflow: Option<&TrackWorkflowState>,
    phrase_protection: lumi_library_sqlite::TrackPhraseProtection,
) -> Value {
    let default_workflow = TrackWorkflowState::default_for(track.id());
    let workflow = workflow.unwrap_or(&default_workflow);
    json!({
        "id": track.id().value(),
        "sourceTrackId": track.source_track_id().as_str(),
        "title": track.title(),
        "artist": track.artist(),
        "bpmMilli": track.bpm_milli(),
        "key": {
            "pitchClass": pitch_class_name(track.musical_key().pitch_class()),
            "mode": key_mode_name(track.musical_key().mode()),
        },
        "durationMillis": track.duration_millis(),
        "colorRgb": track.color().map(|color| color.rgb_u32()),
        "analysisRevision": track.source_revision().as_str(),
        "timelineRevision": track.timeline_revision().map(|revision| revision.value()),
        "usbSources": device_sources.iter().map(|source| json!({
            "sourceId": source.source_id,
            "displayName": source.display_name,
            "syncDisposition": source.sync_disposition,
        })).collect::<Vec<_>>(),
        "readiness": {
            "status": "ready",
            "missingCapabilities": [],
            "warnings": [],
        },
        "workflow": workflow_json(workflow),
        "phraseProtection": {
            "locked": phrase_protection.locked,
            "revision": phrase_protection.revision,
        },
    })
}

fn workflow_json(workflow: &TrackWorkflowState) -> Value {
    json!({
        "preparationStatus": workflow.preparation_status().as_str(),
        "stepId": workflow.step_id(),
        "statusRevision": workflow.status_revision(),
        "effectiveReady": workflow.is_effectively_ready(),
        "attention": workflow.attention().map(|attention| json!({
            "revision": attention.revision(),
            "sourceId": attention.source_id(),
            "sourceRevision": attention.source_revision(),
            "detectedAt": attention.detected_at(),
            "reasons": attention.reasons().iter().map(|reason| reason.as_str()).collect::<Vec<_>>(),
        })),
    })
}

fn rekordbox_mirror_snapshot(
    snapshot: &RekordboxXmlMirrorSnapshot,
) -> Result<SourceMirrorSnapshot, LibraryWorkerError> {
    let tracks = snapshot
        .tracks()
        .iter()
        .map(|track| {
            let duration_millis = track
                .total_time_seconds()
                .map(|seconds| {
                    seconds
                        .checked_mul(1_000)
                        .ok_or(LibraryWorkerError::RekordboxMirrorOverflow)
                })
                .transpose()?;
            Ok(SourceMirrorTrack::try_new(
                lumi_library::SourceTrackId::try_new(track.source_track_id())?,
                track.title(),
                track.artist().map(str::to_owned),
                track.average_bpm().map(str::to_owned),
                track.tonality().map(str::to_owned),
                duration_millis,
                track.colour().map(str::to_owned),
                track.location(),
            )?)
        })
        .collect::<Result<Vec<_>, LibraryWorkerError>>()?;
    let playlists = snapshot
        .playlists()
        .iter()
        .map(|playlist| {
            let track_ids = playlist
                .track_ids()
                .iter()
                .map(|track_id| lumi_library::SourceTrackId::try_new(track_id.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(SourceMirrorPlaylist::try_new(
                playlist.path(),
                playlist.name(),
                track_ids,
            )?)
        })
        .collect::<Result<Vec<_>, LibraryWorkerError>>()?;
    Ok(SourceMirrorSnapshot::try_new(
        lumi_library::LibrarySourceId::try_new(REKORDBOX_XML_SOURCE_ID)?,
        REKORDBOX_XML_SOURCE_KIND,
        "Rekordbox XML",
        SourceRevision::try_new(format!("sha256:{}", snapshot.content_sha256()))?,
        tracks,
        playlists,
    )?)
}

struct RekordboxInstallationPaths {
    database: PathBuf,
    analysis_root: PathBuf,
    sqlcipher: PathBuf,
}

impl RekordboxInstallationPaths {
    fn discover() -> Result<Self, LibraryWorkerError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(LibraryWorkerError::RekordboxInstallationUnavailable)?;
        let database = home.join("Library/Pioneer/rekordbox/master.db");
        let analysis_root = home.join("Library/Pioneer/rekordbox/share");
        let sqlcipher = [
            PathBuf::from("/opt/homebrew/bin/sqlcipher"),
            PathBuf::from("/usr/local/bin/sqlcipher"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or(LibraryWorkerError::RekordboxInstallationUnavailable)?;
        if !database.is_file() || !analysis_root.is_dir() {
            return Err(LibraryWorkerError::RekordboxInstallationUnavailable);
        }
        Ok(Self {
            database,
            analysis_root,
            sqlcipher,
        })
    }
}

struct RekordboxImportTemporaryRoot(PathBuf);

impl RekordboxImportTemporaryRoot {
    fn create() -> Result<Self, LibraryWorkerError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| LibraryWorkerError::RekordboxImportClock)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "lumi-rekordbox-import-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RekordboxImportTemporaryRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rekordbox_canonical_baseline(
    snapshot: &RekordboxXmlMirrorSnapshot,
    analysis: &BTreeMap<String, ResolvedTrackAnalysis>,
    database_sha256: &str,
) -> Result<ImportedLibraryBaseline, LibraryWorkerError> {
    let source_revision = SourceRevision::try_new(format!(
        "xml:{}-db:{}-anlz2",
        &snapshot.content_sha256()[..16],
        &database_sha256[..16]
    ))?;
    let tracks = snapshot
        .tracks()
        .iter()
        .map(|track| {
            let parsed = analysis.get(track.source_track_id()).ok_or_else(|| {
                LibraryWorkerError::MissingRekordboxTrackAnalysis(
                    track.source_track_id().to_owned(),
                )
            })?;
            let canonical_grid = canonical_beat_grid(parsed)?;
            let beat_grid = canonical_grid.beat_grid;
            let total_beats = u32::try_from(beat_grid.markers().len())
                .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?;
            let bpm_milli = parse_bpm_milli(track.average_bpm())
                .or_else(|| median_analysis_bpm_milli(parsed))
                .ok_or_else(|| LibraryWorkerError::InvalidRekordboxMetadata {
                    track_id: track.source_track_id().to_owned(),
                    field: "BPM",
                })?;
            let musical_key = parse_musical_key(track.tonality()).ok_or_else(|| {
                LibraryWorkerError::InvalidRekordboxMetadata {
                    track_id: track.source_track_id().to_owned(),
                    field: "key",
                }
            })?;
            let duration_millis = waveform_duration_millis(parsed)
                .or_else(|| {
                    track
                        .total_time_seconds()
                        .and_then(|seconds| seconds.checked_mul(1_000))
                })
                .or_else(|| inferred_duration_millis(&beat_grid, bpm_milli))
                .ok_or(LibraryWorkerError::RekordboxImportOverflow)?;
            let waveform = downsample_waveform(&parsed.waveform, MAX_IMPORTED_WAVEFORM_POINTS);
            let phrases =
                canonical_phrases(parsed, total_beats, canonical_grid.source_beat_offset)?;
            ImportedTrackAnalysis::try_new(
                lumi_library::SourceTrackId::try_new(track.source_track_id())?,
                source_revision.clone(),
                track.title(),
                track.artist().unwrap_or("Unknown Artist"),
                bpm_milli,
                musical_key,
                duration_millis,
                track.colour().and_then(parse_track_color),
                track.location(),
                beat_grid,
                waveform,
                phrases,
            )
            .and_then(|track| track.with_hot_cues(canonical_hot_cues(parsed, duration_millis)?))
            .map_err(LibraryWorkerError::InvalidRekordboxTrack)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let imported_ids = tracks
        .iter()
        .map(|track| track.source_track_id().as_str())
        .collect::<BTreeSet<_>>();
    let playlists = snapshot
        .playlists()
        .iter()
        .map(|playlist| {
            let track_ids = playlist
                .track_ids()
                .iter()
                .filter(|track_id| imported_ids.contains(track_id.as_str()))
                .map(|track_id| lumi_library::SourceTrackId::try_new(track_id.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            ImportedPlaylist::try_new(
                SourcePlaylistId::try_new(playlist.path())?,
                playlist.name(),
                track_ids,
            )
            .map_err(LibraryWorkerError::InvalidRekordboxBaseline)
        })
        .collect::<Result<Vec<_>, LibraryWorkerError>>()?;
    ImportedLibraryBaseline::try_new(
        lumi_library::LibrarySourceId::try_new(REKORDBOX_CANONICAL_SOURCE_ID)?,
        REKORDBOX_CANONICAL_SOURCE_KIND,
        "Rekordbox 7",
        source_revision,
        tracks,
        playlists,
    )
    .map_err(LibraryWorkerError::InvalidRekordboxBaseline)
}

struct CanonicalBeatGrid {
    beat_grid: BeatGrid,
    /// Number of leading source beats discarded before Rekordbox's first
    /// downbeat. Phrase beat indexes use the original source coordinate space
    /// and must be projected by this exact offset.
    source_beat_offset: u32,
}

fn canonical_beat_grid(
    parsed: &ResolvedTrackAnalysis,
) -> Result<CanonicalBeatGrid, LibraryWorkerError> {
    let source_beat_offset = parsed
        .beat_grid
        .iter()
        .position(|beat| beat.beat_number == 1)
        .ok_or(LibraryWorkerError::IncompleteRekordboxBeatGrid)?;
    let source_beats = &parsed.beat_grid[source_beat_offset..];
    let complete_beats = source_beats.len() - source_beats.len() % 4;
    if complete_beats == 0 {
        return Err(LibraryWorkerError::IncompleteRekordboxBeatGrid);
    }
    let markers = source_beats
        .iter()
        .take(complete_beats)
        .enumerate()
        .map(|(index, beat)| {
            let beat_index =
                u32::try_from(index).map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?;
            let beat_in_bar = u8::try_from(beat_index % 4 + 1)
                .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?;
            if beat.beat_number != u16::from(beat_in_bar) {
                return Err(LibraryWorkerError::InconsistentRekordboxBeatGrid {
                    source_index: source_beat_offset + index,
                    expected: beat_in_bar,
                    actual: beat.beat_number,
                });
            }
            Ok(BeatMarker::new(
                beat_index,
                u64::from(beat.time_millis),
                beat_index / 4 + 1,
                beat_in_bar,
            ))
        })
        .collect::<Result<Vec<_>, LibraryWorkerError>>()?;
    Ok(CanonicalBeatGrid {
        beat_grid: BeatGrid::try_new(4, markers)
            .map_err(LibraryWorkerError::InvalidRekordboxBeatGrid)?,
        source_beat_offset: u32::try_from(source_beat_offset)
            .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?,
    })
}

fn device_analysis_upsert(
    source_id: &str,
    track_id: TrackId,
    device_track: &DeviceTrack,
    analysis: &ResolvedTrackAnalysis,
) -> Result<DeviceAnalysisUpsert, LibraryWorkerError> {
    let canonical_grid = canonical_beat_grid(analysis)?;
    let duration_millis = waveform_duration_millis(analysis)
        .or_else(|| {
            (device_track.duration_millis > 0).then_some(u64::from(device_track.duration_millis))
        })
        .or_else(|| inferred_duration_millis(&canonical_grid.beat_grid, device_track.bpm_milli))
        .ok_or(LibraryWorkerError::RekordboxImportOverflow)?;
    let total_beats = u32::try_from(canonical_grid.beat_grid.markers().len())
        .map_err(|_| LibraryWorkerError::RekordboxImportOverflow)?;
    Ok(DeviceAnalysisUpsert {
        track_id,
        source_id: source_id.to_owned(),
        device_track_id: device_track.device_track_id,
        analysis_revision: format!("device:{source_id}:{}", device_track.analysis_revision),
        source_analysis_revision: device_track.analysis_revision.clone(),
        analyzed_at: device_track.analyzed_at.clone(),
        duration_millis,
        beat_grid: canonical_grid.beat_grid,
        waveform: downsample_waveform(&analysis.waveform, MAX_IMPORTED_WAVEFORM_POINTS),
        raw_phrases: canonical_phrases(analysis, total_beats, canonical_grid.source_beat_offset)?,
        hot_cues: canonical_hot_cues(analysis, duration_millis)?,
    })
}

fn canonical_hot_cues(
    parsed: &ResolvedTrackAnalysis,
    duration_millis: u64,
) -> Result<Vec<lumi_library::HotCue>, lumi_library::TrackValidationError> {
    parsed
        .hot_cues
        .iter()
        .filter(|cue| u64::from(cue.time_millis) < duration_millis)
        .map(|cue| {
            let color_rgb = (u32::from(cue.color_rgb[0]) << 16)
                | (u32::from(cue.color_rgb[1]) << 8)
                | u32::from(cue.color_rgb[2]);
            lumi_library::HotCue::try_new(
                cue.index,
                u64::from(cue.time_millis),
                cue.loop_end_millis
                    .map(u64::from)
                    .filter(|end| *end <= duration_millis),
                cue.comment.clone(),
                color_rgb,
            )
        })
        .collect()
}

fn device_track_matches(candidate: &DeviceMatchCandidate, device: &DeviceTrack) -> bool {
    if !device_metadata_matches(candidate, device) {
        return false;
    }
    let Some(path) = file_uri_path(&candidate.audio_uri) else {
        return false;
    };
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() == u64::from(device.file_size))
        .unwrap_or(false)
}

fn device_metadata_matches(candidate: &DeviceMatchCandidate, device: &DeviceTrack) -> bool {
    if normalize_device_match(&candidate.title) != normalize_device_match(&device.title)
        || normalize_device_match(&candidate.artist) != normalize_device_match(&device.artist)
        || candidate.bpm_milli.abs_diff(device.bpm_milli) > 10
        || candidate
            .duration_millis
            .abs_diff(u64::from(device.duration_millis))
            > 1_000
    {
        return false;
    }
    true
}

fn is_kept_active_revision(
    disposition: &str,
    stored_analysis_revision: &str,
    incoming_analysis_revision: &str,
) -> bool {
    disposition == "kept-active" && stored_analysis_revision == incoming_analysis_revision
}

fn kept_active_track_is_current(
    disposition: &str,
    stored_analysis_revision: &str,
    incoming_analysis_revision: &str,
    stored_metadata_revision: &str,
    incoming_metadata_revision: &str,
) -> bool {
    is_kept_active_revision(
        disposition,
        stored_analysis_revision,
        incoming_analysis_revision,
    ) && stored_metadata_revision == incoming_metadata_revision
}

fn device_status_counts<'a>(tracks: impl Iterator<Item = &'a DeviceInspectionTrack>) -> Value {
    let mut current = 0_u64;
    let mut usb_newer = 0_u64;
    let mut usb_outdated = 0_u64;
    let mut not_in_lumi = 0_u64;
    let mut conflict = 0_u64;
    for track in tracks {
        match track.status {
            "current" => current += 1,
            "usb-newer" => usb_newer += 1,
            "usb-outdated" => usb_outdated += 1,
            "not-in-lumi" => not_in_lumi += 1,
            _ => conflict += 1,
        }
    }
    json!({
        "current": current,
        "usbNewer": usb_newer,
        "usbOutdated": usb_outdated,
        "notInLumi": not_in_lumi,
        "conflict": conflict,
    })
}

fn review_component_json(changed: bool, detail: String) -> Value {
    json!({
        "status": if changed { "changed" } else { "unchanged" },
        "detail": detail,
    })
}

fn review_comparison_json(item: &DeviceReviewComparison) -> Value {
    let mut components = serde_json::Map::new();
    components.insert(
        "beatGrid".to_owned(),
        review_component_json(item.beat_grid_changed, item.beat_grid_detail.clone()),
    );
    components.insert(
        "cuePoints".to_owned(),
        review_component_json(item.hot_cues_changed, item.hot_cues_detail.clone()),
    );
    components.insert(
        "fileData".to_owned(),
        review_component_json(item.file_data_changed, item.file_detail.clone()),
    );
    components.insert(
        "rekordboxPhrases".to_owned(),
        review_component_json(item.raw_phrases_changed, item.raw_phrases_detail.clone()),
    );
    components.insert(
        "waveform".to_owned(),
        review_component_json(item.waveform_changed, item.waveform_detail.clone()),
    );
    Value::Object(components)
}

fn optional_rgb(value: Option<u32>) -> String {
    value.map_or_else(|| "none".to_owned(), |rgb| format!("#{rgb:06X}"))
}

fn beat_grid_review_detail(incoming: &BeatGrid, active: &BeatGrid) -> String {
    let incoming_count = incoming.markers().len();
    let active_count = active.markers().len();
    let first_change = incoming
        .markers()
        .iter()
        .zip(active.markers())
        .position(|(usb, lumi)| usb != lumi);
    match first_change {
        Some(index) => {
            let usb = incoming.markers()[index];
            let lumi = active.markers()[index];
            format!(
                "First change at beat {}: USB {:.3}s · Lumi {:.3}s · USB {incoming_count} beats · Lumi {active_count} beats",
                index + 1,
                usb.time_millis() as f64 / 1_000.0,
                lumi.time_millis() as f64 / 1_000.0
            )
        }
        None if incoming_count != active_count => {
            format!("Beat count differs: USB {incoming_count} · Lumi {active_count}")
        }
        None => format!("USB {incoming_count} beats · Lumi {active_count} beats"),
    }
}

fn hot_cue_review_detail(incoming: &[HotCue], active: &[HotCue]) -> String {
    let labels = |cues: &[HotCue]| {
        cues.iter()
            .map(|cue| hot_cue_label(cue.index()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let incoming_labels = labels(incoming);
    let active_labels = labels(active);
    let changed = incoming
        .iter()
        .map(HotCue::index)
        .chain(active.iter().map(HotCue::index))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|index| {
            incoming.iter().find(|cue| cue.index() == *index)
                != active.iter().find(|cue| cue.index() == *index)
        })
        .map(hot_cue_label)
        .collect::<Vec<_>>();
    let inventory = format!(
        "USB {} cues ({}) · Lumi {} cues ({})",
        incoming.len(),
        if incoming_labels.is_empty() {
            "none"
        } else {
            &incoming_labels
        },
        active.len(),
        if active_labels.is_empty() {
            "none"
        } else {
            &active_labels
        }
    );
    if changed.is_empty() {
        inventory
    } else {
        format!("Changed cue details: {} · {inventory}", changed.join(", "))
    }
}

fn hot_cue_label(index: u8) -> String {
    char::from(b'A'.saturating_add(index.saturating_sub(1))).to_string()
}

fn raw_phrases_review_detail(
    incoming: &[RawPhraseObservation],
    active: &[RawPhraseObservation],
) -> String {
    let first_change = incoming
        .iter()
        .zip(active)
        .position(|(usb, lumi)| usb != lumi);
    match first_change {
        Some(index) => {
            let usb = &incoming[index];
            let lumi = &active[index];
            format!(
                "First change at phrase {}: USB {} [{}–{}] · Lumi {} [{}–{}] · USB {} phrases · Lumi {} phrases",
                index + 1,
                usb.source_label(),
                usb.start_beat(),
                usb.end_beat(),
                lumi.source_label(),
                lumi.start_beat(),
                lumi.end_beat(),
                incoming.len(),
                active.len()
            )
        }
        None if incoming.len() != active.len() => format!(
            "Phrase count differs after the shared prefix: USB {} · Lumi {}",
            incoming.len(),
            active.len()
        ),
        None => format!(
            "USB {} phrases · Lumi source {} phrases",
            incoming.len(),
            active.len()
        ),
    }
}

fn waveform_review_detail(incoming: &[WaveformPoint], active: &[WaveformPoint]) -> String {
    let first_change = incoming
        .iter()
        .zip(active)
        .position(|(usb, lumi)| usb != lumi);
    match first_change {
        Some(index) => format!(
            "First RGB waveform difference at sample {} · USB {} samples · Lumi {} samples",
            index + 1,
            incoming.len(),
            active.len()
        ),
        None if incoming.len() != active.len() => format!(
            "Waveform sample count differs: USB {} · Lumi {}",
            incoming.len(),
            active.len()
        ),
        None => format!(
            "RGB waveform is identical · {} samples compared",
            incoming.len()
        ),
    }
}

fn normalize_device_match(value: &str) -> String {
    value.trim().to_lowercase()
}

fn nonempty_device_metadata<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn device_audio_uri(path: &Path) -> String {
    let value = path.to_string_lossy();
    let mut encoded = String::with_capacity(value.len() + 16);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    format!("file://localhost{encoded}")
}

fn file_uri_path(value: &str) -> Option<PathBuf> {
    let encoded = value
        .strip_prefix("file://localhost")
        .or_else(|| value.strip_prefix("file://"))?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hexadecimal(*bytes.get(index + 1)?)?;
            let low = hexadecimal(*bytes.get(index + 2)?)?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

fn first_available_audio_uri(canonical: &str, device_candidates: &[String]) -> String {
    std::iter::once(canonical)
        .chain(device_candidates.iter().map(String::as_str))
        .find(|audio_uri| file_uri_path(audio_uri).is_some_and(|path| path.is_file()))
        .unwrap_or(canonical)
        .to_owned()
}

fn hexadecimal(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn canonical_phrases(
    parsed: &ResolvedTrackAnalysis,
    total_beats: u32,
    source_beat_offset: u32,
) -> Result<Vec<RawPhraseObservation>, LibraryWorkerError> {
    if parsed.phrases.is_empty() {
        return Ok(Vec::new());
    }
    let mut labels_by_start = BTreeMap::<u32, String>::new();
    for phrase in &parsed.phrases {
        let projected_start = phrase.start_beat.saturating_sub(source_beat_offset);
        if projected_start < total_beats {
            labels_by_start
                .entry(projected_start)
                .or_insert_with(|| phrase.source_label.clone());
        }
    }
    let first_label = labels_by_start
        .values()
        .next()
        .cloned()
        .ok_or(LibraryWorkerError::IncompleteRekordboxPhrases)?;
    labels_by_start.entry(0).or_insert(first_label);
    let starts = labels_by_start.keys().copied().collect::<Vec<_>>();
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(total_beats);
            RawPhraseObservation::try_new(
                *start,
                end,
                labels_by_start.get(start).cloned().unwrap_or_default(),
            )
            .map_err(LibraryWorkerError::InvalidRekordboxTrack)
        })
        .collect()
}

fn downsample_waveform(points: &[AnalysisWaveformPoint], maximum: usize) -> Vec<WaveformPoint> {
    if points.len() <= maximum {
        return points
            .iter()
            .map(|point| WaveformPoint::new(point.low, point.mid, point.high))
            .collect();
    }
    (0..maximum)
        .map(|index| {
            let start = index * points.len() / maximum;
            let end = ((index + 1) * points.len() / maximum).max(start + 1);
            let mut low = 0_u8;
            let mut mid = 0_u8;
            let mut high = 0_u8;
            for point in &points[start..end] {
                low = low.max(point.low);
                mid = mid.max(point.mid);
                high = high.max(point.high);
            }
            WaveformPoint::new(low, mid, high)
        })
        .collect()
}

/// Detailed Rekordbox waveforms contain one column per half-frame (1/150 s).
/// This is more precise than OneLibrary's integer-second duration and keeps
/// waveform, audio and beat-grid coordinates on the same clock.
fn waveform_duration_millis(parsed: &ResolvedTrackAnalysis) -> Option<u64> {
    let frames = u64::try_from(parsed.waveform.len()).ok()?;
    (!parsed.waveform.is_empty()).then(|| frames.saturating_mul(1_000) / 150)
}

fn parse_bpm_milli(value: Option<&str>) -> Option<u32> {
    let bpm = value?.trim().parse::<f64>().ok()?;
    if !bpm.is_finite() || !(20.0..=300.0).contains(&bpm) {
        return None;
    }
    u32::try_from((bpm * 1_000.0).round() as i64).ok()
}

fn median_analysis_bpm_milli(parsed: &ResolvedTrackAnalysis) -> Option<u32> {
    let mut values = parsed
        .beat_grid
        .iter()
        .map(|beat| u32::from(beat.tempo_centi_bpm) * 10)
        .filter(|value| (20_000..=300_000).contains(value))
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.get(values.len() / 2).copied()
}

fn inferred_duration_millis(beat_grid: &BeatGrid, bpm_milli: u32) -> Option<u64> {
    let last = beat_grid.markers().last()?.time_millis();
    last.checked_add(60_000_000_u64 / u64::from(bpm_milli))
}

fn parse_musical_key(value: Option<&str>) -> Option<lumi_domain::MusicalKey> {
    let value = value?.trim();
    if let Some((number, mode)) = parse_camelot(value) {
        return camelot_key(number, mode);
    }
    let minor = value.ends_with('m');
    let root = value.trim_end_matches('m');
    let pitch = match root {
        "C" => PitchClass::C,
        "C#" | "Db" => PitchClass::CSharp,
        "D" => PitchClass::D,
        "D#" | "Eb" => PitchClass::DSharp,
        "E" | "Fb" => PitchClass::E,
        "F" | "E#" => PitchClass::F,
        "F#" | "Gb" => PitchClass::FSharp,
        "G" => PitchClass::G,
        "G#" | "Ab" => PitchClass::GSharp,
        "A" => PitchClass::A,
        "A#" | "Bb" => PitchClass::ASharp,
        "B" | "Cb" => PitchClass::B,
        _ => return None,
    };
    Some(lumi_domain::MusicalKey::new(
        pitch,
        if minor {
            KeyMode::Minor
        } else {
            KeyMode::Major
        },
    ))
}

fn parse_camelot(value: &str) -> Option<(u8, char)> {
    let mode = value.chars().last()?.to_ascii_uppercase();
    if !matches!(mode, 'A' | 'B') {
        return None;
    }
    let number = value[..value.len() - 1].parse::<u8>().ok()?;
    (1..=12).contains(&number).then_some((number, mode))
}

fn camelot_key(number: u8, mode: char) -> Option<lumi_domain::MusicalKey> {
    let pitch = match (number, mode) {
        (1, 'A') => PitchClass::GSharp,
        (2, 'A') => PitchClass::DSharp,
        (3, 'A') => PitchClass::ASharp,
        (4, 'A') => PitchClass::F,
        (5, 'A') => PitchClass::C,
        (6, 'A') => PitchClass::G,
        (7, 'A') => PitchClass::D,
        (8, 'A') => PitchClass::A,
        (9, 'A') => PitchClass::E,
        (10, 'A') => PitchClass::B,
        (11, 'A') => PitchClass::FSharp,
        (12, 'A') => PitchClass::CSharp,
        (1, 'B') => PitchClass::B,
        (2, 'B') => PitchClass::FSharp,
        (3, 'B') => PitchClass::CSharp,
        (4, 'B') => PitchClass::GSharp,
        (5, 'B') => PitchClass::DSharp,
        (6, 'B') => PitchClass::ASharp,
        (7, 'B') => PitchClass::F,
        (8, 'B') => PitchClass::C,
        (9, 'B') => PitchClass::G,
        (10, 'B') => PitchClass::D,
        (11, 'B') => PitchClass::A,
        (12, 'B') => PitchClass::E,
        _ => return None,
    };
    Some(lumi_domain::MusicalKey::new(
        pitch,
        if mode == 'A' {
            KeyMode::Minor
        } else {
            KeyMode::Major
        },
    ))
}

fn parse_track_color(value: &str) -> Option<TrackColor> {
    let hexadecimal = value
        .trim()
        .strip_prefix('#')
        .or_else(|| value.trim().strip_prefix("0x"))
        .unwrap_or(value.trim());
    let rgb = u32::from_str_radix(hexadecimal, 16).ok()?;
    Some(TrackColor::new(
        u8::try_from((rgb >> 16) & 0xFF).ok()?,
        u8::try_from((rgb >> 8) & 0xFF).ok()?,
        u8::try_from(rgb & 0xFF).ok()?,
    ))
}

fn pitch_class_name(value: PitchClass) -> &'static str {
    match value {
        PitchClass::C => "c",
        PitchClass::CSharp => "cSharp",
        PitchClass::D => "d",
        PitchClass::DSharp => "dSharp",
        PitchClass::E => "e",
        PitchClass::F => "f",
        PitchClass::FSharp => "fSharp",
        PitchClass::G => "g",
        PitchClass::GSharp => "gSharp",
        PitchClass::A => "a",
        PitchClass::ASharp => "aSharp",
        PitchClass::B => "b",
    }
}

fn key_mode_name(value: KeyMode) -> &'static str {
    match value {
        KeyMode::Major => "major",
        KeyMode::Minor => "minor",
    }
}

#[derive(Debug, Error)]
pub enum LibraryWorkerError {
    #[error("engine service configuration failed: {0}")]
    Configuration(String),
    #[error("demo library failed: {0}")]
    Demo(#[from] DemoLibraryError),
    #[error("local library file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("library persistence failed: {0}")]
    Persistence(#[from] SqliteLibraryError),
    #[error("Light Plan compilation failed: {0}")]
    LightPlan(#[from] LightPlanError),
    #[error("Rekordbox XML source failed: {0}")]
    RekordboxXml(#[from] RekordboxXmlError),
    #[error("Rekordbox identity resolver failed: {0}")]
    RekordboxResolver(#[from] ResolverError),
    #[error("Rekordbox analysis parser failed: {0}")]
    RekordboxAnalysis(#[from] AnalysisError),
    #[error("Rekordbox Device Library failed: {0}")]
    RekordboxDevice(#[from] DeviceError),
    #[error("the selected Rekordbox device has an invalid PIONEER layout")]
    InvalidRekordboxDeviceRoot,
    #[error("select one or more valid USB playlists before synchronizing")]
    InvalidDevicePlaylistSelection,
    #[error("the selected USB playlists contain no tracks")]
    EmptyDevicePlaylistSelection,
    #[error("the selected track has no reusable Lumi-authored phrase timeline")]
    CreativeTimelineSourceUnavailable,
    #[error(
        "the source phrase timeline has {source_beats} beats but the target has {target_beats}; Lumi will not copy it without review"
    )]
    CreativeTimelineIncompatible {
        source_beats: u32,
        target_beats: u32,
    },
    #[error("the USB review changed; refresh the source before choosing again")]
    DeviceReviewChanged,
    #[error("Rekordbox is not installed in a supported local location")]
    RekordboxInstallationUnavailable,
    #[error("the temporary Rekordbox import clock is unavailable")]
    RekordboxImportClock,
    #[error("Rekordbox import arithmetic overflowed")]
    RekordboxImportOverflow,
    #[error(
        "Rekordbox resolved {resolved} of {requested} selected tracks ({path_mismatches} path mismatches)"
    )]
    IncompleteRekordboxResolution {
        requested: usize,
        resolved: usize,
        path_mismatches: usize,
    },
    #[error("Rekordbox parsed {parsed} of {requested} selected tracks")]
    IncompleteRekordboxAnalysis { requested: usize, parsed: usize },
    #[error("Rekordbox analysis is missing for track {0}")]
    MissingRekordboxTrackAnalysis(String),
    #[error("Rekordbox track {track_id} has invalid {field} metadata")]
    InvalidRekordboxMetadata {
        track_id: String,
        field: &'static str,
    },
    #[error("Rekordbox analysis contains no complete beatgrid")]
    IncompleteRekordboxBeatGrid,
    #[error(
        "Rekordbox beatgrid phase is inconsistent at source beat {source_index}: expected {expected}, got {actual}"
    )]
    InconsistentRekordboxBeatGrid {
        source_index: usize,
        expected: u8,
        actual: u16,
    },
    #[error("Rekordbox analysis contains no usable phrases")]
    IncompleteRekordboxPhrases,
    #[error("Rekordbox beatgrid is invalid: {0}")]
    InvalidRekordboxBeatGrid(#[from] lumi_library::BeatGridValidationError),
    #[error("Rekordbox track is invalid: {0}")]
    InvalidRekordboxTrack(#[from] lumi_library::TrackValidationError),
    #[error("Rekordbox baseline is invalid: {0}")]
    InvalidRekordboxBaseline(#[from] lumi_library::LibraryBaselineValidationError),
    #[error("library query is invalid: {0}")]
    Query(#[from] lumi_library::TrackPageRequestError),
    #[error("phrase-role defaults failed: {0}")]
    PhraseRoleDefaults(#[from] PhraseRoleDefaultsError),
    #[error("phrase-role change was rejected: {0}")]
    PhraseRoleCatalog(#[from] PhraseRoleCatalogError),
    #[error("Autoloop defaults failed: {0}")]
    AutoloopDefaults(#[from] AutoloopDefaultsError),
    #[error("Autoloop catalog change was rejected: {0}")]
    AutoloopCatalog(#[from] AutoloopCatalogError),
    #[error("invalid library identifier: {0}")]
    Identifier(#[from] lumi_library::TextIdentifierError),
    #[error("source mirror is invalid: {0}")]
    SourceMirror(#[from] lumi_library::SourceMirrorValidationError),
    #[error("Rekordbox mirror duration overflowed")]
    RekordboxMirrorOverflow,
    #[error("there is no Rekordbox XML preview waiting to be applied")]
    NoPendingRekordboxPreview,
    #[error("the Rekordbox XML export or selection changed; preview again before Apply")]
    RekordboxPreviewChanged,
    #[error("timeline edit was rejected: {0}")]
    TimelineEdit(#[from] TimelineEditError),
    #[error("persisted timeline is invalid: {0}")]
    TimelineValidation(#[from] lumi_library::TimelineValidationError),
    #[error("simulator track is invalid: {0}")]
    SimulatorTrackValidation(#[from] lumi_domain::TrackValidationError),
    #[error("simulator track arithmetic overflow")]
    SimulatorTrackOverflow,
    #[error("library track {0} does not exist")]
    UnknownTrack(u64),
    #[error("a selected track to preserve no longer exists")]
    UnknownResetPreservedTrack,
    #[error("there is no reviewed library reset waiting to be applied")]
    NoPendingLibraryReset,
    #[error("the library changed after the reset preview; preview the reset again")]
    LibraryResetPreviewChanged,
    #[error("the automatic pre-reset backup is missing")]
    MissingLibraryResetBackup,
    #[error("the automatic pre-reset backup is not a valid SQLite library")]
    InvalidLibraryResetBackup,
    #[error("engine-owned backups require a persistent channel database")]
    BackupUnavailable,
    #[error("backup path is outside this Lumi channel's managed Backups directory")]
    UntrustedBackupPath,
    #[error("the track editor selection changed")]
    EditorTrackMismatch,
    #[error("the selected track has no Lumi timeline")]
    MissingTimeline,
    #[error("Lumi phrase editing is protected for this track; unlock it before changing phrases")]
    TrackPhrasesProtected,
    #[error("source phrases cannot form a complete bar-aligned timeline")]
    InvalidSourceTimeline,
    #[error("source phrase '{raw_label}' has no mapping for provider '{provider_kind}'")]
    UnmappedSourcePhrase {
        provider_kind: String,
        raw_label: String,
    },
    #[error(
        "source phrase '{raw_label}' for provider '{provider_kind}' maps to archived role '{role_id:?}'"
    )]
    ArchivedSourcePhraseMapping {
        provider_kind: String,
        raw_label: String,
        role_id: PhraseRoleId,
    },
    #[error("phrase role does not exist")]
    UnknownPhraseRole,
    #[error("archived phrase roles cannot be assigned to new or edited phrases")]
    ArchivedPhraseRole,
    #[error("phrase-role catalog changed; expected revision {expected}, actual {actual}")]
    PhraseRoleCatalogRevisionConflict { expected: u64, actual: u64 },
    #[error("Autoloop catalog changed; expected revision {expected}, actual {actual}")]
    AutoloopCatalogRevisionConflict { expected: u64, actual: u64 },
    #[error("timeline revision {0} is invalid")]
    InvalidTimelineRevision(u64),
    #[error("timeline revision {0} does not exist")]
    UnknownTimelineRevision(u64),
    #[error("timeline revision changed; expected {expected:?}, actual {actual:?}")]
    TimelineRevisionConflict {
        expected: TimelineRevision,
        actual: TimelineRevision,
    },
    #[error("there is no timeline edit to undo")]
    NothingToUndo,
    #[error("there is no timeline edit to redo")]
    NothingToRedo,
    #[error("the configured music-library source is missing from persistence")]
    MissingLibrarySource,
    #[error("timeline history is corrupt")]
    CorruptHistory,
    #[error("timeline history overflowed")]
    HistoryOverflow,
    #[error("source refresh does not match the active library")]
    SourceRefreshIdentityMismatch,
    #[error("there is no source refresh waiting for review")]
    NoPendingSourceRefresh,
    #[error("the source refresh no longer contains the selected track")]
    MissingIncomingTrack,
    #[error("metadata-only refreshes preserve the Lumi timeline and require Keep Lumi")]
    MetadataRefreshRequiresKeepLumi,
    #[error("source reconciliation failed: {0}")]
    Reconcile(#[from] ReconcileError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use lumi_domain::{PhraseKind, ThemeId, TrackId};
    use lumi_library::{
        LibraryRepository as _, LibraryTrackSort, PhraseLoopStrategy, PhraseRoleId,
        ReconcileStrategy, TimelineEditCommand, TimelineRevisionOrigin, TrackPageRequest,
        VariantId, WaveformPoint,
    };
    use lumi_library_demo::{DemoLibraryRevision, DemoLibrarySourceProvider};
    use lumi_library_source::MusicLibrarySourceProvider as _;
    use lumi_library_sqlite::DeviceMatchCandidate;
    use lumi_rekordbox_analysis::{
        AnalysisBeat, AnalysisPhrase, ResolvedTrackAnalysis, TrackAnalysisCoverage,
    };
    use lumi_rekordbox_device::{DeviceLibrarySnapshot, DeviceTrack};
    use serde_json::json;

    use super::{
        AutoloopCatalogMutation, DeviceInspection, DeviceReviewComparison, LibraryQueryUpdate,
        LibraryWorker, LibraryWorkerError, PhraseRoleCatalogMutation, canonical_beat_grid,
        canonical_phrases, deck_waveform_preview_points, device_audio_uri, device_metadata_matches,
        device_track_matches, first_available_audio_uri, is_kept_active_revision,
        kept_active_track_is_current,
    };

    #[test]
    fn kept_active_is_exact_revision_scoped_for_analysis_and_hot_cues() {
        assert!(is_kept_active_revision(
            "kept-active",
            "analysis-gray-1",
            "analysis-gray-1"
        ));
        assert!(!is_kept_active_revision(
            "kept-active",
            "analysis-gray-1",
            "analysis-gray-2"
        ));
        assert!(!is_kept_active_revision(
            "held-conflict",
            "analysis-gray-1",
            "analysis-gray-1"
        ));
        assert!(kept_active_track_is_current(
            "kept-active",
            "analysis-gray-1",
            "analysis-gray-1",
            "metadata-gray-1",
            "metadata-gray-1"
        ));
        assert!(!kept_active_track_is_current(
            "kept-active",
            "analysis-gray-1",
            "analysis-gray-1",
            "metadata-gray-1",
            "metadata-color-update"
        ));
    }

    fn resolved_analysis_with_grid(beat_numbers: &[u16]) -> ResolvedTrackAnalysis {
        ResolvedTrackAnalysis {
            coverage: TrackAnalysisCoverage::default(),
            beat_grid: beat_numbers
                .iter()
                .enumerate()
                .map(|(index, beat_number)| AnalysisBeat {
                    beat_number: *beat_number,
                    tempo_centi_bpm: 15_500,
                    time_millis: u32::try_from(index).unwrap_or_default() * 387,
                })
                .collect(),
            waveform: Vec::new(),
            phrases: Vec::new(),
            hot_cues: Vec::new(),
        }
    }

    #[test]
    fn rekordbox_downbeat_phase_and_exact_times_are_authoritative()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut analysis = resolved_analysis_with_grid(&[4, 1, 2, 3, 4, 1, 2, 3, 4, 1]);
        analysis.phrases = vec![
            AnalysisPhrase {
                start_beat: 0,
                end_beat: 5,
                source_label: "Intro".to_owned(),
            },
            AnalysisPhrase {
                start_beat: 5,
                end_beat: 9,
                source_label: "Drop".to_owned(),
            },
        ];

        let canonical = canonical_beat_grid(&analysis)?;
        assert_eq!(canonical.source_beat_offset, 1);
        assert_eq!(canonical.beat_grid.markers().len(), 8);
        assert_eq!(canonical.beat_grid.markers()[0].time_millis(), 387);
        assert_eq!(canonical.beat_grid.markers()[0].bar_index(), 1);
        assert_eq!(canonical.beat_grid.markers()[0].beat_in_bar(), 1);
        assert_eq!(canonical.beat_grid.markers()[4].time_millis(), 1_935);
        assert_eq!(canonical.beat_grid.markers()[4].bar_index(), 2);
        assert_eq!(canonical.beat_grid.markers()[4].beat_in_bar(), 1);

        let phrases = canonical_phrases(&analysis, 8, canonical.source_beat_offset)?;
        assert_eq!(phrases[0].start_beat(), 0);
        assert_eq!(phrases[0].end_beat(), 4);
        assert_eq!(phrases[0].source_label(), "Intro");
        assert_eq!(phrases[1].start_beat(), 4);
        assert_eq!(phrases[1].end_beat(), 8);
        assert_eq!(phrases[1].source_label(), "Drop");
        Ok(())
    }

    #[test]
    fn inconsistent_rekordbox_beat_phase_fails_closed() {
        let analysis = resolved_analysis_with_grid(&[4, 1, 2, 4, 4, 1, 2, 3, 4]);
        assert!(matches!(
            canonical_beat_grid(&analysis),
            Err(LibraryWorkerError::InconsistentRekordboxBeatGrid {
                source_index: 3,
                expected: 3,
                actual: 4,
            })
        ));
    }

    #[test]
    #[ignore = "requires LUMI_REKORDBOX_ANALYSIS_DAT"]
    fn mounted_rekordbox_analysis_preserves_every_retained_source_beat()
    -> Result<(), Box<dyn std::error::Error>> {
        let dat_path = PathBuf::from(std::env::var("LUMI_REKORDBOX_ANALYSIS_DAT")?);
        let analysis_root = dat_path.parent().ok_or("analysis DAT has no parent")?;
        let temporary = super::RekordboxImportTemporaryRoot::create()?;
        let request = lumi_rekordbox_analysis::ResolvedAnalysisRequest::try_new(
            analysis_root,
            temporary.path().join("exact-grid-evidence"),
            [lumi_rekordbox_analysis::ResolvedAnalysisTrack::try_new(
                "track", &dat_path,
            )?],
        )?;
        let resolved = lumi_rekordbox_analysis::snapshot_resolved_analysis_data(&request)?;
        let source = resolved.tracks.get("track").ok_or("analysis is missing")?;
        let canonical = canonical_beat_grid(source)?;
        let offset = usize::try_from(canonical.source_beat_offset)?;

        let phrases = canonical_phrases(
            source,
            u32::try_from(canonical.beat_grid.markers().len())?,
            canonical.source_beat_offset,
        )?;
        assert_eq!(phrases.len(), source.phrases.len());
        assert_eq!(phrases.first().map(|phrase| phrase.start_beat()), Some(0));
        for (phrase, source_phrase) in phrases.iter().zip(&source.phrases) {
            assert_eq!(
                phrase.start_beat(),
                source_phrase
                    .start_beat
                    .checked_sub(canonical.source_beat_offset)
                    .ok_or("source phrase precedes the canonical beat grid")?
            );
        }

        for (canonical_index, marker) in canonical.beat_grid.markers().iter().enumerate() {
            let source_marker = &source.beat_grid[offset + canonical_index];
            assert_eq!(marker.time_millis(), u64::from(source_marker.time_millis));
            assert_eq!(
                marker.beat_in_bar(),
                u8::try_from(source_marker.beat_number)?
            );
        }
        Ok(())
    }

    #[test]
    fn exact_unique_metadata_can_match_when_usb_container_size_differs() {
        let candidate = DeviceMatchCandidate {
            track_id: lumi_domain::TrackId::new(42),
            source_id: "rekordbox7-local".to_owned(),
            source_kind: "rekordbox7".to_owned(),
            has_user_timeline_edits: false,
            title: "90s Bitch - Extended Mix".to_owned(),
            artist: "Maddix, The Rocketman".to_owned(),
            bpm_milli: 145_000,
            duration_millis: 192_000,
            audio_uri: "file://localhost/nonexistent/local-track.wav".to_owned(),
        };
        let device = DeviceTrack {
            device_track_id: 1_031,
            title: "90s Bitch - Extended Mix".to_owned(),
            artist: "Maddix, The Rocketman".to_owned(),
            musical_key: "4A".to_owned(),
            color_rgb: Some(0x32_80_ff),
            bpm_milli: 145_000,
            duration_millis: 192_000,
            file_size: 123_456,
            audio_path: PathBuf::from("/Volumes/Test/90s Bitch.wav"),
            analysis_dat_path: PathBuf::from("/Volumes/Test/USBANLZ/track.DAT"),
            metadata_revision: "metadata".to_owned(),
            analysis_revision: "analysis".to_owned(),
            analyzed_at: "2026-08-11".to_owned(),
            audio_signature: "signature".to_owned(),
            simulator_signature: 77,
            master_database_id: 1,
            master_content_id: 2,
            analysis_update_count: 3,
            information_update_count: 4,
            cue_update_count: 5,
        };

        assert!(device_metadata_matches(&candidate, &device));
        assert!(!device_track_matches(&candidate, &device));
    }

    #[test]
    #[ignore = "requires LUMI_DEVICE_POC_DATABASE and LUMI_REKORDBOX_DEVICE_ROOT"]
    fn mounted_device_sync_hydrates_the_same_canonical_track_by_real_and_simulator_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let database_path = std::env::var("LUMI_DEVICE_POC_DATABASE")?;
        let device_root = std::env::var("LUMI_REKORDBOX_DEVICE_ROOT")?;
        let device = lumi_rekordbox_device::read_device_library(&device_root)?;
        let mut worker = LibraryWorker::demo_at(std::path::Path::new(&database_path))?;

        let selected_playlist_id = std::env::var("LUMI_DEVICE_POC_PLAYLIST_ID")
            .ok()
            .and_then(|value| value.parse::<u32>().ok());
        let playlist_ids = device
            .playlists
            .iter()
            .map(|playlist| playlist.device_playlist_id)
            .filter(|playlist_id| selected_playlist_id.is_none_or(|value| value == *playlist_id))
            .collect::<Vec<_>>();
        let selected_track_count = device
            .playlists
            .iter()
            .filter(|playlist| playlist_ids.contains(&playlist.device_playlist_id))
            .flat_map(|playlist| playlist.track_ids.iter().copied())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let source_id = std::env::var("LUMI_DEVICE_POC_SOURCE_ID")
            .unwrap_or_else(|_| "usb-fs:mounted-device-poc".to_owned());
        let result = worker.sync_rekordbox_device(&device_root, Some(&source_id), &playlist_ids)?;
        assert_eq!(result.tracks, selected_track_count);
        assert!(result.matched > 0);
        assert!(result.refreshed_analyses <= result.matched);

        let (device_track, real) = device
            .tracks
            .values()
            .find_map(|device_track| {
                worker
                    .connected_track(device_track.device_track_id, 0)
                    .ok()
                    .flatten()
                    .map(|track| (device_track, track))
            })
            .ok_or("no synchronized device track resolved")?;
        let simulated = worker
            .connected_track(42, device_track.simulator_signature)?
            .ok_or("simulated identity did not resolve")?;
        assert_eq!(
            real.prepared.metadata.id(),
            simulated.prepared.metadata.id()
        );
        Ok(())
    }

    #[test]
    fn deck_waveform_preview_is_bounded_and_peak_preserving() {
        let mut waveform = (0..16_384)
            .map(|_| WaveformPoint::new(8, 16, 24))
            .collect::<Vec<_>>();
        waveform[8_191] = WaveformPoint::new(255, 254, 253);

        let preview = deck_waveform_preview_points(&waveform, 1_024);

        assert_eq!(preview.len(), 1_024);
        assert!(preview.contains(&[255, 254, 253]));
    }

    #[test]
    fn collection_total_is_independent_from_the_active_playlist()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.query(LibraryQueryUpdate {
            search: String::new(),
            playlist_id: Some(2),
            workflow_filter: None,
            workflow_step_id: None,
            offset: 0,
            limit: 50,
            sort: LibraryTrackSort::default(),
        });

        let snapshot = worker.snapshot_json()?;

        assert_eq!(snapshot["collectionTotal"], 3);
        assert_eq!(snapshot["page"]["total"], 2);
        Ok(())
    }

    #[test]
    fn library_reset_apply_rejects_phrase_work_created_after_review()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.preview_library_reset(&[])?;
        let token = worker
            .pending_library_reset
            .as_ref()
            .ok_or("reset preview")?
            .token
            .clone();
        worker.open_editor(1)?;
        worker.edit_timeline(
            1,
            1,
            TimelineEditCommand::ChangeRole {
                phrase_index: 0,
                role_id: PhraseRoleId::try_new("synth")?,
            },
        )?;

        let result = worker.apply_library_reset(&token, "/not/used/after/stale-review.sqlite");

        assert!(matches!(
            result,
            Err(LibraryWorkerError::LibraryResetPreviewChanged)
        ));
        assert!(worker.pending_library_reset.is_none());
        assert_eq!(worker.snapshot_json()?["collectionTotal"], 3);
        assert_eq!(
            worker
                .repository
                .timeline_head(TrackId::new(1))?
                .map(|head| head.revision().value()),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn simulator_track_uses_exact_lumi_revision_and_fails_closed_on_stale_or_unknown_matches()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.open_editor(1)?;
        worker.edit_timeline(
            1,
            1,
            TimelineEditCommand::ChangeRole {
                phrase_index: 0,
                role_id: PhraseRoleId::try_new("synth")?,
            },
        )?;

        let (metadata, context) = worker.simulator_track(1, 2)?.into_parts();
        let identity = metadata
            .identity_facts()
            .ok_or("library simulator track must include identity facts")?;
        assert_eq!(identity.provider_kind(), "demo");
        assert_eq!(identity.lumi_timeline_revision(), 2);
        assert_eq!(metadata.phrases()[0].kind(), PhraseKind::Build);
        let resolved = context.resolve(ThemeId::new(1))?;
        assert_eq!(resolved[0].role_id, "synth");
        assert_eq!(resolved[0].strategy, "auto");
        assert_eq!(resolved[0].entry_id, "theme-1--mapping-5");

        assert!(matches!(
            worker.simulator_track(1, 1),
            Err(LibraryWorkerError::TimelineRevisionConflict { .. })
        ));
        assert!(matches!(
            worker.simulator_track(999_999, 1),
            Err(LibraryWorkerError::UnknownTrack(999_999))
        ));
        Ok(())
    }

    #[test]
    fn default_phrase_roles_are_seeded_once_and_user_changes_survive_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("lumi-engine-roles-{unique}.sqlite"));
        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            let initial = worker.snapshot_json()?;
            let roles = initial["phraseRoleSettings"]["roles"]
                .as_array()
                .ok_or("phrase-role settings are missing")?;
            assert_eq!(initial["phraseRoleSettings"]["revision"], 1);
            assert_eq!(roles.len(), 11);
            assert_eq!(roles[0]["id"], "intro-outro");
            assert_eq!(roles[5]["id"], "synth");

            worker.mutate_phrase_role_catalog(
                1,
                PhraseRoleCatalogMutation::Rename {
                    role_id: PhraseRoleId::try_new("synth")?,
                    display_name: "Lead Synth".to_owned(),
                },
            )?;
            worker.mutate_phrase_role_catalog(
                2,
                PhraseRoleCatalogMutation::SetArchived {
                    role_id: PhraseRoleId::try_new("synth")?,
                    archived: true,
                },
            )?;
        }

        let worker = LibraryWorker::demo_at(&path)?;
        let restarted = worker.snapshot_json()?;
        let roles = restarted["phraseRoleSettings"]["roles"]
            .as_array()
            .ok_or("phrase-role settings are missing after restart")?;
        assert_eq!(restarted["phraseRoleSettings"]["revision"], 3);
        assert_eq!(roles.len(), 11);
        let synth = roles
            .iter()
            .find(|role| role["id"] == "synth")
            .ok_or("stable Synth role is missing")?;
        assert_eq!(synth["name"], "Lead Synth");
        assert_eq!(synth["archived"], true);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn mapping_changes_only_initialize_future_timelines_and_keep_raw_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let original_track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("demo track ID is missing")?;
        worker.open_editor(original_track_id)?;
        let before = worker.snapshot_json()?;
        assert_eq!(before["editor"]["phrases"][0]["roleId"], "intro-outro");
        assert_eq!(before["editor"]["sourcePhrases"][0]["rawLabel"], "Intro");

        worker.mutate_phrase_role_catalog(
            1,
            PhraseRoleCatalogMutation::SetSourceMapping {
                provider_kind: "demo".to_owned(),
                raw_label: "Demo".to_owned(),
                role_id: PhraseRoleId::try_new("synth")?,
            },
        )?;
        let unchanged = worker.snapshot_json()?;
        assert_eq!(unchanged["editor"]["timeline"]["revision"], 1);
        assert_eq!(unchanged["editor"]["phrases"][0]["roleId"], "intro-outro");

        let new_baseline = DemoLibrarySourceProvider::scaled(1)?.load_baseline()?;
        worker.repository.import_baseline(&new_baseline)?;
        let new_track = worker
            .repository
            .page_tracks(TrackPageRequest::try_new(0, 200)?)?
            .tracks()
            .iter()
            .find(|track| track.source_track_id().as_str() == "scale-00000")
            .ok_or("new scale track was not imported")?
            .id();
        worker.ensure_timeline(new_track)?;
        let timeline = worker
            .repository
            .timeline_head(new_track)?
            .ok_or("new track timeline was not initialized")?;
        assert_eq!(timeline.phrases()[0].role_id().as_str(), "synth");
        Ok(())
    }

    #[test]
    fn archived_roles_cannot_receive_mappings_or_initialize_future_timelines()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.mutate_phrase_role_catalog(
            1,
            PhraseRoleCatalogMutation::SetSourceMapping {
                provider_kind: "demo".to_owned(),
                raw_label: "Demo".to_owned(),
                role_id: PhraseRoleId::try_new("synth")?,
            },
        )?;
        worker.mutate_phrase_role_catalog(
            2,
            PhraseRoleCatalogMutation::SetArchived {
                role_id: PhraseRoleId::try_new("synth")?,
                archived: true,
            },
        )?;

        let rejected_mapping = worker.mutate_phrase_role_catalog(
            3,
            PhraseRoleCatalogMutation::SetSourceMapping {
                provider_kind: "demo".to_owned(),
                raw_label: "Intro".to_owned(),
                role_id: PhraseRoleId::try_new("synth")?,
            },
        );
        assert!(matches!(
            rejected_mapping,
            Err(super::LibraryWorkerError::ArchivedPhraseRole)
        ));

        let new_baseline = DemoLibrarySourceProvider::scaled(1)?.load_baseline()?;
        worker.repository.import_baseline(&new_baseline)?;
        let new_track = worker
            .repository
            .page_tracks(TrackPageRequest::try_new(0, 200)?)?
            .tracks()
            .iter()
            .find(|track| track.source_track_id().as_str() == "scale-00000")
            .ok_or("new scale track was not imported")?
            .id();
        let initialization = worker.ensure_timeline(new_track);
        assert!(matches!(
            initialization,
            Err(super::LibraryWorkerError::ArchivedSourcePhraseMapping { .. })
        ));
        Ok(())
    }

    #[test]
    fn phrase_role_usage_and_synth_assignment_are_exact_and_stale_safe()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("demo track ID is missing")?;
        worker.open_editor(track_id)?;
        worker.edit_timeline(
            track_id,
            1,
            TimelineEditCommand::ChangeRole {
                phrase_index: 0,
                role_id: PhraseRoleId::try_new("synth")?,
            },
        )?;
        let snapshot = worker.snapshot_json()?;
        assert_eq!(snapshot["editor"]["phrases"][0]["roleId"], "synth");
        let synth = snapshot["phraseRoleSettings"]["roles"]
            .as_array()
            .and_then(|roles| roles.iter().find(|role| role["id"] == "synth"))
            .ok_or("Synth usage is missing")?;
        assert_eq!(synth["usage"]["trackCount"], 1);
        assert_eq!(synth["usage"]["phraseCount"], 1);
        assert_eq!(synth["usage"]["catalogRowCount"], 4);

        let stale = worker.mutate_phrase_role_catalog(
            2,
            PhraseRoleCatalogMutation::Rename {
                role_id: PhraseRoleId::try_new("synth")?,
                display_name: "Lead Synth".to_owned(),
            },
        );
        assert!(matches!(
            stale,
            Err(
                super::LibraryWorkerError::PhraseRoleCatalogRevisionConflict {
                    expected: 2,
                    actual: 1,
                }
            )
        ));
        Ok(())
    }

    #[test]
    fn four_theme_autoloop_defaults_and_mutations_survive_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("lumi-engine-autoloops-{unique}.sqlite"));
        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            let initial = worker.snapshot_json()?;
            assert_eq!(initial["autoloopCatalog"]["revision"], 1);
            assert_eq!(
                initial["autoloopCatalog"]["themes"]
                    .as_array()
                    .map(Vec::len),
                Some(4)
            );
            let synth = initial["autoloopCatalog"]["roles"]
                .as_array()
                .and_then(|roles| roles.iter().find(|role| role["id"] == "synth"))
                .ok_or("Synth matrix row is missing")?;
            assert_eq!(synth["variants"].as_array().map(Vec::len), Some(4));
            assert_eq!(initial["autoloopCatalog"]["preflight"]["status"], "ready");

            worker.mutate_autoloop_catalog(
                1,
                AutoloopCatalogMutation::RenameTheme {
                    theme_id: ThemeId::new(1),
                    display_name: "Electric Garden".to_owned(),
                },
            )?;
            worker.mutate_autoloop_catalog(
                2,
                AutoloopCatalogMutation::AddVariant {
                    role_id: PhraseRoleId::try_new("synth")?,
                    display_name: "Variant 3".to_owned(),
                },
            )?;
            let incomplete = worker.snapshot_json()?;
            assert_eq!(
                incomplete["autoloopCatalog"]["preflight"]["missingCellCount"],
                4
            );
            let stale = worker.mutate_autoloop_catalog(
                1,
                AutoloopCatalogMutation::RenameTheme {
                    theme_id: ThemeId::new(2),
                    display_name: "Ocean Garden".to_owned(),
                },
            );
            assert!(matches!(
                stale,
                Err(LibraryWorkerError::AutoloopCatalogRevisionConflict {
                    expected: 1,
                    actual: 3,
                })
            ));
        }

        let worker = LibraryWorker::demo_at(&path)?;
        let restarted = worker.snapshot_json()?;
        assert_eq!(restarted["autoloopCatalog"]["revision"], 3);
        assert_eq!(
            restarted["autoloopCatalog"]["themes"][0]["name"],
            "Electric Garden"
        );
        assert_eq!(
            restarted["autoloopCatalog"]["preflight"]["missingCellCount"],
            4
        );
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn new_phrase_roles_block_preflight_until_they_have_a_variant()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.mutate_phrase_role_catalog(
            1,
            PhraseRoleCatalogMutation::Add {
                display_name: "Vocal".to_owned(),
            },
        )?;

        let uncovered = worker.snapshot_json()?;
        assert_eq!(
            uncovered["autoloopCatalog"]["preflight"]["missingRoleCount"],
            1
        );
        assert_eq!(
            uncovered["autoloopCatalog"]["preflight"]["missingRoleIds"][0],
            "custom-1"
        );
        assert_eq!(
            uncovered["autoloopCatalog"]["preflight"]["status"],
            "incomplete"
        );

        worker.mutate_autoloop_catalog(
            1,
            AutoloopCatalogMutation::AddVariant {
                role_id: PhraseRoleId::try_new("custom-1")?,
                display_name: "Variant 1".to_owned(),
            },
        )?;
        let covered = worker.snapshot_json()?;
        assert_eq!(
            covered["autoloopCatalog"]["preflight"]["missingRoleCount"],
            0
        );
        assert_eq!(
            covered["autoloopCatalog"]["preflight"]["missingCellCount"],
            4
        );
        Ok(())
    }

    #[test]
    fn phrase_loop_strategy_is_role_safe_revisioned_and_restart_persistent()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("lumi-engine-loop-strategy-{unique}.sqlite"));
        let track_id;
        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
                .as_u64()
                .ok_or("demo track ID is missing")?;
            worker.open_editor(track_id)?;
            worker.set_phrase_loop_strategy(
                track_id,
                1,
                1,
                0,
                PhraseLoopStrategy::FixedVariant(VariantId::try_new("mapping-1")?),
            )?;
            let fixed = worker.snapshot_json()?;
            assert_eq!(fixed["editor"]["timeline"]["revision"], 2);
            assert_eq!(fixed["editor"]["timeline"]["reason"], "changeLoopStrategy");
            assert_eq!(
                fixed["editor"]["phrases"][0]["loopStrategy"]["kind"],
                "fixedVariant"
            );
            assert_eq!(
                fixed["editor"]["phrases"][0]["loopStrategy"]["fixedVariantId"],
                "mapping-1"
            );
            assert_eq!(
                fixed["editor"]["phrases"][0]["loopStrategy"]["locked"],
                true
            );
        }
        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            worker.open_editor(track_id)?;
            let restarted = worker.snapshot_json()?;
            assert_eq!(
                restarted["editor"]["phrases"][0]["loopStrategy"]["kind"],
                "fixedVariant"
            );
            worker.mutate_autoloop_catalog(
                1,
                AutoloopCatalogMutation::RenameTheme {
                    theme_id: ThemeId::new(1),
                    display_name: "Electric Garden".to_owned(),
                },
            )?;
            let stale =
                worker.set_phrase_loop_strategy(track_id, 2, 1, 0, PhraseLoopStrategy::Auto);
            assert!(matches!(
                stale,
                Err(LibraryWorkerError::AutoloopCatalogRevisionConflict {
                    expected: 1,
                    actual: 2,
                })
            ));
            worker.set_phrase_loop_strategy(track_id, 2, 2, 0, PhraseLoopStrategy::Auto)?;
        }
        let mut worker = LibraryWorker::demo_at(&path)?;
        worker.open_editor(track_id)?;
        let automatic = worker.snapshot_json()?;
        assert_eq!(automatic["editor"]["timeline"]["revision"], 3);
        assert_eq!(
            automatic["editor"]["phrases"][0]["loopStrategy"]["kind"],
            "auto"
        );
        assert_eq!(
            automatic["editor"]["phrases"][0]["loopStrategy"]["locked"],
            false
        );
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn editor_snapshot_exposes_read_only_analysis_and_closes_cleanly()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let collection = worker.snapshot_json()?;
        let track_id = collection["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("demo track ID is missing")?;

        worker.open_editor(track_id)?;
        let opened = worker.snapshot_json()?;
        assert_eq!(opened["editor"]["track"]["id"], track_id);
        assert_eq!(opened["editor"]["beatGrid"]["beatsPerBar"], 4);
        assert!(
            opened["editor"]["beatGrid"]["markers"]
                .as_array()
                .is_some_and(|markers| !markers.is_empty())
        );
        assert!(
            opened["editor"]["waveform"]
                .as_array()
                .is_some_and(|points| !points.is_empty())
        );
        assert!(
            opened["editor"]["phrases"]
                .as_array()
                .is_some_and(|phrases| !phrases.is_empty())
        );

        worker.close_editor();
        assert!(worker.snapshot_json()?["editor"].is_null());
        Ok(())
    }

    #[test]
    fn unknown_editor_track_is_rejected_without_changing_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        assert!(worker.open_editor(u64::MAX).is_err());
        assert!(worker.snapshot_json()?["editor"].is_null());
        Ok(())
    }

    #[test]
    fn source_mapping_becomes_the_authoritative_lumi_timeline()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let imported = worker.snapshot_json()?;
        assert_eq!(imported["page"]["tracks"][0]["timelineRevision"], 1);
        let track_id = imported["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("track id")?;

        worker.open_editor(track_id)?;
        let snapshot = worker.snapshot_json()?;
        assert_eq!(snapshot["editor"]["timeline"]["revision"], 1);
        assert_eq!(
            snapshot["editor"]["timeline"]["reason"],
            "initialSourceMapping"
        );
        assert_eq!(snapshot["editor"]["timeline"]["canUndo"], false);
        assert_eq!(snapshot["editor"]["phrases"][0]["roleId"], "intro-outro");
        assert!(
            snapshot["editor"]["phrases"]
                .as_array()
                .is_some_and(|phrases| phrases.iter().all(|phrase| {
                    phrase["startBeat"]
                        .as_u64()
                        .is_some_and(|value| value % 4 == 0)
                        && phrase["endBeat"]
                            .as_u64()
                            .is_some_and(|value| value % 4 == 0)
                }))
        );
        Ok(())
    }

    #[test]
    fn edit_undo_redo_restore_and_stale_rejection_are_deterministic()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("track id")?;
        worker.open_editor(track_id)?;

        worker.edit_timeline(
            track_id,
            1,
            TimelineEditCommand::Split {
                phrase_index: 0,
                at_beat: 4,
            },
        )?;
        let edited = worker.snapshot_json()?;
        assert_eq!(edited["editor"]["timeline"]["revision"], 2);
        assert_eq!(edited["editor"]["timeline"]["reason"], "splitPhrase");
        assert_eq!(edited["editor"]["timeline"]["canUndo"], true);
        assert_eq!(
            edited["editor"]["phrases"].as_array().map(Vec::len),
            Some(5)
        );

        let stale = worker.edit_timeline(
            track_id,
            1,
            TimelineEditCommand::ChangeRole {
                phrase_index: 0,
                role_id: PhraseRoleId::try_new("synth")?,
            },
        );
        assert!(matches!(
            stale,
            Err(super::LibraryWorkerError::TimelineRevisionConflict { .. })
        ));

        worker.undo_timeline(track_id, 2)?;
        let undone = worker.snapshot_json()?;
        assert_eq!(undone["editor"]["timeline"]["revision"], 3);
        assert_eq!(undone["editor"]["timeline"]["reason"], "undo");
        assert_eq!(undone["editor"]["timeline"]["canRedo"], true);
        assert_eq!(
            undone["editor"]["phrases"].as_array().map(Vec::len),
            Some(4)
        );

        worker.redo_timeline(track_id, 3)?;
        let redone = worker.snapshot_json()?;
        assert_eq!(redone["editor"]["timeline"]["revision"], 4);
        assert_eq!(redone["editor"]["timeline"]["reason"], "redo");
        assert_eq!(
            redone["editor"]["phrases"].as_array().map(Vec::len),
            Some(5)
        );

        worker.restore_timeline_revision(track_id, 4, 1)?;
        let restored = worker.snapshot_json()?;
        assert_eq!(restored["editor"]["timeline"]["revision"], 5);
        assert_eq!(restored["editor"]["timeline"]["reason"], "restoreRevision");
        assert_eq!(
            restored["editor"]["phrases"].as_array().map(Vec::len),
            Some(4)
        );
        Ok(())
    }

    #[test]
    fn phrase_protection_is_persisted_and_enforced_below_the_ui()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("track id")?;
        worker.open_editor(track_id)?;
        worker.set_track_phrase_protection(track_id, 0, true)?;
        let locked = worker.snapshot_json()?;
        assert_eq!(
            locked["editor"]["track"]["phraseProtection"]["locked"],
            true
        );
        assert_eq!(locked["editor"]["track"]["phraseProtection"]["revision"], 1);

        let rejected = worker.edit_timeline(
            track_id,
            1,
            TimelineEditCommand::Split {
                phrase_index: 0,
                at_beat: 4,
            },
        );
        assert!(matches!(
            rejected,
            Err(super::LibraryWorkerError::TrackPhrasesProtected)
        ));
        assert_eq!(worker.snapshot_json()?["editor"]["timeline"]["revision"], 1);

        worker.set_track_phrase_protection(track_id, 1, false)?;
        worker.edit_timeline(
            track_id,
            1,
            TimelineEditCommand::Split {
                phrase_index: 0,
                at_beat: 4,
            },
        )?;
        assert_eq!(worker.snapshot_json()?["editor"]["timeline"]["revision"], 2);
        Ok(())
    }

    #[test]
    fn phrase_protection_keeps_the_active_workflow_query_and_page_atomic()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("track id")?;
        worker.open_editor(track_id)?;
        worker.query(LibraryQueryUpdate {
            search: String::new(),
            playlist_id: None,
            workflow_filter: Some(lumi_library::TrackWorkflowFilter::ChangedAfterUsbSync),
            workflow_step_id: None,
            offset: 0,
            limit: 50,
            sort: LibraryTrackSort::default(),
        });

        let before = worker.snapshot_json()?;
        assert_eq!(before["query"]["workflowFilter"], "changedAfterUsbSync");
        assert_eq!(before["page"]["total"], 0);

        worker.set_track_phrase_protection(track_id, 0, true)?;
        let after = worker.snapshot_json()?;
        assert_eq!(after["query"]["workflowFilter"], "changedAfterUsbSync");
        assert_eq!(after["page"]["total"], 0);
        assert_eq!(after["editor"]["track"]["phraseProtection"]["locked"], true);
        Ok(())
    }

    #[test]
    fn timeline_and_undo_redo_cursor_survive_worker_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("lumi-engine-history-{unique}.sqlite"));
        let track_id;
        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
                .as_u64()
                .ok_or("track id")?;
            worker.open_editor(track_id)?;
            worker.edit_timeline(
                track_id,
                1,
                TimelineEditCommand::Split {
                    phrase_index: 0,
                    at_beat: 4,
                },
            )?;
            worker.undo_timeline(track_id, 2)?;
        }

        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            worker.open_editor(track_id)?;
            let restored = worker.snapshot_json()?;
            assert_eq!(restored["editor"]["timeline"]["revision"], 3);
            assert_eq!(restored["editor"]["timeline"]["canRedo"], true);
            worker.redo_timeline(track_id, 3)?;
            assert_eq!(worker.snapshot_json()?["editor"]["timeline"]["revision"], 4);
        }
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn source_refresh_is_previewed_then_explicitly_reconciled()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let horizon_id = worker.snapshot_json()?["page"]["tracks"]
            .as_array()
            .ok_or("tracks")?
            .iter()
            .find(|track| track["sourceTrackId"] == "horizon-lines")
            .and_then(|track| track["id"].as_u64())
            .ok_or("horizon id")?;
        worker.open_editor(horizon_id)?;
        worker.preview_demo_source_refresh()?;
        let preview = worker.snapshot_json()?;
        assert_eq!(preview["source"]["status"], "changesAvailable");
        assert_eq!(preview["sourceRefresh"]["changeCount"], 3);
        assert_eq!(
            preview["editor"]["sourceReconciliation"]["toRevision"],
            "horizon-lines-v2"
        );
        assert!(
            preview["editor"]["sourceReconciliation"]["conflicts"]
                .as_array()
                .is_some_and(|conflicts| !conflicts.is_empty())
        );
        let golden: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../fixtures/source-reconciliation/horizon-lines-preview.json"
        ))?;
        assert_eq!(preview["editor"]["sourceReconciliation"], golden);

        worker.reconcile_source_refresh(horizon_id, 1, ReconcileStrategy::ReplaceWithSource)?;
        let reconciled = worker.snapshot_json()?;
        assert_eq!(reconciled["editor"]["timeline"]["revision"], 2);
        assert_eq!(
            reconciled["editor"]["timeline"]["reason"],
            "sourceReconcile"
        );
        assert!(reconciled["editor"]["sourceReconciliation"].is_null());
        assert_eq!(reconciled["sourceRefresh"]["changeCount"], 2);

        worker.close_editor();
        let afterglow_id = reconciled["page"]["tracks"]
            .as_array()
            .ok_or("tracks")?
            .iter()
            .find(|track| track["sourceTrackId"] == "afterglow-drive")
            .and_then(|track| track["id"].as_u64())
            .ok_or("afterglow id")?;
        worker.open_editor(afterglow_id)?;
        worker.reconcile_source_refresh(afterglow_id, 1, ReconcileStrategy::KeepLumi)?;
        let metadata_refresh = worker.snapshot_json()?;
        assert_eq!(
            metadata_refresh["editor"]["track"]["title"],
            "Afterglow Drive (Extended)"
        );
        assert_eq!(metadata_refresh["editor"]["timeline"]["revision"], 1);
        assert_eq!(metadata_refresh["sourceRefresh"]["changeCount"], 1);
        Ok(())
    }

    #[test]
    fn epic_two_a_golden_survives_restart_refresh_and_four_theme_resolution()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("lumi-epic-2a-golden-{unique}.sqlite"));
        let horizon_id;

        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            let initial = worker.snapshot_json()?;
            let tracks = initial["page"]["tracks"]
                .as_array()
                .ok_or("demo tracks are missing")?;
            let track_id = |source_track_id: &str| {
                tracks
                    .iter()
                    .find(|track| track["sourceTrackId"] == source_track_id)
                    .and_then(|track| track["id"].as_u64())
                    .ok_or("demo track ID is missing")
            };
            horizon_id = track_id("horizon-lines")?;
            let afterglow_id = track_id("afterglow-drive")?;
            let northern_id = track_id("northern-pulse")?;

            worker.query(LibraryQueryUpdate {
                search: "Horizon Lines".to_owned(),
                playlist_id: None,
                workflow_filter: None,
                workflow_step_id: None,
                offset: 0,
                limit: 50,
                sort: LibraryTrackSort::default(),
            });
            let browsed = worker.snapshot_json()?;
            assert_eq!(browsed["page"]["total"], 1);
            assert_eq!(browsed["page"]["tracks"][0]["id"], horizon_id);

            worker.open_editor(horizon_id)?;
            worker.edit_timeline(
                horizon_id,
                1,
                TimelineEditCommand::ChangeRole {
                    phrase_index: 0,
                    role_id: PhraseRoleId::try_new("synth")?,
                },
            )?;
            worker.set_phrase_loop_strategy(
                horizon_id,
                2,
                1,
                0,
                PhraseLoopStrategy::FixedVariant(VariantId::try_new("mapping-5")?),
            )?;
            worker.mutate_phrase_role_catalog(
                1,
                PhraseRoleCatalogMutation::Rename {
                    role_id: PhraseRoleId::try_new("synth")?,
                    display_name: "Lead Synth".to_owned(),
                },
            )?;

            worker.preview_demo_source_refresh()?;
            assert_eq!(worker.snapshot_json()?["sourceRefresh"]["changeCount"], 3);
            worker.reconcile_source_refresh(horizon_id, 3, ReconcileStrategy::KeepLumi)?;

            worker.close_editor();
            worker.open_editor(afterglow_id)?;
            worker.reconcile_source_refresh(afterglow_id, 1, ReconcileStrategy::KeepLumi)?;

            worker.close_editor();
            worker.open_editor(northern_id)?;
            worker.reconcile_source_refresh(northern_id, 1, ReconcileStrategy::KeepLumi)?;
            let refreshed = worker.snapshot_json()?;
            assert!(refreshed["sourceRefresh"].is_null());
            assert_eq!(refreshed["source"]["revision"], "demo-library-v2");
        }

        let mut restarted = LibraryWorker::demo_at(&path)?;
        restarted.open_editor(horizon_id)?;
        let snapshot = restarted.snapshot_json()?;
        assert_eq!(snapshot["source"]["revision"], "demo-library-v2");
        assert_eq!(snapshot["editor"]["timeline"]["revision"], 4);
        assert_eq!(snapshot["editor"]["phrases"][0]["roleId"], "synth");
        assert_eq!(
            snapshot["editor"]["phrases"][0]["loopStrategy"]["fixedVariantId"],
            "mapping-5"
        );

        let (_, context) = restarted.simulator_track(horizon_id, 4)?.into_parts();
        let theme_resolution = (1..=4)
            .map(|theme_id| {
                let cue = context
                    .resolve(ThemeId::new(theme_id))?
                    .into_iter()
                    .next()
                    .ok_or("resolved Theme has no first cue")?;
                Ok(json!({
                    "themeId": theme_id,
                    "roleId": cue.role_id,
                    "strategy": cue.strategy,
                    "variantId": cue.variant_id,
                    "entryId": cue.entry_id,
                }))
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        let synth_role = snapshot["phraseRoleSettings"]["roles"]
            .as_array()
            .and_then(|roles| roles.iter().find(|role| role["id"] == "synth"))
            .ok_or("persisted Synth role is missing")?;
        let peak_time_playlist = snapshot["playlists"]
            .as_array()
            .and_then(|playlists| {
                playlists
                    .iter()
                    .find(|playlist| playlist["sourcePlaylistId"] == "peak-time")
            })
            .ok_or("persisted Peak Time playlist is missing")?;
        assert_eq!(peak_time_playlist["name"], "Peak Time 2026");
        let phrase_roles = snapshot["editor"]["phrases"]
            .as_array()
            .ok_or("persisted phrases are missing")?
            .iter()
            .map(|phrase| phrase["roleId"].clone())
            .collect::<Vec<_>>();
        let evidence = json!({
            "scenarioVersion": 1,
            "offline": true,
            "source": {
                "id": snapshot["source"]["id"],
                "providerKind": snapshot["providerKind"],
                "revision": snapshot["source"]["revision"],
                "status": snapshot["source"]["status"],
                "peakTimePlaylist": peak_time_playlist["name"],
            },
            "browse": {
                "query": "Horizon Lines",
                "resultCount": 1,
                "trackId": horizon_id,
                "sourceTrackId": snapshot["editor"]["track"]["sourceTrackId"],
            },
            "editor": {
                "analysisRevision": snapshot["editor"]["track"]["analysisRevision"],
                "timelineRevision": snapshot["editor"]["timeline"]["revision"],
                "baselineRevision": snapshot["editor"]["timeline"]["baselineRevision"],
                "reason": snapshot["editor"]["timeline"]["reason"],
                "phraseRoles": phrase_roles,
                "firstPhraseStartBeat": snapshot["editor"]["phrases"][0]["startBeat"],
                "firstPhraseEndBeat": snapshot["editor"]["phrases"][0]["endBeat"],
            },
            "phraseRoleSettings": {
                "revision": snapshot["phraseRoleSettings"]["revision"],
                "stableId": synth_role["id"],
                "displayName": synth_role["name"],
                "archived": synth_role["archived"],
            },
            "loopStrategy": {
                "kind": snapshot["editor"]["phrases"][0]["loopStrategy"]["kind"],
                "variantId": snapshot["editor"]["phrases"][0]["loopStrategy"]["fixedVariantId"],
                "catalogRevision": snapshot["editor"]["phrases"][0]["loopStrategy"]["validatedCatalogRevision"],
            },
            "themeResolution": theme_resolution,
            "persistence": {
                "workerRestarted": true,
                "sourceRefreshPending": !snapshot["sourceRefresh"].is_null(),
            },
        });
        let mut encoded = serde_json::to_vec_pretty(&evidence)?;
        encoded.push(b'\n');
        assert_eq!(
            String::from_utf8_lossy(&encoded),
            String::from_utf8_lossy(include_bytes!(
                "../../../../fixtures/epic-2a-v1/library-editor-e2e.json"
            ))
        );
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn interrupted_source_refresh_reopens_on_last_committed_source_and_resumes()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("lumi-refresh-recovery-{unique}.sqlite"));
        let horizon_id;

        {
            let mut worker = LibraryWorker::demo_at(&path)?;
            horizon_id = worker.snapshot_json()?["page"]["tracks"]
                .as_array()
                .and_then(|tracks| {
                    tracks
                        .iter()
                        .find(|track| track["sourceTrackId"] == "horizon-lines")
                })
                .and_then(|track| track["id"].as_u64())
                .ok_or("Horizon Lines is missing")?;
            worker.open_editor(horizon_id)?;
            worker.preview_demo_source_refresh()?;
            worker.reconcile_source_refresh(horizon_id, 1, ReconcileStrategy::ReplaceWithSource)?;
            let partial = worker.snapshot_json()?;
            assert_eq!(partial["source"]["revision"], "demo-library-v1");
            assert_eq!(partial["sourceRefresh"]["changeCount"], 2);
            assert_eq!(
                partial["editor"]["track"]["analysisRevision"],
                "horizon-lines-v2"
            );
            let latest = DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2)
                .load_baseline()?;
            worker.repository.restore_source_checkpoint(&latest)?;
        }

        let mut restarted = LibraryWorker::demo_at(&path)?;
        let recovered = restarted.snapshot_json()?;
        assert_eq!(recovered["source"]["revision"], "demo-library-v1");
        assert!(recovered["sourceRefresh"].is_null());
        let peak_time = recovered["playlists"]
            .as_array()
            .and_then(|playlists| {
                playlists
                    .iter()
                    .find(|playlist| playlist["sourcePlaylistId"] == "peak-time")
            })
            .ok_or("recovered Peak Time playlist is missing")?;
        assert_eq!(peak_time["name"], "Peak Time");
        restarted.preview_demo_source_refresh()?;
        let resumed = restarted.snapshot_json()?;
        assert_eq!(resumed["sourceRefresh"]["changeCount"], 2);
        assert_eq!(resumed["source"]["status"], "changesAvailable");

        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn playback_prefers_an_existing_device_audio_location_when_canonical_is_unmounted()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let mounted = std::env::temp_dir().join(format!("lumi-mounted-audio-{unique}.mp3"));
        std::fs::write(&mounted, b"test audio location")?;
        let mounted_uri = device_audio_uri(&mounted);
        let selected = first_available_audio_uri(
            "file://localhost/Volumes/Disconnected/Track.mp3",
            &[
                "file://localhost/Volumes/Also%20Disconnected/Track.mp3".to_owned(),
                mounted_uri.clone(),
            ],
        );
        assert_eq!(selected, mounted_uri);
        std::fs::remove_file(mounted)?;
        Ok(())
    }

    #[test]
    fn review_comparisons_remain_available_after_inspecting_another_usb()
    -> Result<(), Box<dyn std::error::Error>> {
        fn inspection(source_id: &str, device_track_id: u32) -> DeviceInspection {
            DeviceInspection {
                snapshot: DeviceLibrarySnapshot {
                    source_id: source_id.to_owned(),
                    display_name: source_id.to_owned(),
                    database_path: std::path::PathBuf::from("/tmp/exportLibrary.db"),
                    database_revision: "revision".to_owned(),
                    database_version: "1".to_owned(),
                    exported_at: "2026-08-23".to_owned(),
                    tracks: BTreeMap::new(),
                    playlists: Vec::new(),
                },
                selected_playlist_ids: Vec::new(),
                tracks: BTreeMap::new(),
                review_comparisons: BTreeMap::from([(
                    device_track_id,
                    DeviceReviewComparison {
                        beat_grid_changed: false,
                        hot_cues_changed: false,
                        file_data_changed: true,
                        raw_phrases_changed: false,
                        waveform_changed: true,
                        beat_grid_detail: "unchanged grid".to_owned(),
                        hot_cues_detail: "unchanged cues".to_owned(),
                        raw_phrases_detail: "unchanged phrases".to_owned(),
                        waveform_detail: "changed waveform".to_owned(),
                        file_detail: "changed file data".to_owned(),
                    },
                )]),
            }
        }

        let mut worker = LibraryWorker::demo()?;
        worker.remember_device_inspection(inspection("usb-chrm", 1031));
        worker.remember_device_inspection(inspection("usb-gray", 1256));

        assert!(worker.device_review_comparisons_by_source["usb-chrm"].contains_key(&1031));
        assert!(worker.device_review_comparisons_by_source["usb-gray"].contains_key(&1256));
        assert_eq!(
            worker
                .pending_device_inspection
                .as_ref()
                .map(|current| current.snapshot.source_id.as_str()),
            Some("usb-gray")
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires a mounted OneLibrary USB selected by LUMI_TEST_USB_ROOT"]
    fn mounted_usb_inspection_fits_the_local_protocol() -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::var("LUMI_TEST_USB_ROOT")?;
        let mut worker = LibraryWorker::demo()?;
        let trusted_source_id = std::env::var("LUMI_TEST_USB_SOURCE_ID").ok();
        worker.inspect_rekordbox_device(root, trusted_source_id.as_deref())?;
        let second_source_id = std::env::var("LUMI_TEST_USB_SECOND_SOURCE_ID").ok();
        if let Ok(second_root) = std::env::var("LUMI_TEST_USB_SECOND_ROOT") {
            worker.inspect_rekordbox_device(second_root, second_source_id.as_deref())?;
        }
        let encoded = serde_json::to_vec(&worker.snapshot_json()?)?;
        eprintln!("USB inspection snapshot: {} bytes", encoded.len());
        if let Ok(output_path) = std::env::var("LUMI_TEST_USB_SNAPSHOT_OUTPUT") {
            std::fs::write(output_path, &encoded)?;
        }
        assert!(encoded.len() <= lumi_protocol::MAX_MESSAGE_BYTES);
        if let (Some(first_source_id), Some(second_source_id)) =
            (trusted_source_id.as_deref(), second_source_id.as_deref())
        {
            let snapshot: serde_json::Value = serde_json::from_slice(&encoded)?;
            for source_id in [first_source_id, second_source_id] {
                let source = snapshot["rekordboxDevices"]
                    .as_array()
                    .and_then(|sources| {
                        sources
                            .iter()
                            .find(|source| source["sourceId"] == source_id)
                    })
                    .ok_or("inspected USB source missing from snapshot")?;
                assert!(source["reviewTracks"].as_array().is_some_and(|tracks| {
                    tracks.iter().all(|track| !track["components"].is_null())
                }));
            }
        }
        Ok(())
    }

    #[test]
    fn creative_timeline_reuse_is_revisioned_and_exact_beat_safe()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        let equal_fixture = DemoLibrarySourceProvider::scaled(2)?.load_baseline()?;
        worker.repository.import_baseline(&equal_fixture)?;
        worker.ensure_imported_timelines()?;
        let tracks = worker
            .repository
            .page_tracks(TrackPageRequest::try_new(0, 200)?)?
            .tracks()
            .to_vec();
        let pair = tracks.iter().enumerate().find_map(|(index, source)| {
            let source_beats = worker
                .repository
                .timeline_head(source.id())
                .ok()??
                .total_beats();
            tracks.iter().skip(index + 1).find_map(|target| {
                let target_beats = worker
                    .repository
                    .timeline_head(target.id())
                    .ok()??
                    .total_beats();
                (source_beats == target_beats).then_some((source.clone(), target.clone()))
            })
        });
        let (source, target) = pair.ok_or("demo fixture needs an equal-beat track pair")?;
        let source_head = worker
            .repository
            .timeline_head(source.id())?
            .ok_or("source")?;
        let edited = source_head.edit(TimelineEditCommand::ChangeRole {
            phrase_index: 0,
            role_id: PhraseRoleId::try_new("drop")?,
        })?;
        worker
            .repository
            .append_timeline_revision(&edited, Some(source_head.revision()))?;

        let target_before = worker
            .repository
            .timeline_head(target.id())?
            .ok_or("target")?;
        worker.open_editor(target.id().value())?;
        worker.reuse_creative_timeline(
            source.id().value(),
            target.id().value(),
            target_before.revision().value(),
        )?;

        let target_after = worker
            .repository
            .timeline_head(target.id())?
            .ok_or("target")?;
        assert_eq!(
            target_after.revision().value(),
            target_before.revision().value() + 1
        );
        assert_eq!(
            target_after.origin(),
            TimelineRevisionOrigin::RevisionRestore
        );
        assert_eq!(target_after.phrases(), edited.phrases());
        assert_eq!(
            worker
                .repository
                .timeline_head(source.id())?
                .ok_or("source")?,
            edited
        );
        Ok(())
    }
}
