use std::collections::BTreeMap;

use lumi_domain::{
    KeyMode, PhraseKind, PitchClass, ThemeId, TrackId, TrackIdentityFacts, TrackMetadata,
    TrackPhrase,
};
use lumi_library::{
    AutoloopCatalog, AutoloopCatalogError, AutoloopResolutionReason, AutoloopVariantMove,
    ImportedLibraryBaseline, ImportedTrackAnalysis, LibraryRepository, LibraryTrackQuery,
    LumiPhraseTimeline, PhraseInstance, PhraseLoopStrategy, PhraseRole, PhraseRoleCatalogError,
    PhraseRoleId, PhraseRoleMove, PlaylistId, ReconcileError, ReconcilePreview, ReconcileStrategy,
    SourceChangeClass, SourcePhraseMapping, SourceRevision, SourceTrackDiff, TimelineEditCommand,
    TimelineEditError, TimelineRevision, TimelineRevisionOrigin, TimelineRevisionReason,
    TimelineRevisionSummary, TrackPageRequest, TrackSummary, VariantId, reconcile_timeline,
};
use lumi_library_demo::{DemoLibraryError, DemoLibraryRevision, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider as _;
use lumi_library_sqlite::{SqliteLibraryError, SqliteLibraryRepository};
use serde_json::{Value, json};
use thiserror::Error;

use crate::autoloop_defaults::{AutoloopDefaultsError, seeded_autoloop_catalog};
use crate::phrase_role_defaults::{
    PhraseRoleDefaultsError, provider_display_name, seeded_phrase_role_catalog,
};

const DEFAULT_PAGE_LIMIT: u16 = 50;
const DATABASE_PATH_ENVIRONMENT: &str = "LUMI_LIBRARY_DATABASE_PATH";

pub struct LibraryWorker {
    repository: SqliteLibraryRepository,
    source_id: String,
    source_kind: String,
    source_name: String,
    source_revision: String,
    search: String,
    playlist_id: Option<PlaylistId>,
    offset: u32,
    limit: u16,
    editor_track_id: Option<TrackId>,
    pending_source_refresh: Option<ImportedLibraryBaseline>,
}

#[derive(Clone, Debug)]
pub struct LibraryLocalPlaybackTrack {
    metadata: TrackMetadata,
    context: LibraryPlanContext,
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
    beat_times_millis: Vec<u64>,
    waveform: Vec<lumi_library::WaveformPoint>,
    catalog: AutoloopCatalog,
    phrases: Vec<LibraryPhrasePlanContext>,
    autoloop_overrides: BTreeMap<u16, VariantId>,
}

#[derive(Clone, Debug)]
struct LibraryPhrasePlanContext {
    phrase_index: u16,
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

impl LibraryPlanContext {
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
        json!({
            "source": "localLibrary",
            "style": "rgb",
            "points": self.waveform.iter().map(|point| json!({
                "low": point.low() / 8,
                "mid": point.mid() / 8,
                "high": point.high() / 8,
            })).collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn beat_at_millis(&self, position_millis: u64) -> u32 {
        let index = self
            .beat_times_millis
            .partition_point(|marker| *marker <= position_millis);
        u32::try_from(index.saturating_sub(1)).unwrap_or(u32::MAX)
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

    pub fn resolve(
        &self,
        theme_id: ThemeId,
    ) -> Result<Vec<ResolvedLibraryCue>, AutoloopCatalogError> {
        self.phrases
            .iter()
            .map(|phrase| {
                let override_variant = self.autoloop_overrides.get(&phrase.phrase_index);
                let resolution = if let Some(variant_id) = override_variant {
                    self.catalog.resolve(
                        theme_id,
                        &phrase.role_id,
                        Some(variant_id),
                        self.catalog.revision(),
                    )?
                } else {
                    self.catalog.resolve_loop_strategy(
                        theme_id,
                        &phrase.role_id,
                        &phrase.strategy,
                        self.catalog.revision(),
                    )?
                };
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
                    strategy: "planOverride",
                    variant_id: cell.variant_id().as_str().to_owned(),
                    entry_id: cell.entry_id().as_str().to_owned(),
                    entry_name: cell.display_name().to_owned(),
                    bank_number: theme_id.value(),
                    autoloop_number: Some(autoloop_number),
                    catalog_revision: self.catalog.revision(),
                    resolution_reason: "planOverride".to_owned(),
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
        let repository = match std::env::var_os(DATABASE_PATH_ENVIRONMENT) {
            Some(path) => SqliteLibraryRepository::open(path)?,
            None => SqliteLibraryRepository::in_memory()?,
        };
        Self::demo_with_repository(repository)
    }

    #[cfg(test)]
    fn demo_at(path: &std::path::Path) -> Result<Self, LibraryWorkerError> {
        Self::demo_with_repository(SqliteLibraryRepository::open(path)?)
    }

    fn demo_with_repository(
        mut repository: SqliteLibraryRepository,
    ) -> Result<Self, LibraryWorkerError> {
        let provider = DemoLibrarySourceProvider::curated();
        let baseline = provider.load_baseline()?;
        if repository
            .page_tracks(TrackPageRequest::try_new(0, 1)?)?
            .total()
            == 0
        {
            repository.import_baseline(&baseline)?;
        }
        let latest_baseline =
            DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2).load_baseline()?;
        let persisted_before_recovery = repository
            .library_source(baseline.source_id())?
            .ok_or(LibraryWorkerError::MissingLibrarySource)?;
        match repository.complete_source_refresh(&latest_baseline) {
            Ok(()) => {}
            Err(SqliteLibraryError::IncompleteSourceRefresh(_))
                if persisted_before_recovery.revision() == latest_baseline.source_revision() =>
            {
                repository.restore_source_checkpoint(&baseline)?;
            }
            Err(SqliteLibraryError::IncompleteSourceRefresh(_)) => {}
            Err(error) => return Err(error.into()),
        }
        let persisted_source = repository
            .library_source(baseline.source_id())?
            .ok_or(LibraryWorkerError::MissingLibrarySource)?;
        seed_default_role_catalog(&mut repository)?;
        seed_default_autoloop_catalog(&mut repository)?;
        let mut worker = Self {
            repository,
            source_id: persisted_source.id().as_str().to_owned(),
            source_kind: persisted_source.kind().to_owned(),
            source_name: persisted_source.display_name().to_owned(),
            source_revision: persisted_source.revision().as_str().to_owned(),
            search: String::new(),
            playlist_id: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
            editor_track_id: None,
            pending_source_refresh: None,
        };
        let track_ids = worker
            .repository
            .page_tracks(TrackPageRequest::try_new(0, 200)?)?
            .tracks()
            .iter()
            .map(TrackSummary::id)
            .collect::<Vec<_>>();
        for track_id in track_ids {
            worker.ensure_timeline(track_id)?;
        }
        Ok(worker)
    }

    pub fn query(&mut self, search: String, playlist_id: Option<u64>, offset: u32, limit: u16) {
        self.search = search;
        self.playlist_id = playlist_id.map(PlaylistId::new);
        self.offset = offset;
        self.limit = limit;
    }

    pub fn open_editor(&mut self, track_id: u64) -> Result<(), LibraryWorkerError> {
        let track_id = TrackId::new(track_id);
        self.ensure_timeline(track_id)?;
        self.editor_track_id = Some(track_id);
        Ok(())
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
            audio_uri: track.audio_uri().to_owned(),
            duration_millis: track.summary().duration_millis(),
            beat_times_millis: track
                .beat_grid()
                .markers()
                .iter()
                .map(|marker| marker.time_millis())
                .collect(),
            waveform: track.waveform().to_vec(),
            catalog,
            autoloop_overrides: BTreeMap::new(),
            phrases: timeline
                .phrases()
                .iter()
                .map(|phrase| LibraryPhrasePlanContext {
                    phrase_index: phrase.index(),
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
        let request = TrackPageRequest::try_new(self.offset, self.limit)?;
        let query = LibraryTrackQuery::try_new(self.search.clone(), self.playlist_id, request)?;
        let page = self.repository.query_tracks(&query)?;
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
        let source_refresh = match &self.pending_source_refresh {
            Some(baseline) => json!({
                "revision": baseline.source_revision().as_str(),
                "changeCount": self.pending_source_change_count()?,
            }),
            None => Value::Null,
        };
        Ok(json!({
            "condition": if page.total() == 0 && self.search.is_empty() && self.playlist_id.is_none() {
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
            "capabilities": {
                "playlists": true,
                "color": true,
                "beatGrid": true,
                "waveform": true,
                "rawPhrases": true,
                "localAudio": true,
            },
            "collectionTotal": collection_total,
            "query": {
                "search": self.search,
                "playlistId": self.playlist_id.map(|id| id.value()),
                "offset": page.offset(),
                "limit": self.limit,
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
                "tracks": page.tracks().iter().map(track_json).collect::<Vec<_>>(),
            },
            "phraseRoleSettings": self.phrase_role_settings_json()?,
            "autoloopCatalog": self.autoloop_catalog_json()?,
            "editor": self.editor_json()?,
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
        Ok(json!({
            "track": track_json(track.summary()),
            "audioUri": track.audio_uri(),
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
        }))
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
) -> Result<(), LibraryWorkerError> {
    let existing = repository.phrase_role_catalog()?;
    if existing.defaults_version() >= lumi_library::PHRASE_ROLE_DEFAULTS_VERSION {
        return Ok(());
    }
    let seeded = seeded_phrase_role_catalog(&existing)?;
    repository.initialize_phrase_role_catalog(&seeded)?;
    Ok(())
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

fn track_json(track: &TrackSummary) -> Value {
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
        "readiness": {
            "status": "ready",
            "missingCapabilities": [],
            "warnings": [],
        },
    })
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
    #[error("demo library failed: {0}")]
    Demo(#[from] DemoLibraryError),
    #[error("library persistence failed: {0}")]
    Persistence(#[from] SqliteLibraryError),
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
    #[error("the track editor selection changed")]
    EditorTrackMismatch,
    #[error("the selected track has no Lumi timeline")]
    MissingTimeline,
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use lumi_domain::{PhraseKind, ThemeId};
    use lumi_library::{
        LibraryRepository as _, PhraseLoopStrategy, PhraseRoleId, ReconcileStrategy,
        TimelineEditCommand, TrackPageRequest, VariantId,
    };
    use lumi_library_demo::{DemoLibraryRevision, DemoLibrarySourceProvider};
    use lumi_library_source::MusicLibrarySourceProvider as _;
    use serde_json::json;

    use super::{
        AutoloopCatalogMutation, LibraryWorker, LibraryWorkerError, PhraseRoleCatalogMutation,
    };

    #[test]
    fn collection_total_is_independent_from_the_active_playlist()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut worker = LibraryWorker::demo()?;
        worker.query(String::new(), Some(2), 0, 50);

        let snapshot = worker.snapshot_json()?;

        assert_eq!(snapshot["collectionTotal"], 3);
        assert_eq!(snapshot["page"]["total"], 2);
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

            worker.query("Horizon Lines".to_owned(), None, 0, 50);
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
}
