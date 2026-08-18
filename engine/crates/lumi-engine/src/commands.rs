use std::error::Error;
use std::fmt;

use lumi_domain::{
    DeckId, OperationCommand, PlanId, PlanRevision, SceneId, StateRevision, ThemeId, TrackLoadId,
};
use lumi_library::{
    AutoloopVariantMove, PhraseAbsorption, PhraseConflictChoice, PhraseLoopStrategy, PhraseRoleId,
    PhraseRoleMove, ReconcileSide, ReconcileStrategy, ThemeSpecificVariant, TimelineEditCommand,
    VariantId,
};
use lumi_light_plans::LightPlanningPolicy;
use lumi_midi_output::MidiAddress;
use lumi_protocol::{MessageEnvelope, MessageType};
use lumi_simulator::SimulationSpeed;
use serde_json::Value;

use crate::library::{AutoloopCatalogMutation, PhraseRoleCatalogMutation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanCommandContext {
    pub plan_id: PlanId,
    pub track_load_id: TrackLoadId,
    pub expected_revision: PlanRevision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeckSourceSelection {
    ConnectedDecks,
    LocalPlayback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionCommand {
    GetSnapshot {
        include_library: bool,
    },
    QueryLibrary {
        search: String,
        playlist_id: Option<u64>,
        offset: u32,
        limit: u16,
    },
    OpenLibraryTrackEditor {
        track_id: u64,
    },
    GetLibraryTrackWaveform {
        track_id: u64,
    },
    CloseLibraryTrackEditor,
    PreviewDemoSourceRefresh,
    PreviewRekordboxXmlSync {
        folder: String,
        followed_paths: Vec<String>,
        include_future_child_playlists: bool,
    },
    ApplyRekordboxXmlSync {
        folder: String,
        followed_paths: Vec<String>,
        include_future_child_playlists: bool,
        expected_content_sha256: String,
    },
    ImportRekordboxAnalysis {
        folder: String,
        followed_paths: Vec<String>,
        include_future_child_playlists: bool,
        expected_content_sha256: String,
    },
    InspectRekordboxDevice {
        root: String,
        source_id: Option<String>,
    },
    SyncRekordboxDevice {
        root: String,
        source_id: Option<String>,
        playlist_ids: Vec<u32>,
    },
    PreviewLibraryReset {
        preserve_track_ids: Vec<u64>,
    },
    ApplyLibraryReset {
        expected_token: String,
        backup_database_path: String,
    },
    CreateLibraryBackup {
        destination: String,
    },
    RestoreLibraryBackup {
        source: String,
        rollback: String,
    },
    ReconcileLibrarySource {
        track_id: u64,
        expected_revision: u64,
        strategy: ReconcileStrategy,
    },
    EditLibraryTimeline {
        track_id: u64,
        expected_revision: u64,
        command: TimelineEditCommand,
    },
    SetLibraryPhraseLoopStrategy {
        track_id: u64,
        expected_timeline_revision: u64,
        expected_catalog_revision: u64,
        phrase_index: u16,
        strategy: PhraseLoopStrategy,
    },
    UndoLibraryTimeline {
        track_id: u64,
        expected_revision: u64,
    },
    RedoLibraryTimeline {
        track_id: u64,
        expected_revision: u64,
    },
    RestoreLibraryTimelineRevision {
        track_id: u64,
        expected_revision: u64,
        target_revision: u64,
    },
    MutatePhraseRoleCatalog {
        expected_revision: u64,
        mutation: PhraseRoleCatalogMutation,
    },
    MutateAutoloopCatalog {
        expected_revision: u64,
        mutation: AutoloopCatalogMutation,
    },
    ReplaceLightPlanningPolicy {
        expected_revision: u64,
        policy: LightPlanningPolicy,
    },
    PreviewLightPlan {
        track_id: u64,
        expected_timeline_revision: u64,
        theme_id: u64,
        variation_seed: u64,
        policy: LightPlanningPolicy,
    },
    PublishMidiSource,
    StopMidiSource,
    SetAbletonLinkEnabled {
        enabled: bool,
    },
    TestAbletonLinkHelper,
    SetOutputTimingOffset {
        millis: i16,
    },
    SendMidiLearnPulse,
    SendMidiAddressLearnPulse {
        address: MidiAddress,
    },
    TriggerMidiAutoloop {
        bank_number: u8,
        autoloop_number: u8,
    },
    LoadLibraryTrackOnLocalDeck {
        track_id: u64,
        deck_id: DeckId,
        expected_timeline_revision: u64,
        expected_state_revision: StateRevision,
    },
    UpdateLocalPlaybackTransport {
        deck_id: DeckId,
        track_load_id: TrackLoadId,
        position_millis: u64,
        playing: bool,
    },
    SetLocalPlaybackLeader {
        deck_id: DeckId,
        expected_state_revision: StateRevision,
    },
    SelectDeckSourceMode {
        mode: DeckSourceSelection,
        expected_state_revision: StateRevision,
    },
    LoadDemoSession {
        expected_revision: StateRevision,
    },
    SetOperationState {
        expected_revision: StateRevision,
        command: OperationCommand,
    },
    SetSimulationSpeed {
        expected_revision: StateRevision,
        speed: SimulationSpeed,
    },
    SetSimulationPlayback {
        expected_revision: StateRevision,
        playing: bool,
    },
    AdvanceSimulation {
        expected_revision: StateRevision,
        elapsed_ticks: u64,
    },
    AdvanceToNextTrack {
        expected_revision: StateRevision,
    },
    SelectTheme {
        context: PlanCommandContext,
        theme_id: ThemeId,
    },
    SelectThemeFromPhrase {
        context: PlanCommandContext,
        phrase_index: u16,
        theme_id: ThemeId,
    },
    SelectScene {
        context: PlanCommandContext,
        phrase_index: u16,
        scene_id: SceneId,
    },
    SetCueLock {
        context: PlanCommandContext,
        phrase_index: u16,
        locked: bool,
    },
    RegeneratePlan {
        context: PlanCommandContext,
    },
    ResetDemoSession {
        expected_revision: StateRevision,
    },
}

impl SessionCommand {
    pub const fn is_mutating(&self) -> bool {
        !matches!(
            self,
            Self::GetSnapshot { .. }
                | Self::QueryLibrary { .. }
                | Self::OpenLibraryTrackEditor { .. }
                | Self::GetLibraryTrackWaveform { .. }
                | Self::CloseLibraryTrackEditor
        )
    }

    pub const fn context(&self) -> Option<PlanCommandContext> {
        match self {
            Self::GetSnapshot { .. }
            | Self::QueryLibrary { .. }
            | Self::OpenLibraryTrackEditor { .. }
            | Self::GetLibraryTrackWaveform { .. }
            | Self::CloseLibraryTrackEditor
            | Self::PreviewDemoSourceRefresh
            | Self::PreviewRekordboxXmlSync { .. }
            | Self::ApplyRekordboxXmlSync { .. }
            | Self::ImportRekordboxAnalysis { .. }
            | Self::InspectRekordboxDevice { .. }
            | Self::SyncRekordboxDevice { .. }
            | Self::PreviewLibraryReset { .. }
            | Self::ApplyLibraryReset { .. }
            | Self::CreateLibraryBackup { .. }
            | Self::RestoreLibraryBackup { .. }
            | Self::ReconcileLibrarySource { .. }
            | Self::EditLibraryTimeline { .. }
            | Self::SetLibraryPhraseLoopStrategy { .. }
            | Self::UndoLibraryTimeline { .. }
            | Self::RedoLibraryTimeline { .. }
            | Self::RestoreLibraryTimelineRevision { .. }
            | Self::MutatePhraseRoleCatalog { .. }
            | Self::MutateAutoloopCatalog { .. }
            | Self::ReplaceLightPlanningPolicy { .. }
            | Self::PreviewLightPlan { .. }
            | Self::PublishMidiSource
            | Self::StopMidiSource
            | Self::SetAbletonLinkEnabled { .. }
            | Self::TestAbletonLinkHelper
            | Self::SetOutputTimingOffset { .. }
            | Self::SendMidiLearnPulse
            | Self::SendMidiAddressLearnPulse { .. }
            | Self::TriggerMidiAutoloop { .. }
            | Self::LoadLibraryTrackOnLocalDeck { .. }
            | Self::UpdateLocalPlaybackTransport { .. }
            | Self::SetLocalPlaybackLeader { .. }
            | Self::SelectDeckSourceMode { .. }
            | Self::LoadDemoSession { .. }
            | Self::SetOperationState { .. }
            | Self::SetSimulationSpeed { .. }
            | Self::SetSimulationPlayback { .. }
            | Self::AdvanceSimulation { .. }
            | Self::AdvanceToNextTrack { .. }
            | Self::ResetDemoSession { .. } => None,
            Self::SelectTheme { context, .. }
            | Self::SelectThemeFromPhrase { context, .. }
            | Self::SelectScene { context, .. }
            | Self::SetCueLock { context, .. }
            | Self::RegeneratePlan { context } => Some(*context),
        }
    }

    pub const fn changes_library_revision(&self) -> bool {
        matches!(
            self,
            Self::ApplyRekordboxXmlSync { .. }
                | Self::ImportRekordboxAnalysis { .. }
                | Self::SyncRekordboxDevice { .. }
                | Self::ApplyLibraryReset { .. }
                | Self::RestoreLibraryBackup { .. }
                | Self::ReconcileLibrarySource { .. }
                | Self::EditLibraryTimeline { .. }
                | Self::SetLibraryPhraseLoopStrategy { .. }
                | Self::UndoLibraryTimeline { .. }
                | Self::RedoLibraryTimeline { .. }
                | Self::RestoreLibraryTimelineRevision { .. }
                | Self::MutatePhraseRoleCatalog { .. }
                | Self::MutateAutoloopCatalog { .. }
                | Self::ReplaceLightPlanningPolicy { .. }
        )
    }
}

pub fn decode_command(envelope: &MessageEnvelope) -> Result<SessionCommand, CommandDecodeError> {
    if envelope.message_type != MessageType::Command {
        return Err(CommandDecodeError::WrongMessageType);
    }
    let kind = string(&envelope.payload, "kind")?;
    match kind {
        "getSnapshot" => Ok(SessionCommand::GetSnapshot {
            include_library: optional_boolean(&envelope.payload, "includeLibrary")?.unwrap_or(true),
        }),
        "queryLibrary" => Ok(SessionCommand::QueryLibrary {
            search: library_search(&envelope.payload)?,
            playlist_id: optional_unsigned(&envelope.payload, "playlistId")?,
            offset: u32::try_from(optional_unsigned(&envelope.payload, "offset")?.unwrap_or(0))
                .map_err(|_| CommandDecodeError::InvalidField("offset"))?,
            limit: library_limit(optional_unsigned(&envelope.payload, "limit")?.unwrap_or(50))?,
        }),
        "openLibraryTrackEditor" => Ok(SessionCommand::OpenLibraryTrackEditor {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
        }),
        "getLibraryTrackWaveform" => Ok(SessionCommand::GetLibraryTrackWaveform {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
        }),
        "closeLibraryTrackEditor" => Ok(SessionCommand::CloseLibraryTrackEditor),
        "previewDemoSourceRefresh" => Ok(SessionCommand::PreviewDemoSourceRefresh),
        "previewRekordboxXmlSync" => Ok(SessionCommand::PreviewRekordboxXmlSync {
            folder: string(&envelope.payload, "folder")?.to_owned(),
            followed_paths: string_array(&envelope.payload, "followedPaths")?,
            include_future_child_playlists: boolean(
                &envelope.payload,
                "includeFutureChildPlaylists",
            )?,
        }),
        "applyRekordboxXmlSync" => Ok(SessionCommand::ApplyRekordboxXmlSync {
            folder: string(&envelope.payload, "folder")?.to_owned(),
            followed_paths: string_array(&envelope.payload, "followedPaths")?,
            include_future_child_playlists: boolean(
                &envelope.payload,
                "includeFutureChildPlaylists",
            )?,
            expected_content_sha256: string(&envelope.payload, "expectedContentSha256")?.to_owned(),
        }),
        "importRekordboxAnalysis" => Ok(SessionCommand::ImportRekordboxAnalysis {
            folder: string(&envelope.payload, "folder")?.to_owned(),
            followed_paths: string_array(&envelope.payload, "followedPaths")?,
            include_future_child_playlists: boolean(
                &envelope.payload,
                "includeFutureChildPlaylists",
            )?,
            expected_content_sha256: string(&envelope.payload, "expectedContentSha256")?.to_owned(),
        }),
        "inspectRekordboxDevice" => Ok(SessionCommand::InspectRekordboxDevice {
            root: string(&envelope.payload, "root")?.to_owned(),
            source_id: optional_string(&envelope.payload, "sourceId").map(str::to_owned),
        }),
        "syncRekordboxDevice" => Ok(SessionCommand::SyncRekordboxDevice {
            root: string(&envelope.payload, "root")?.to_owned(),
            source_id: optional_string(&envelope.payload, "sourceId").map(str::to_owned),
            playlist_ids: u32_array(&envelope.payload, "playlistIds")?,
        }),
        "previewLibraryReset" => Ok(SessionCommand::PreviewLibraryReset {
            preserve_track_ids: u64_array(&envelope.payload, "preserveTrackIds")?,
        }),
        "applyLibraryReset" => Ok(SessionCommand::ApplyLibraryReset {
            expected_token: string(&envelope.payload, "expectedResetToken")?.to_owned(),
            backup_database_path: string(&envelope.payload, "backupDatabasePath")?.to_owned(),
        }),
        "createLibraryBackup" => Ok(SessionCommand::CreateLibraryBackup {
            destination: string(&envelope.payload, "destination")?.to_owned(),
        }),
        "restoreLibraryBackup" => Ok(SessionCommand::RestoreLibraryBackup {
            source: string(&envelope.payload, "source")?.to_owned(),
            rollback: string(&envelope.payload, "rollback")?.to_owned(),
        }),
        "reconcileLibrarySource" => Ok(SessionCommand::ReconcileLibrarySource {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_revision: positive_unsigned(&envelope.payload, "expectedTimelineRevision")?,
            strategy: reconcile_strategy(&envelope.payload)?,
        }),
        "editLibraryTimeline" => Ok(SessionCommand::EditLibraryTimeline {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_revision: positive_unsigned(&envelope.payload, "expectedTimelineRevision")?,
            command: timeline_edit(&envelope.payload)?,
        }),
        "setLibraryPhraseLoopStrategy" => Ok(SessionCommand::SetLibraryPhraseLoopStrategy {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_timeline_revision: positive_unsigned(
                &envelope.payload,
                "expectedTimelineRevision",
            )?,
            expected_catalog_revision: positive_unsigned(
                &envelope.payload,
                "expectedAutoloopCatalogRevision",
            )?,
            phrase_index: phrase_index_value(&envelope.payload)?,
            strategy: phrase_loop_strategy(&envelope.payload)?,
        }),
        "undoLibraryTimeline" => Ok(SessionCommand::UndoLibraryTimeline {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_revision: positive_unsigned(&envelope.payload, "expectedTimelineRevision")?,
        }),
        "redoLibraryTimeline" => Ok(SessionCommand::RedoLibraryTimeline {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_revision: positive_unsigned(&envelope.payload, "expectedTimelineRevision")?,
        }),
        "restoreLibraryTimelineRevision" => Ok(SessionCommand::RestoreLibraryTimelineRevision {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_revision: positive_unsigned(&envelope.payload, "expectedTimelineRevision")?,
            target_revision: positive_unsigned(&envelope.payload, "targetTimelineRevision")?,
        }),
        "mutatePhraseRoleCatalog" => Ok(SessionCommand::MutatePhraseRoleCatalog {
            expected_revision: positive_unsigned(&envelope.payload, "expectedPhraseRoleRevision")?,
            mutation: phrase_role_mutation(&envelope.payload)?,
        }),
        "mutateAutoloopCatalog" => Ok(SessionCommand::MutateAutoloopCatalog {
            expected_revision: positive_unsigned(
                &envelope.payload,
                "expectedAutoloopCatalogRevision",
            )?,
            mutation: autoloop_catalog_mutation(&envelope.payload)?,
        }),
        "replaceLightPlanningPolicy" => Ok(SessionCommand::ReplaceLightPlanningPolicy {
            expected_revision: positive_unsigned(
                &envelope.payload,
                "expectedLightPlanningRevision",
            )?,
            policy: serde_json::from_value(
                envelope
                    .payload
                    .get("policy")
                    .cloned()
                    .ok_or(CommandDecodeError::InvalidField("policy"))?,
            )
            .map_err(|_| CommandDecodeError::InvalidField("policy"))?,
        }),
        "previewLightPlan" => Ok(SessionCommand::PreviewLightPlan {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            expected_timeline_revision: positive_unsigned(
                &envelope.payload,
                "expectedTimelineRevision",
            )?,
            theme_id: positive_unsigned(&envelope.payload, "themeId")?,
            variation_seed: positive_unsigned(&envelope.payload, "variationSeed")?,
            policy: serde_json::from_value(
                envelope
                    .payload
                    .get("policy")
                    .cloned()
                    .ok_or(CommandDecodeError::InvalidField("policy"))?,
            )
            .map_err(|_| CommandDecodeError::InvalidField("policy"))?,
        }),
        "publishMidiSource" => Ok(SessionCommand::PublishMidiSource),
        "stopMidiSource" => Ok(SessionCommand::StopMidiSource),
        "setAbletonLinkEnabled" => Ok(SessionCommand::SetAbletonLinkEnabled {
            enabled: boolean(&envelope.payload, "enabled")?,
        }),
        "testAbletonLinkHelper" => Ok(SessionCommand::TestAbletonLinkHelper),
        "setOutputTimingOffset" => {
            let millis = signed(&envelope.payload, "millis")?;
            if !(-250..=250).contains(&millis) {
                return Err(CommandDecodeError::InvalidField("millis"));
            }
            Ok(SessionCommand::SetOutputTimingOffset {
                millis: i16::try_from(millis)
                    .map_err(|_| CommandDecodeError::InvalidField("millis"))?,
            })
        }
        "sendMidiLearnPulse" => Ok(SessionCommand::SendMidiLearnPulse),
        "sendMidiAddressLearnPulse" => Ok(SessionCommand::SendMidiAddressLearnPulse {
            address: midi_address(&envelope.payload)?,
        }),
        "triggerMidiAutoloop" => {
            let bank_number = midi_number(&envelope.payload, "bankNumber", 4)?;
            let autoloop_number = midi_number(&envelope.payload, "autoloopNumber", 32)?;
            Ok(SessionCommand::TriggerMidiAutoloop {
                bank_number,
                autoloop_number,
            })
        }
        "loadLibraryTrackOnLocalDeck" => Ok(SessionCommand::LoadLibraryTrackOnLocalDeck {
            track_id: positive_unsigned(&envelope.payload, "trackId")?,
            deck_id: DeckId::new(
                u8::try_from(positive_unsigned(&envelope.payload, "deckId")?)
                    .map_err(|_| CommandDecodeError::InvalidField("deckId"))?,
            ),
            expected_timeline_revision: positive_unsigned(
                &envelope.payload,
                "expectedTimelineRevision",
            )?,
            expected_state_revision: state_revision(envelope)?,
        }),
        "updateLocalPlaybackTransport" => Ok(SessionCommand::UpdateLocalPlaybackTransport {
            deck_id: DeckId::new(
                u8::try_from(positive_unsigned(&envelope.payload, "deckId")?)
                    .map_err(|_| CommandDecodeError::InvalidField("deckId"))?,
            ),
            track_load_id: TrackLoadId::new(positive_unsigned(&envelope.payload, "trackLoadId")?),
            position_millis: unsigned(&envelope.payload, "positionMillis")?,
            playing: boolean(&envelope.payload, "playing")?,
        }),
        "setLocalPlaybackLeader" => Ok(SessionCommand::SetLocalPlaybackLeader {
            deck_id: DeckId::new(
                u8::try_from(positive_unsigned(&envelope.payload, "deckId")?)
                    .map_err(|_| CommandDecodeError::InvalidField("deckId"))?,
            ),
            expected_state_revision: state_revision(envelope)?,
        }),
        "selectDeckSourceMode" => Ok(SessionCommand::SelectDeckSourceMode {
            mode: match string(&envelope.payload, "mode")? {
                "connectedDecks" => DeckSourceSelection::ConnectedDecks,
                "localPlayback" => DeckSourceSelection::LocalPlayback,
                _ => return Err(CommandDecodeError::InvalidField("mode")),
            },
            expected_state_revision: state_revision(envelope)?,
        }),
        "loadDemoSession" => Ok(SessionCommand::LoadDemoSession {
            expected_revision: state_revision(envelope)?,
        }),
        "setOperationState" => Ok(SessionCommand::SetOperationState {
            expected_revision: state_revision(envelope)?,
            command: operation_command(string(&envelope.payload, "operationState")?)?,
        }),
        "setSimulationSpeed" => Ok(SessionCommand::SetSimulationSpeed {
            expected_revision: state_revision(envelope)?,
            speed: simulation_speed(unsigned(&envelope.payload, "speed")?)?,
        }),
        "setSimulationPlayback" => Ok(SessionCommand::SetSimulationPlayback {
            expected_revision: state_revision(envelope)?,
            playing: boolean(&envelope.payload, "playing")?,
        }),
        "advanceSimulation" => Ok(SessionCommand::AdvanceSimulation {
            expected_revision: state_revision(envelope)?,
            elapsed_ticks: elapsed_ticks(envelope)?,
        }),
        "advanceToNextTrack" => Ok(SessionCommand::AdvanceToNextTrack {
            expected_revision: state_revision(envelope)?,
        }),
        "selectTheme" => Ok(SessionCommand::SelectTheme {
            context: context(envelope)?,
            theme_id: ThemeId::new(unsigned(&envelope.payload, "themeId")?),
        }),
        "selectThemeFromPhrase" => Ok(SessionCommand::SelectThemeFromPhrase {
            context: context(envelope)?,
            phrase_index: phrase_index(envelope)?,
            theme_id: ThemeId::new(unsigned(&envelope.payload, "themeId")?),
        }),
        "selectScene" => Ok(SessionCommand::SelectScene {
            context: context(envelope)?,
            phrase_index: phrase_index(envelope)?,
            scene_id: SceneId::new(unsigned(&envelope.payload, "sceneId")?),
        }),
        "setCueLock" => Ok(SessionCommand::SetCueLock {
            context: context(envelope)?,
            phrase_index: phrase_index(envelope)?,
            locked: boolean(&envelope.payload, "locked")?,
        }),
        "regeneratePlan" => Ok(SessionCommand::RegeneratePlan {
            context: context(envelope)?,
        }),
        "resetDemoSession" => Ok(SessionCommand::ResetDemoSession {
            expected_revision: state_revision(envelope)?,
        }),
        _ => Err(CommandDecodeError::UnsupportedKind),
    }
}

fn midi_address(
    payload: &serde_json::Map<String, Value>,
) -> Result<MidiAddress, CommandDecodeError> {
    let kind = string(payload, "targetKind")?;
    let raw_number = if kind == "custom" {
        unsigned(payload, "targetNumber")?
    } else {
        positive_unsigned(payload, "targetNumber")?
    };
    let number =
        u8::try_from(raw_number).map_err(|_| CommandDecodeError::InvalidField("targetNumber"))?;
    match kind {
        "bank" => MidiAddress::bank(number),
        "autoloop" => MidiAddress::autoloop(number),
        "custom" => MidiAddress::custom(
            u8::try_from(positive_unsigned(payload, "channel")?)
                .map_err(|_| CommandDecodeError::InvalidField("channel"))?,
            number,
        ),
        _ => return Err(CommandDecodeError::InvalidField("targetKind")),
    }
    .ok_or(CommandDecodeError::InvalidField("targetNumber"))
}

fn midi_number(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
    maximum: u8,
) -> Result<u8, CommandDecodeError> {
    let number = u8::try_from(positive_unsigned(payload, field)?)
        .map_err(|_| CommandDecodeError::InvalidField(field))?;
    if number <= maximum {
        Ok(number)
    } else {
        Err(CommandDecodeError::InvalidField(field))
    }
}

fn reconcile_strategy(
    payload: &serde_json::Map<String, Value>,
) -> Result<ReconcileStrategy, CommandDecodeError> {
    match string(payload, "strategy")? {
        "keepLumi" => Ok(ReconcileStrategy::KeepLumi),
        "rebase" => Ok(ReconcileStrategy::Rebase),
        "replaceWithSource" => Ok(ReconcileStrategy::ReplaceWithSource),
        "merge" => {
            let values = payload
                .get("choices")
                .and_then(Value::as_array)
                .ok_or(CommandDecodeError::InvalidField("choices"))?;
            if values.len() > 10_000 {
                return Err(CommandDecodeError::InvalidField("choices"));
            }
            let choices = values
                .iter()
                .map(|value| {
                    let object = value
                        .as_object()
                        .ok_or(CommandDecodeError::InvalidField("choices"))?;
                    let phrase_index = u16::try_from(unsigned(object, "phraseIndex")?)
                        .map_err(|_| CommandDecodeError::InvalidField("phraseIndex"))?;
                    let side = match string(object, "side")? {
                        "lumi" => ReconcileSide::Lumi,
                        "source" => ReconcileSide::Source,
                        _ => return Err(CommandDecodeError::InvalidField("side")),
                    };
                    Ok(PhraseConflictChoice { phrase_index, side })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ReconcileStrategy::Merge(choices))
        }
        _ => Err(CommandDecodeError::InvalidField("strategy")),
    }
}

fn autoloop_catalog_mutation(
    payload: &serde_json::Map<String, Value>,
) -> Result<AutoloopCatalogMutation, CommandDecodeError> {
    match string(payload, "operation")? {
        "renameTheme" => Ok(AutoloopCatalogMutation::RenameTheme {
            theme_id: ThemeId::new(positive_unsigned(payload, "themeId")?),
            display_name: string(payload, "displayName")?.to_owned(),
        }),
        "addVariant" => Ok(AutoloopCatalogMutation::AddVariant {
            role_id: phrase_role_id(payload)?,
            display_name: string(payload, "displayName")?.to_owned(),
        }),
        "renameVariant" => Ok(AutoloopCatalogMutation::RenameVariant {
            role_id: phrase_role_id(payload)?,
            variant_id: variant_id(payload)?,
            display_name: string(payload, "displayName")?.to_owned(),
        }),
        "moveVariantEarlier" => Ok(AutoloopCatalogMutation::MoveVariant {
            role_id: phrase_role_id(payload)?,
            variant_id: variant_id(payload)?,
            direction: AutoloopVariantMove::Earlier,
        }),
        "moveVariantLater" => Ok(AutoloopCatalogMutation::MoveVariant {
            role_id: phrase_role_id(payload)?,
            variant_id: variant_id(payload)?,
            direction: AutoloopVariantMove::Later,
        }),
        "archiveVariant" => Ok(AutoloopCatalogMutation::SetVariantArchived {
            role_id: phrase_role_id(payload)?,
            variant_id: variant_id(payload)?,
            archived: true,
        }),
        "restoreVariant" => Ok(AutoloopCatalogMutation::SetVariantArchived {
            role_id: phrase_role_id(payload)?,
            variant_id: variant_id(payload)?,
            archived: false,
        }),
        "setCell" => Ok(AutoloopCatalogMutation::SetCell {
            theme_id: ThemeId::new(positive_unsigned(payload, "themeId")?),
            role_id: phrase_role_id(payload)?,
            variant_id: variant_id(payload)?,
            display_name: optional_string(payload, "displayName").map(str::to_owned),
        }),
        "setButton" => Ok(AutoloopCatalogMutation::SetButton {
            theme_id: ThemeId::new(positive_unsigned(payload, "themeId")?),
            button_number: u16::try_from(positive_unsigned(payload, "buttonNumber")?)
                .map_err(|_| CommandDecodeError::InvalidField("buttonNumber"))?,
            role_id: phrase_role_id(payload)?,
            display_name: optional_string(payload, "displayName").map(str::to_owned),
        }),
        "clearButton" => Ok(AutoloopCatalogMutation::ClearButton {
            theme_id: ThemeId::new(positive_unsigned(payload, "themeId")?),
            button_number: u16::try_from(positive_unsigned(payload, "buttonNumber")?)
                .map_err(|_| CommandDecodeError::InvalidField("buttonNumber"))?,
        }),
        _ => Err(CommandDecodeError::InvalidField("operation")),
    }
}

fn phrase_role_mutation(
    payload: &serde_json::Map<String, Value>,
) -> Result<PhraseRoleCatalogMutation, CommandDecodeError> {
    match string(payload, "operation")? {
        "add" => Ok(PhraseRoleCatalogMutation::Add {
            display_name: string(payload, "displayName")?.to_owned(),
        }),
        "rename" => Ok(PhraseRoleCatalogMutation::Rename {
            role_id: phrase_role_id(payload)?,
            display_name: string(payload, "displayName")?.to_owned(),
        }),
        "moveEarlier" => Ok(PhraseRoleCatalogMutation::Move {
            role_id: phrase_role_id(payload)?,
            direction: PhraseRoleMove::Earlier,
        }),
        "moveLater" => Ok(PhraseRoleCatalogMutation::Move {
            role_id: phrase_role_id(payload)?,
            direction: PhraseRoleMove::Later,
        }),
        "archive" => Ok(PhraseRoleCatalogMutation::SetArchived {
            role_id: phrase_role_id(payload)?,
            archived: true,
        }),
        "restore" => Ok(PhraseRoleCatalogMutation::SetArchived {
            role_id: phrase_role_id(payload)?,
            archived: false,
        }),
        "setSourceMapping" => Ok(PhraseRoleCatalogMutation::SetSourceMapping {
            provider_kind: string(payload, "providerKind")?.to_owned(),
            raw_label: string(payload, "rawLabel")?.to_owned(),
            role_id: phrase_role_id(payload)?,
        }),
        _ => Err(CommandDecodeError::InvalidField("operation")),
    }
}

fn timeline_edit(
    payload: &serde_json::Map<String, Value>,
) -> Result<TimelineEditCommand, CommandDecodeError> {
    let operation = string(payload, "operation")?;
    match operation {
        "create" => Ok(TimelineEditCommand::Create {
            start_beat: timeline_beat(payload, "startBeat")?,
            end_beat: timeline_beat(payload, "endBeat")?,
            role_id: phrase_role_id(payload)?,
        }),
        "split" => Ok(TimelineEditCommand::Split {
            phrase_index: phrase_index_value(payload)?,
            at_beat: timeline_beat(payload, "atBeat")?,
        }),
        "mergePrevious" => Ok(TimelineEditCommand::MergePrevious {
            phrase_index: phrase_index_value(payload)?,
        }),
        "mergeNext" => Ok(TimelineEditCommand::MergeNext {
            phrase_index: phrase_index_value(payload)?,
        }),
        "moveBoundary" => Ok(TimelineEditCommand::MoveBoundary {
            boundary_after_phrase_index: phrase_index_value(payload)?,
            to_beat: timeline_beat(payload, "toBeat")?,
        }),
        "deleteAbsorbPrevious" => Ok(TimelineEditCommand::Delete {
            phrase_index: phrase_index_value(payload)?,
            absorb_into: PhraseAbsorption::Previous,
        }),
        "deleteAbsorbNext" => Ok(TimelineEditCommand::Delete {
            phrase_index: phrase_index_value(payload)?,
            absorb_into: PhraseAbsorption::Next,
        }),
        "changeRole" => Ok(TimelineEditCommand::ChangeRole {
            phrase_index: phrase_index_value(payload)?,
            role_id: phrase_role_id(payload)?,
        }),
        _ => Err(CommandDecodeError::InvalidField("operation")),
    }
}

fn phrase_loop_strategy(
    payload: &serde_json::Map<String, Value>,
) -> Result<PhraseLoopStrategy, CommandDecodeError> {
    match string(payload, "strategy")? {
        "auto" => Ok(PhraseLoopStrategy::Auto),
        "fixedVariant" => Ok(PhraseLoopStrategy::FixedVariant(variant_id(payload)?)),
        "themeSpecificExact" => {
            let values = payload
                .get("themeOverrides")
                .and_then(Value::as_array)
                .ok_or(CommandDecodeError::InvalidField("themeOverrides"))?;
            let mut overrides = Vec::with_capacity(values.len());
            for value in values {
                let object = value
                    .as_object()
                    .ok_or(CommandDecodeError::InvalidField("themeOverrides"))?;
                overrides.push(ThemeSpecificVariant::new(
                    ThemeId::new(positive_unsigned(object, "themeId")?),
                    variant_id(object)?,
                ));
            }
            Ok(PhraseLoopStrategy::ThemeSpecificExact(overrides))
        }
        _ => Err(CommandDecodeError::InvalidField("strategy")),
    }
}

fn phrase_role_id(
    payload: &serde_json::Map<String, Value>,
) -> Result<PhraseRoleId, CommandDecodeError> {
    PhraseRoleId::try_new(string(payload, "roleId")?)
        .map_err(|_| CommandDecodeError::InvalidField("roleId"))
}

fn variant_id(payload: &serde_json::Map<String, Value>) -> Result<VariantId, CommandDecodeError> {
    VariantId::try_new(string(payload, "variantId")?)
        .map_err(|_| CommandDecodeError::InvalidField("variantId"))
}

fn phrase_index_value(payload: &serde_json::Map<String, Value>) -> Result<u16, CommandDecodeError> {
    u16::try_from(unsigned(payload, "phraseIndex")?)
        .map_err(|_| CommandDecodeError::InvalidField("phraseIndex"))
}

fn timeline_beat(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u32, CommandDecodeError> {
    u32::try_from(unsigned(payload, field)?).map_err(|_| CommandDecodeError::InvalidField(field))
}

fn state_revision(envelope: &MessageEnvelope) -> Result<StateRevision, CommandDecodeError> {
    Ok(StateRevision::new(unsigned(
        &envelope.payload,
        "expectedStateRevision",
    )?))
}

fn operation_command(value: &str) -> Result<OperationCommand, CommandDecodeError> {
    match value {
        "armed" => Ok(OperationCommand::Arm),
        "live" => Ok(OperationCommand::Start),
        "paused" => Ok(OperationCommand::Pause),
        "off" => Ok(OperationCommand::Off),
        _ => Err(CommandDecodeError::InvalidField("operationState")),
    }
}

fn simulation_speed(value: u64) -> Result<SimulationSpeed, CommandDecodeError> {
    match value {
        1 => Ok(SimulationSpeed::One),
        4 => Ok(SimulationSpeed::Four),
        16 => Ok(SimulationSpeed::Sixteen),
        64 => Ok(SimulationSpeed::SixtyFour),
        _ => Err(CommandDecodeError::InvalidField("speed")),
    }
}

fn elapsed_ticks(envelope: &MessageEnvelope) -> Result<u64, CommandDecodeError> {
    let value = unsigned(&envelope.payload, "elapsedTicks")?;
    if value == 0 || value > 1_000 {
        return Err(CommandDecodeError::InvalidField("elapsedTicks"));
    }
    Ok(value)
}

fn context(envelope: &MessageEnvelope) -> Result<PlanCommandContext, CommandDecodeError> {
    let plan_id = string(&envelope.payload, "planId")?
        .parse::<u64>()
        .map_err(|_| CommandDecodeError::InvalidField("planId"))?;
    let track_load_id = unsigned(&envelope.payload, "trackLoadId")?;
    let expected_revision = unsigned(&envelope.payload, "expectedPlanRevision")?;
    if plan_id == 0 || track_load_id == 0 || expected_revision == 0 {
        return Err(CommandDecodeError::InvalidPlanContext);
    }
    Ok(PlanCommandContext {
        plan_id: PlanId::new(plan_id),
        track_load_id: TrackLoadId::new(track_load_id),
        expected_revision: PlanRevision::new(expected_revision),
    })
}

fn phrase_index(envelope: &MessageEnvelope) -> Result<u16, CommandDecodeError> {
    u16::try_from(unsigned(&envelope.payload, "phraseIndex")?)
        .map_err(|_| CommandDecodeError::InvalidField("phraseIndex"))
}

fn string<'a>(
    payload: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, CommandDecodeError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(CommandDecodeError::InvalidField(field))
}

fn unsigned(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u64, CommandDecodeError> {
    payload
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(CommandDecodeError::InvalidField(field))
}

fn signed(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<i64, CommandDecodeError> {
    payload
        .get(field)
        .and_then(Value::as_i64)
        .ok_or(CommandDecodeError::InvalidField(field))
}

fn positive_unsigned(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u64, CommandDecodeError> {
    let value = unsigned(payload, field)?;
    if value == 0 {
        return Err(CommandDecodeError::InvalidField(field));
    }
    Ok(value)
}

fn boolean(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<bool, CommandDecodeError> {
    payload
        .get(field)
        .and_then(Value::as_bool)
        .ok_or(CommandDecodeError::InvalidField(field))
}

fn optional_boolean(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Option<bool>, CommandDecodeError> {
    payload
        .get(field)
        .map(|value| {
            value
                .as_bool()
                .ok_or(CommandDecodeError::InvalidField(field))
        })
        .transpose()
}

fn string_array(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, CommandDecodeError> {
    let values = payload
        .get(field)
        .and_then(Value::as_array)
        .ok_or(CommandDecodeError::InvalidField(field))?;
    if values.is_empty() || values.len() > 20_000 {
        return Err(CommandDecodeError::InvalidField(field));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|text| !text.is_empty() && text.len() <= 2_048)
                .map(str::to_owned)
                .ok_or(CommandDecodeError::InvalidField(field))
        })
        .collect()
}

fn u32_array(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<u32>, CommandDecodeError> {
    let values = payload
        .get(field)
        .and_then(Value::as_array)
        .ok_or(CommandDecodeError::InvalidField(field))?;
    if values.is_empty() || values.len() > 20_000 {
        return Err(CommandDecodeError::InvalidField(field));
    }
    let converted = values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|number| u32::try_from(number).ok())
                .filter(|number| *number > 0)
                .ok_or(CommandDecodeError::InvalidField(field))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if converted
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != converted.len()
    {
        return Err(CommandDecodeError::InvalidField(field));
    }
    Ok(converted)
}

fn u64_array(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<u64>, CommandDecodeError> {
    let values = payload
        .get(field)
        .and_then(Value::as_array)
        .ok_or(CommandDecodeError::InvalidField(field))?;
    if values.len() > 20_000 {
        return Err(CommandDecodeError::InvalidField(field));
    }
    let converted = values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|number| *number > 0)
                .ok_or(CommandDecodeError::InvalidField(field))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if converted
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != converted.len()
    {
        return Err(CommandDecodeError::InvalidField(field));
    }
    Ok(converted)
}

fn optional_string<'a>(
    payload: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Option<&'a str> {
    payload.get(field).and_then(Value::as_str)
}

fn optional_unsigned(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Option<u64>, CommandDecodeError> {
    match payload.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or(CommandDecodeError::InvalidField(field)),
    }
}

fn library_limit(value: u64) -> Result<u16, CommandDecodeError> {
    let limit = u16::try_from(value).map_err(|_| CommandDecodeError::InvalidField("limit"))?;
    if limit == 0 || limit > 200 {
        return Err(CommandDecodeError::InvalidField("limit"));
    }
    Ok(limit)
}

fn library_search(payload: &serde_json::Map<String, Value>) -> Result<String, CommandDecodeError> {
    let search = optional_string(payload, "search").unwrap_or_default();
    if search.len() > 200 {
        return Err(CommandDecodeError::InvalidField("search"));
    }
    Ok(search.to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandDecodeError {
    WrongMessageType,
    UnsupportedKind,
    InvalidField(&'static str),
    InvalidPlanContext,
}

impl fmt::Display for CommandDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongMessageType => formatter.write_str("a command envelope is required"),
            Self::UnsupportedKind => formatter.write_str("the command kind is unsupported"),
            Self::InvalidField(field) => write!(formatter, "command field {field} is invalid"),
            Self::InvalidPlanContext => formatter.write_str("the plan context is invalid"),
        }
    }
}

impl Error for CommandDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_defaults_to_full_but_accepts_an_explicit_live_projection() {
        let full = command_envelope(serde_json::json!({
            "kind": "getSnapshot",
        }));
        assert_eq!(
            decode_command(&full),
            Ok(SessionCommand::GetSnapshot {
                include_library: true,
            })
        );

        let live = command_envelope(serde_json::json!({
            "kind": "getSnapshot",
            "includeLibrary": false,
        }));
        assert_eq!(
            decode_command(&live),
            Ok(SessionCommand::GetSnapshot {
                include_library: false,
            })
        );

        let invalid = command_envelope(serde_json::json!({
            "kind": "getSnapshot",
            "includeLibrary": "no",
        }));
        assert_eq!(
            decode_command(&invalid),
            Err(CommandDecodeError::InvalidField("includeLibrary"))
        );
    }

    #[test]
    fn ableton_link_enablement_decodes_as_an_explicit_boolean_command() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "setAbletonLinkEnabled",
            "enabled": true,
        }));
        assert_eq!(
            decode_command(&envelope),
            Ok(SessionCommand::SetAbletonLinkEnabled { enabled: true })
        );
    }

    #[test]
    fn ableton_link_helper_test_decodes_as_an_explicit_command() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "testAbletonLinkHelper",
        }));
        assert_eq!(
            decode_command(&envelope),
            Ok(SessionCommand::TestAbletonLinkHelper)
        );
    }

    #[test]
    fn output_timing_offset_accepts_only_the_safe_signed_range() {
        let valid = command_envelope(serde_json::json!({
            "kind": "setOutputTimingOffset",
            "millis": -125,
        }));
        assert_eq!(
            decode_command(&valid),
            Ok(SessionCommand::SetOutputTimingOffset { millis: -125 })
        );

        let invalid = command_envelope(serde_json::json!({
            "kind": "setOutputTimingOffset",
            "millis": 251,
        }));
        assert_eq!(
            decode_command(&invalid),
            Err(CommandDecodeError::InvalidField("millis"))
        );
    }

    #[test]
    fn custom_modifier_midi_address_accepts_note_zero_on_an_explicit_channel() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "sendMidiAddressLearnPulse",
            "targetKind": "custom",
            "targetNumber": 0,
            "channel": 14,
        }));
        let Some(expected) = MidiAddress::custom(14, 0) else {
            panic!("custom modifier address must be valid");
        };
        assert_eq!(
            decode_command(&envelope),
            Ok(SessionCommand::SendMidiAddressLearnPulse { address: expected })
        );
    }

    #[test]
    fn light_plan_preview_carries_the_complete_draft_policy() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "previewLightPlan",
            "trackId": 42,
            "expectedTimelineRevision": 7,
            "themeId": 2,
            "variationSeed": 9,
            "policy": {
                "revision": 3,
                "themeCooldownTracks": 1,
                "autoloopCooldownUses": 2,
                "duplicatePlanWindow": 4,
                "rules": [],
                "modifiers": [],
                "modifierRules": []
            }
        }));
        let policy = LightPlanningPolicy {
            revision: 3,
            ..LightPlanningPolicy::default()
        };
        assert_eq!(
            decode_command(&envelope),
            Ok(SessionCommand::PreviewLightPlan {
                track_id: 42,
                expected_timeline_revision: 7,
                theme_id: 2,
                variation_seed: 9,
                policy,
            })
        );
    }

    #[test]
    fn rekordbox_preview_decodes_exact_bounded_selection() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "previewRekordboxXmlSync",
            "folder": "/Music/Rekordbox XML",
            "followedPaths": ["Sets/Beach Set", "Genre 5 Stars"],
            "includeFutureChildPlaylists": true,
        }));

        assert_eq!(
            decode_command(&envelope),
            Ok(SessionCommand::PreviewRekordboxXmlSync {
                folder: "/Music/Rekordbox XML".to_owned(),
                followed_paths: vec!["Sets/Beach Set".to_owned(), "Genre 5 Stars".to_owned(),],
                include_future_child_playlists: true,
            })
        );
    }

    #[test]
    fn rekordbox_preview_rejects_an_empty_selection() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "previewRekordboxXmlSync",
            "folder": "/Music/Rekordbox XML",
            "followedPaths": [],
            "includeFutureChildPlaylists": true,
        }));

        assert_eq!(
            decode_command(&envelope),
            Err(CommandDecodeError::InvalidField("followedPaths"))
        );
    }

    #[test]
    fn rekordbox_analysis_import_is_bound_to_the_expected_export() {
        let envelope = command_envelope(serde_json::json!({
            "kind": "importRekordboxAnalysis",
            "folder": "/Music/Rekordbox XML",
            "followedPaths": ["Sets/Beach Set"],
            "includeFutureChildPlaylists": true,
            "expectedContentSha256": "abc123",
        }));

        assert_eq!(
            decode_command(&envelope),
            Ok(SessionCommand::ImportRekordboxAnalysis {
                folder: "/Music/Rekordbox XML".to_owned(),
                followed_paths: vec!["Sets/Beach Set".to_owned()],
                include_future_child_playlists: true,
                expected_content_sha256: "abc123".to_owned(),
            })
        );
    }

    #[test]
    fn library_waveform_detail_is_a_read_only_track_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = command_envelope(serde_json::json!({
            "kind": "getLibraryTrackWaveform",
            "trackId": 42,
        }));

        let command = decode_command(&envelope)?;

        assert_eq!(
            command,
            SessionCommand::GetLibraryTrackWaveform { track_id: 42 }
        );
        assert!(!command.is_mutating());
        Ok(())
    }

    fn command_envelope(payload: Value) -> MessageEnvelope {
        let payload = match payload {
            Value::Object(payload) => payload,
            _ => panic!("test payload must be an object"),
        };
        MessageEnvelope {
            protocol_version: 1,
            message_type: MessageType::Command,
            message_id: "command-test".to_owned(),
            sequence: 1,
            correlation_id: "test".to_owned(),
            sent_at: "2026-08-06T00:00:00Z".to_owned(),
            payload,
        }
    }
}
