use std::collections::BTreeMap;

use lumi_domain::{KeyMode, PitchClass, TrackId};
use lumi_library::{
    LibraryRepository, LibraryTrackQuery, LumiPhraseTimeline, PhraseInstance, PhraseLoopStrategy,
    PhraseRole, PhraseRoleCatalogError, PhraseRoleId, PhraseRoleMove, PlaylistId,
    SourcePhraseMapping, SourceRevision, TimelineEditCommand, TimelineEditError, TimelineRevision,
    TimelineRevisionOrigin, TimelineRevisionReason, TimelineRevisionSummary, TrackPageRequest,
    TrackSummary,
};
use lumi_library_demo::{DemoLibraryError, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider as _;
use lumi_library_sqlite::{SqliteLibraryError, SqliteLibraryRepository};
use serde_json::{Value, json};
use thiserror::Error;

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
        repository.import_baseline(&baseline)?;
        seed_default_role_catalog(&mut repository)?;
        let mut worker = Self {
            repository,
            source_id: baseline.source_id().as_str().to_owned(),
            source_kind: baseline.source_kind().to_owned(),
            source_name: baseline.display_name().to_owned(),
            source_revision: baseline.source_revision().as_str().to_owned(),
            search: String::new(),
            playlist_id: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
            editor_track_id: None,
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
        let beats_per_bar = u32::from(track.beat_grid().beats_per_bar());
        let total_beats = u32::try_from(track.beat_grid().markers().len())
            .map_err(|_| LibraryWorkerError::InvalidSourceTimeline)?;
        if total_beats == 0 || !total_beats.is_multiple_of(beats_per_bar) {
            return Err(LibraryWorkerError::InvalidSourceTimeline);
        }
        let total_bars = total_beats / beats_per_bar;
        let role_catalog = self.repository.phrase_role_catalog()?;
        let mut phrases = Vec::with_capacity(track.raw_phrases().len());
        for (index, phrase) in track.raw_phrases().iter().enumerate() {
            if !phrase.start_beat().is_multiple_of(beats_per_bar)
                || !phrase.end_beat().is_multiple_of(beats_per_bar)
            {
                return Err(LibraryWorkerError::InvalidSourceTimeline);
            }
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
                phrase.start_beat() / beats_per_bar,
                phrase.end_beat() / beats_per_bar,
                role_id,
            ));
        }
        let timeline = LumiPhraseTimeline::try_new_with_history(
            track_id,
            TimelineRevision::initial(),
            SourceRevision::try_new(track.summary().source_revision().as_str())?,
            total_bars,
            TimelineRevisionOrigin::SourceImport,
            TimelineRevisionReason::InitialSourceMapping,
            None,
            None,
            phrases,
        )?;
        self.repository.append_timeline_revision(&timeline, None)?;
        Ok(())
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
                "status": "current",
            },
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
                        "catalogRowCount": usage.map_or(0, |value| value.catalog_row_count()),
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
        let history = self.rebuild_history(track_id)?;
        let revisions = self
            .repository
            .timeline_revisions(track_id, TrackPageRequest::try_new(0, 200)?)?;
        let beats_per_bar = u32::from(track.beat_grid().beats_per_bar());
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
                "startBeat": phrase.start_bar() * beats_per_bar,
                "endBeat": phrase.end_bar() * beats_per_bar,
                "roleId": phrase.role_id().as_str(),
                "role": role_display_name(roles, phrase.role_id()),
                "origin": origin_name(timeline.origin()),
                "loopStrategy": loop_strategy_name(phrase.loop_strategy()),
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

fn role_display_name(roles: &[PhraseRole], id: &PhraseRoleId) -> String {
    roles
        .iter()
        .find(|role| role.id() == id)
        .map(PhraseRole::display_name)
        .unwrap_or_else(|| id.as_str())
        .to_owned()
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
    #[error("invalid library identifier: {0}")]
    Identifier(#[from] lumi_library::TextIdentifierError),
    #[error("timeline edit was rejected: {0}")]
    TimelineEdit(#[from] TimelineEditError),
    #[error("persisted timeline is invalid: {0}")]
    TimelineValidation(#[from] lumi_library::TimelineValidationError),
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
    #[error("timeline history is corrupt")]
    CorruptHistory,
    #[error("timeline history overflowed")]
    HistoryOverflow,
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use lumi_library::{
        LibraryRepository as _, PhraseRoleId, TimelineEditCommand, TrackPageRequest,
    };
    use lumi_library_demo::DemoLibrarySourceProvider;
    use lumi_library_source::MusicLibrarySourceProvider as _;

    use super::{LibraryWorker, PhraseRoleCatalogMutation};

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
        assert_eq!(synth["usage"]["catalogRowCount"], 0);

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
                at_bar: 4,
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
                    at_bar: 4,
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
}
