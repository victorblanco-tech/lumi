use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write as _};
use std::net::Ipv4Addr;
use std::time::Duration;

use lumi_deck_source::DeckSourceProvider as _;
use lumi_domain::{
    ClientId, CommandSequence, CueOrigin, CueReason, DecisionReason, DeckObservation,
    DeckSourceStatus, DomainEvent, EffectId, EffectResult, EffectResultEnvelope, EffectSequence,
    KeyMode, LightingLook, LightingPlan, MonotonicTime, OperationCommand, OperationState,
    OutputEffectReason, OutputEffectResult, OutputEffectStatus, OutputExecutionRequest, PhraseKind,
    PitchClass, PlanRevision, PlanStatus, PlanValidationError, RuntimeHealth, SceneCategory,
    SceneId, SemanticLightingAction, SerializedRuntime, SerializedRuntimeError, TimelineResult,
    TimelineSource, TrackLoadId, TrackMetadata, UserCommandEnvelope, WorkerId,
};
use lumi_lighting_output::LightingOutputProvider as _;
use lumi_local_playback::{LocalPlaybackDeckSourceProvider, LocalPlaybackError};
use lumi_midi_coremidi::CoreMidiSourceProvider;
use lumi_midi_output::{MidiOutputController, MidiSourceState};
use lumi_output_dry_run::{DryRunLightingOutputProvider, DryRunOutputError};
use lumi_planner::{
    DeterministicPlanner, PlanMutationError, PlannerError, PlannerTrack, PlanningInput,
    PlanningOptions, StableChoiceSource, ThemeSelectionContext,
};
use lumi_protocol::{
    CommandDisposition, CommandIdCache, InvalidCacheCapacity, MAX_MESSAGE_BYTES, MessageDecoder,
    MessageEnvelope, MessageType, PROTOCOL_VERSION,
};
use lumi_simulator::{
    ManualClock, MonotonicClock as _, SimulationControl, SimulatorDeckSourceProvider,
    SimulatorError,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::StartupReady;
use crate::commands::{DeckSourceSelection, SessionCommand, decode_command};
use crate::library::{LibraryPlanContext, LibraryWorker, LibraryWorkerError, ResolvedLibraryCue};

const SESSION_TOKEN_ENVIRONMENT_KEY: &str = "LUMI_SESSION_TOKEN";
const MINIMUM_SESSION_TOKEN_BYTES: usize = 32;
const MAXIMUM_SESSION_TOKEN_BYTES: usize = 256;
const MAXIMUM_AUTHENTICATION_BYTES: usize = 512;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_QUEUE_CAPACITY: usize = 256;
const COMMAND_ID_CACHE_CAPACITY: usize = 256;
const LIBRARY_CONTEXT_CAPACITY: usize = 256;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionAuthentication {
    session_token: String,
}

/// Runs one app-scoped engine session until its authenticated client disconnects.
pub async fn run() -> Result<(), EngineError> {
    let session_token =
        env::var(SESSION_TOKEN_ENVIRONMENT_KEY).map_err(|_| EngineError::MissingSessionToken)?;
    validate_session_token(&session_token)?;
    let mut runtime = initialized_product_runtime()?;

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let endpoint = listener.local_addr()?;
    write_startup_record(endpoint.port())?;

    let (stream, peer) = timeout(CONNECTION_TIMEOUT, listener.accept())
        .await
        .map_err(|_| EngineError::ConnectionTimeout)??;

    if !peer.ip().is_loopback() {
        return Err(EngineError::NonLoopbackPeer);
    }

    serve_authenticated_client(stream, &session_token, &mut runtime).await
}

fn validate_session_token(session_token: &str) -> Result<(), EngineError> {
    if !(MINIMUM_SESSION_TOKEN_BYTES..=MAXIMUM_SESSION_TOKEN_BYTES).contains(&session_token.len()) {
        return Err(EngineError::InvalidSessionToken);
    }

    Ok(())
}

fn write_startup_record(port: u16) -> Result<(), EngineError> {
    let record = StartupReady {
        record_type: "engineReady".to_owned(),
        host: Ipv4Addr::LOCALHOST.to_string(),
        port,
        protocol_version: PROTOCOL_VERSION,
    };
    let encoded = serde_json::to_string(&record)?;

    println!("{encoded}");
    io::stdout().flush()?;
    Ok(())
}

async fn serve_authenticated_client(
    stream: TcpStream,
    expected_token: &str,
    runtime: &mut EngineRuntime,
) -> Result<(), EngineError> {
    let (mut reader, mut writer) = stream.into_split();
    let authentication_bytes = timeout(
        AUTHENTICATION_TIMEOUT,
        read_bounded_line(&mut reader, MAXIMUM_AUTHENTICATION_BYTES),
    )
    .await
    .map_err(|_| EngineError::AuthenticationTimeout)??;
    let authentication: SessionAuthentication = serde_json::from_slice(&authentication_bytes)
        .map_err(|_| EngineError::InvalidAuthentication)?;

    if !tokens_match(expected_token, &authentication.session_token) {
        return Err(EngineError::AuthenticationRejected);
    }

    let mut response_sequence = 1_u64;
    write_envelope(
        &mut writer,
        &snapshot_envelope(runtime, response_sequence, "session-bootstrap")?,
    )
    .await?;
    let mut command_ids = CommandIdCache::new(COMMAND_ID_CACHE_CAPACITY)?;

    while let Some(command_bytes) = read_command_line(&mut reader).await? {
        response_sequence = response_sequence
            .checked_add(1)
            .ok_or(EngineError::ResponseSequenceOverflow)?;
        let response = match MessageDecoder::decode(&command_bytes) {
            Ok(envelope) => {
                handle_command(runtime, &mut command_ids, &envelope, response_sequence)?
            }
            Err(error) => error_envelope(
                response_sequence,
                "unknown-command",
                "invalidCommand",
                "invalidEnvelope",
                &error.to_string(),
                false,
                None,
            )?,
        };
        write_envelope(&mut writer, &response).await?;
    }

    Ok(())
}

async fn write_envelope<W>(writer: &mut W, envelope: &MessageEnvelope) -> Result<(), EngineError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut encoded = serde_json::to_vec(envelope)?;
    encoded.push(b'\n');
    writer.write_all(&encoded).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_command_line<R>(reader: &mut R) -> Result<Option<Vec<u8>>, EngineError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(256);
    loop {
        match reader.read_u8().await {
            Ok(b'\n') => return Ok(Some(bytes)),
            Ok(byte) => {
                bytes.push(byte);
                if bytes.len() > MAX_MESSAGE_BYTES {
                    return Err(EngineError::MessageOversized);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof && bytes.is_empty() => {
                return Ok(None);
            }
            Err(error) => return Err(EngineError::Io(error)),
        }
    }
}

async fn read_bounded_line<R>(reader: &mut R, maximum: usize) -> Result<Vec<u8>, EngineError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(128);

    while bytes.len() <= maximum {
        let byte = reader.read_u8().await?;
        if byte == b'\n' {
            return Ok(bytes);
        }
        bytes.push(byte);
    }

    Err(EngineError::AuthenticationOversized)
}

fn tokens_match(expected: &str, received: &str) -> bool {
    expected.len() == received.len() && bool::from(expected.as_bytes().ct_eq(received.as_bytes()))
}

struct EngineRuntime {
    state: SerializedRuntime,
    clock: ManualClock,
    deck_source: SimulatorDeckSourceProvider<ManualClock>,
    local_deck_source: LocalPlaybackDeckSourceProvider,
    deck_source_mode: DeckSourceMode,
    planning_worker: PlanningWorker,
    output_worker: OutputWorker,
    library_worker: LibraryWorker,
    operation_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeckSourceMode {
    ConnectedDecks,
    LocalPlayback,
    Simulator,
}

impl EngineRuntime {
    fn deck_source_kind(&self) -> &'static str {
        match self.deck_source_mode {
            DeckSourceMode::ConnectedDecks => "beatLinkTrigger",
            DeckSourceMode::LocalPlayback => "localPlayback",
            DeckSourceMode::Simulator => "simulator",
        }
    }

    fn leader_deck_id(&self) -> Option<lumi_domain::DeckId> {
        match self.deck_source_mode {
            DeckSourceMode::ConnectedDecks => None,
            DeckSourceMode::LocalPlayback => self.local_deck_source.leader_deck_id(),
            DeckSourceMode::Simulator => Some(self.deck_source.leader_deck_id()),
        }
    }
}

fn initialized_product_runtime() -> Result<EngineRuntime, EngineError> {
    initialized_runtime_for_mode(ManualClock::new(0), DeckSourceMode::LocalPlayback)
}

fn initialized_runtime() -> Result<EngineRuntime, EngineError> {
    initialized_runtime_with_clock(ManualClock::new(0))
}

fn initialized_runtime_with_clock(clock: ManualClock) -> Result<EngineRuntime, EngineError> {
    initialized_runtime_for_mode(clock, DeckSourceMode::Simulator)
}

fn initialized_runtime_for_mode(
    clock: ManualClock,
    deck_source_mode: DeckSourceMode,
) -> Result<EngineRuntime, EngineError> {
    let mut runtime =
        SerializedRuntime::try_new(EVENT_QUEUE_CAPACITY).map_err(SerializedRuntimeError::from)?;
    submit_and_process(
        &mut runtime,
        DomainEvent::RuntimeStarted {
            at: MonotonicTime::new(0),
        },
    )?;
    let mut deck_source = SimulatorDeckSourceProvider::demo(clock.clone())?;
    let mut local_deck_source = LocalPlaybackDeckSourceProvider::new(clock.now())?;
    let mut planning_worker = PlanningWorker::new();
    let mut output_worker = OutputWorker::new();
    let library_worker = LibraryWorker::demo()?;
    match deck_source_mode {
        DeckSourceMode::ConnectedDecks => {}
        DeckSourceMode::LocalPlayback => {
            for event in local_deck_source.drain_events()? {
                planning_worker.process_source_event(
                    &mut runtime,
                    &mut output_worker,
                    event,
                    local_deck_source.leader_deck_id(),
                )?;
            }
        }
        DeckSourceMode::Simulator => {
            for event in deck_source.drain_events()? {
                planning_worker.process_source_event(
                    &mut runtime,
                    &mut output_worker,
                    event,
                    Some(deck_source.leader_deck_id()),
                )?;
            }
        }
    }
    Ok(EngineRuntime {
        state: runtime,
        clock,
        deck_source,
        local_deck_source,
        deck_source_mode,
        planning_worker,
        output_worker,
        library_worker,
        operation_sequence: 0,
    })
}

fn process_pending_source_events(runtime: &mut EngineRuntime) -> Result<(), EngineError> {
    let leader_deck_id = runtime.leader_deck_id();
    match runtime.deck_source_mode {
        DeckSourceMode::ConnectedDecks => {}
        DeckSourceMode::LocalPlayback => {
            for event in runtime.local_deck_source.drain_events()? {
                runtime.planning_worker.process_source_event(
                    &mut runtime.state,
                    &mut runtime.output_worker,
                    event,
                    leader_deck_id,
                )?;
            }
        }
        DeckSourceMode::Simulator => {
            for event in runtime.deck_source.drain_events()? {
                runtime.planning_worker.process_source_event(
                    &mut runtime.state,
                    &mut runtime.output_worker,
                    event,
                    leader_deck_id,
                )?;
            }
        }
    }
    Ok(())
}

struct PlanningWorker {
    planner: DeterministicPlanner<StableChoiceSource>,
    effect_sequence: u64,
    recent_theme_ids: Vec<lumi_domain::ThemeId>,
    library_contexts: BTreeMap<TrackLoadId, LibraryPlanContext>,
}

fn planner_track(metadata: &TrackMetadata) -> PlannerTrack {
    if metadata.phrases().is_empty() {
        PlannerTrack::without_analysis(metadata.id(), metadata.duration_beats())
    } else {
        PlannerTrack::analyzed(metadata)
    }
}

impl PlanningWorker {
    fn new() -> Self {
        Self {
            planner: DeterministicPlanner::epic_one(),
            effect_sequence: 0,
            recent_theme_ids: Vec::new(),
            library_contexts: BTreeMap::new(),
        }
    }

    fn register_library_context(
        &mut self,
        track_load_id: TrackLoadId,
        context: LibraryPlanContext,
    ) {
        self.library_contexts.insert(track_load_id, context);
        while self.library_contexts.len() > LIBRARY_CONTEXT_CAPACITY {
            let Some(oldest) = self.library_contexts.keys().next().copied() else {
                break;
            };
            self.library_contexts.remove(&oldest);
        }
    }

    fn library_context(&self, track_load_id: TrackLoadId) -> Option<&LibraryPlanContext> {
        self.library_contexts.get(&track_load_id)
    }

    fn materialize_library_plan(&self, plan: LightingPlan) -> Result<LightingPlan, EngineError> {
        let Some(context) = self.library_context(plan.track_load_id()) else {
            return Ok(plan);
        };
        let cues = plan
            .cues()
            .iter()
            .map(|cue| {
                let SemanticLightingAction::ApplyLook(look) = cue.action() else {
                    return Ok(cue.clone());
                };
                let resolution = context
                    .resolve(look.theme_id())
                    .map_err(LibraryWorkerError::from)?
                    .into_iter()
                    .find(|candidate| candidate.phrase_index == cue.phrase_index())
                    .ok_or(EngineError::MissingLibraryAutoloopResolution)?;
                let autoloop_number = resolution
                    .autoloop_number
                    .ok_or(EngineError::MissingLibraryAutoloopAddress)?;
                let materialized = LightingLook::try_new(
                    look.theme_id(),
                    look.theme_name().to_owned(),
                    SceneId::new(u64::from(autoloop_number)),
                    resolution.entry_name,
                    look.category(),
                    look.loop_selection(),
                )?;
                Ok(cue.revised(
                    SemanticLightingAction::ApplyLook(materialized),
                    cue.origin(),
                    cue.locked(),
                ))
            })
            .collect::<Result<Vec<_>, EngineError>>()?;
        Ok(plan.with_materialized_cues(cues)?)
    }

    fn process_source_event(
        &mut self,
        runtime: &mut SerializedRuntime,
        output_worker: &mut OutputWorker,
        event: DomainEvent,
        _leader_deck_id: Option<lumi_domain::DeckId>,
    ) -> Result<(), EngineError> {
        let planning_input = match &event {
            DomainEvent::Observation(observation) => match &observation.observation {
                DeckObservation::TrackLoaded {
                    deck_id,
                    metadata,
                    track_load_id,
                } => Some(PlanningInput {
                    deck_id: *deck_id,
                    track_load_id: *track_load_id,
                    track: planner_track(metadata),
                }),
                _ => None,
            },
            _ => None,
        };
        let observed_at = event.monotonic_time();
        process_domain_event(runtime, output_worker, event)?;
        if let Some(input) = planning_input {
            self.effect_sequence = self
                .effect_sequence
                .checked_add(1)
                .ok_or(EngineError::PlanningEffectSequenceOverflow)?;
            let context = ThemeSelectionContext::new(self.recent_theme_ids.clone());
            let generated = self.planner.generate_with_context(&input, &context)?;
            let plan = self.materialize_library_plan(generated)?;
            if let Some(decision) = plan.theme_decision() {
                self.recent_theme_ids.push(decision.theme_id());
                if self.recent_theme_ids.len() > 8 {
                    self.recent_theme_ids.remove(0);
                }
            }
            process_domain_event(
                runtime,
                output_worker,
                DomainEvent::EffectResult(EffectResultEnvelope {
                    effect_id: EffectId::new(self.effect_sequence),
                    worker_id: WorkerId::new(1),
                    sequence: EffectSequence::new(self.effect_sequence),
                    completed_at: observed_at,
                    result: EffectResult::PlanGenerated(plan),
                }),
            )?;
        }
        Ok(())
    }

    fn options(&self) -> PlanningOptions {
        self.planner.options()
    }

    fn accept_revised_plan(
        &mut self,
        runtime: &mut SerializedRuntime,
        plan: LightingPlan,
    ) -> Result<(), EngineError> {
        self.effect_sequence = self
            .effect_sequence
            .checked_add(1)
            .ok_or(EngineError::PlanningEffectSequenceOverflow)?;
        submit_and_process(
            runtime,
            DomainEvent::EffectResult(EffectResultEnvelope {
                effect_id: EffectId::new(self.effect_sequence),
                worker_id: WorkerId::new(1),
                sequence: EffectSequence::new(self.effect_sequence),
                completed_at: MonotonicTime::new(0),
                result: EffectResult::PlanGenerated(plan),
            }),
        )?;
        Ok(())
    }
}

struct OutputWorker {
    provider: DryRunLightingOutputProvider,
    midi_output: MidiOutputController<CoreMidiSourceProvider>,
    effect_sequence: u64,
}

impl OutputWorker {
    fn new() -> Self {
        Self {
            provider: DryRunLightingOutputProvider::default(),
            midi_output: MidiOutputController::new(CoreMidiSourceProvider::new()),
            effect_sequence: 0,
        }
    }

    fn process_effects(
        &mut self,
        runtime: &mut SerializedRuntime,
        effects: Vec<lumi_domain::Effect>,
    ) -> Result<(), EngineError> {
        for effect in effects {
            let (result, completed_at) = match effect {
                lumi_domain::Effect::EnsureOutputClosed { .. } => {
                    (EffectResult::OutputGateClosed, MonotonicTime::new(0))
                }
                lumi_domain::Effect::ExecuteCue(request) => {
                    let is_current = execution_context_is_current(runtime.state(), &request);
                    let result = if is_current {
                        let result = self.provider.execute(&request, request.scheduled_at())?;
                        if self.midi_output.status().state == MidiSourceState::Ready
                            && let Some((bank_number, autoloop_number)) =
                                automatic_midi_target(request.action())?
                        {
                            self.midi_output
                                .trigger_autoloop(bank_number, autoloop_number)
                                .map_err(|error| EngineError::Midi(error.to_string()))?;
                        }
                        result
                    } else {
                        OutputEffectResult::new(
                            request.clone(),
                            request.scheduled_at(),
                            OutputEffectStatus::Skipped,
                            OutputEffectReason::StaleExecutionContext,
                        )
                    };
                    let completed_at = result.actual_at();
                    (EffectResult::OutputEffectRecorded(result), completed_at)
                }
            };
            self.effect_sequence = self
                .effect_sequence
                .checked_add(1)
                .ok_or(EngineError::OutputEffectSequenceOverflow)?;
            submit_and_process(
                runtime,
                DomainEvent::EffectResult(EffectResultEnvelope {
                    effect_id: EffectId::new(self.effect_sequence),
                    worker_id: WorkerId::new(2),
                    sequence: EffectSequence::new(self.effect_sequence),
                    completed_at,
                    result,
                }),
            )?;
        }
        Ok(())
    }

    fn midi_status(&self) -> lumi_midi_output::MidiSourceStatus {
        self.midi_output.status()
    }
}

fn automatic_midi_target(action: &SemanticLightingAction) -> Result<Option<(u8, u8)>, EngineError> {
    let SemanticLightingAction::ApplyLook(look) = action else {
        return Ok(None);
    };
    let bank_number = u8::try_from(look.theme_id().value())
        .map_err(|_| EngineError::Midi("Theme bank does not fit the MIDI profile".to_owned()))?;
    let autoloop_number = u8::try_from(look.scene_id().value()).map_err(|_| {
        EngineError::Midi("Autoloop button does not fit the MIDI profile".to_owned())
    })?;
    Ok(Some((bank_number, autoloop_number)))
}

fn execution_context_is_current(
    state: &lumi_domain::RuntimeState,
    request: &OutputExecutionRequest,
) -> bool {
    state.operation() == OperationState::Live
        && state.leader_deck() == Some(request.deck_id())
        && state
            .deck(request.deck_id())
            .is_some_and(|deck| deck.track_load_id() == request.track_load_id())
        && state.active_plan().is_some_and(|plan| {
            plan.id() == request.plan_id()
                && plan.revision() == request.plan_revision()
                && plan.track_load_id() == request.track_load_id()
                && plan
                    .cues()
                    .get(usize::from(request.phrase_index()))
                    .is_some_and(|cue| {
                        cue.id() == request.cue_id() && cue.action() == request.action()
                    })
        })
}

fn process_domain_event(
    runtime: &mut SerializedRuntime,
    output_worker: &mut OutputWorker,
    event: DomainEvent,
) -> Result<(), EngineError> {
    let processed = submit_and_process(runtime, event)?;
    output_worker.process_effects(runtime, processed.effects)
}

fn submit_and_process(
    runtime: &mut SerializedRuntime,
    event: DomainEvent,
) -> Result<lumi_domain::ProcessResult, EngineError> {
    runtime
        .submit(event)
        .map_err(SerializedRuntimeError::from)?;
    let Some(processed) = runtime
        .process_next()
        .map_err(SerializedRuntimeError::from)?
    else {
        return Err(EngineError::SubmittedEventMissing);
    };
    Ok(processed)
}

fn handle_command(
    runtime: &mut EngineRuntime,
    command_ids: &mut CommandIdCache,
    envelope: &MessageEnvelope,
    response_sequence: u64,
) -> Result<MessageEnvelope, EngineError> {
    let command = match decode_command(envelope) {
        Ok(command) => command,
        Err(error) => {
            return error_envelope(
                response_sequence,
                &envelope.message_id,
                "invalidCommand",
                "invalidCommandPayload",
                &error.to_string(),
                false,
                None,
            );
        }
    };

    let is_mutating = command.is_mutating();
    if is_mutating && command_ids.contains(&envelope.message_id) {
        return snapshot_envelope(runtime, response_sequence, &envelope.message_id);
    }

    if let Err(error) = apply_command(runtime, command) {
        return application_error_envelope(response_sequence, &envelope.message_id, &error);
    }
    if is_mutating {
        debug_assert_eq!(
            command_ids.observe(&envelope.message_id),
            CommandDisposition::FirstSeen
        );
    }
    snapshot_envelope(runtime, response_sequence, &envelope.message_id)
}

fn apply_command(
    runtime: &mut EngineRuntime,
    command: SessionCommand,
) -> Result<(), CommandApplicationError> {
    match command {
        SessionCommand::GetSnapshot => return Ok(()),
        SessionCommand::QueryLibrary {
            search,
            playlist_id,
            offset,
            limit,
        } => {
            runtime
                .library_worker
                .query(search, playlist_id, offset, limit);
            return Ok(());
        }
        SessionCommand::OpenLibraryTrackEditor { track_id } => {
            runtime.library_worker.open_editor(track_id)?;
            return Ok(());
        }
        SessionCommand::CloseLibraryTrackEditor => {
            runtime.library_worker.close_editor();
            return Ok(());
        }
        SessionCommand::PreviewDemoSourceRefresh => {
            runtime.library_worker.preview_demo_source_refresh()?;
            return Ok(());
        }
        SessionCommand::ReconcileLibrarySource {
            track_id,
            expected_revision,
            strategy,
        } => {
            runtime.library_worker.reconcile_source_refresh(
                track_id,
                expected_revision,
                strategy,
            )?;
            return Ok(());
        }
        SessionCommand::EditLibraryTimeline {
            track_id,
            expected_revision,
            command,
        } => {
            runtime
                .library_worker
                .edit_timeline(track_id, expected_revision, command)?;
            return Ok(());
        }
        SessionCommand::SetLibraryPhraseLoopStrategy {
            track_id,
            expected_timeline_revision,
            expected_catalog_revision,
            phrase_index,
            strategy,
        } => {
            runtime.library_worker.set_phrase_loop_strategy(
                track_id,
                expected_timeline_revision,
                expected_catalog_revision,
                phrase_index,
                strategy,
            )?;
            return Ok(());
        }
        SessionCommand::UndoLibraryTimeline {
            track_id,
            expected_revision,
        } => {
            runtime
                .library_worker
                .undo_timeline(track_id, expected_revision)?;
            return Ok(());
        }
        SessionCommand::RedoLibraryTimeline {
            track_id,
            expected_revision,
        } => {
            runtime
                .library_worker
                .redo_timeline(track_id, expected_revision)?;
            return Ok(());
        }
        SessionCommand::RestoreLibraryTimelineRevision {
            track_id,
            expected_revision,
            target_revision,
        } => {
            runtime.library_worker.restore_timeline_revision(
                track_id,
                expected_revision,
                target_revision,
            )?;
            return Ok(());
        }
        SessionCommand::MutatePhraseRoleCatalog {
            expected_revision,
            mutation,
        } => {
            runtime
                .library_worker
                .mutate_phrase_role_catalog(expected_revision, mutation)?;
            return Ok(());
        }
        SessionCommand::MutateAutoloopCatalog {
            expected_revision,
            mutation,
        } => {
            runtime
                .library_worker
                .mutate_autoloop_catalog(expected_revision, mutation)?;
            return Ok(());
        }
        SessionCommand::PublishMidiSource => {
            runtime
                .output_worker
                .midi_output
                .publish()
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            return Ok(());
        }
        SessionCommand::StopMidiSource => {
            runtime.output_worker.midi_output.stop();
            return Ok(());
        }
        SessionCommand::SendMidiLearnPulse => {
            runtime
                .output_worker
                .midi_output
                .send_learn_pulse()
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            return Ok(());
        }
        SessionCommand::SendMidiAddressLearnPulse { address } => {
            runtime
                .output_worker
                .midi_output
                .send_address_learn_pulse(address)
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            return Ok(());
        }
        SessionCommand::TriggerMidiAutoloop {
            bank_number,
            autoloop_number,
        } => {
            runtime
                .output_worker
                .midi_output
                .trigger_autoloop(bank_number, autoloop_number)
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            return Ok(());
        }
        SessionCommand::LoadLibraryTrackOnLocalDeck {
            track_id,
            deck_id,
            expected_timeline_revision,
            expected_state_revision,
        } => {
            validate_state_revision(runtime, expected_state_revision)?;
            let prepared = runtime
                .library_worker
                .local_playback_track(track_id, expected_timeline_revision)?;
            let (metadata, context) = prepared.into_parts();
            let track_load_id = match runtime.deck_source_mode {
                DeckSourceMode::ConnectedDecks => {
                    return Err(CommandApplicationError::WrongDeckSourceMode);
                }
                DeckSourceMode::LocalPlayback => {
                    let at = runtime
                        .clock
                        .advance(1)
                        .ok_or(CommandApplicationError::ClockOverflow)?;
                    runtime
                        .local_deck_source
                        .load_track(deck_id, metadata, at)?
                }
                DeckSourceMode::Simulator => runtime.deck_source.load_track(deck_id, metadata)?,
            };
            runtime
                .planning_worker
                .register_library_context(track_load_id, context);
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::UpdateLocalPlaybackTransport {
            deck_id,
            track_load_id,
            position_millis,
            playing,
        } => {
            if runtime.deck_source_mode != DeckSourceMode::LocalPlayback {
                return Err(CommandApplicationError::WrongDeckSourceMode);
            }
            let beat = runtime
                .planning_worker
                .library_context(track_load_id)
                .ok_or(CommandApplicationError::TrackLoadMismatch)?
                .beat_at_millis(position_millis);
            let at = runtime
                .clock
                .advance(1)
                .ok_or(CommandApplicationError::ClockOverflow)?;
            runtime.local_deck_source.update_transport(
                deck_id,
                track_load_id,
                beat,
                playing,
                at,
            )?;
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::SetLocalPlaybackLeader {
            deck_id,
            expected_state_revision,
        } => {
            validate_state_revision(runtime, expected_state_revision)?;
            if runtime.deck_source_mode != DeckSourceMode::LocalPlayback {
                return Err(CommandApplicationError::WrongDeckSourceMode);
            }
            let at = runtime
                .clock
                .advance(1)
                .ok_or(CommandApplicationError::ClockOverflow)?;
            runtime.local_deck_source.set_leader(deck_id, at)?;
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::SelectDeckSourceMode {
            mode,
            expected_state_revision,
        } => {
            validate_state_revision(runtime, expected_state_revision)?;
            let target = match mode {
                DeckSourceSelection::ConnectedDecks => DeckSourceMode::ConnectedDecks,
                DeckSourceSelection::LocalPlayback => DeckSourceMode::LocalPlayback,
            };
            if runtime.deck_source_mode != target {
                if runtime.deck_source_mode == DeckSourceMode::LocalPlayback {
                    let at = runtime
                        .clock
                        .advance(1)
                        .ok_or(CommandApplicationError::ClockOverflow)?;
                    runtime.local_deck_source.clear(at)?;
                    process_pending_source_events(runtime)
                        .map_err(CommandApplicationError::Engine)?;
                }
                runtime.deck_source_mode = target;
            }
            return Ok(());
        }
        SessionCommand::LoadDemoSession { expected_revision }
        | SessionCommand::ResetDemoSession { expected_revision } => {
            validate_state_revision(runtime, expected_revision)?;
            *runtime = initialized_runtime().map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::SetOperationState {
            expected_revision,
            command,
        } => {
            apply_operation_command(runtime, expected_revision, command)?;
            return Ok(());
        }
        SessionCommand::SetSimulationSpeed {
            expected_revision,
            speed,
        } => {
            validate_state_revision(runtime, expected_revision)?;
            runtime
                .deck_source
                .apply_control(SimulationControl::SetSpeed(speed))?;
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::SetSimulationPlayback {
            expected_revision,
            playing,
        } => {
            validate_state_revision(runtime, expected_revision)?;
            let control = if playing {
                SimulationControl::Resume
            } else {
                SimulationControl::Pause
            };
            runtime.deck_source.apply_control(control)?;
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::AdvanceSimulation {
            expected_revision,
            elapsed_ticks,
        } => {
            validate_state_revision(runtime, expected_revision)?;
            runtime
                .clock
                .advance(elapsed_ticks)
                .ok_or(CommandApplicationError::ClockOverflow)?;
            runtime.deck_source.update_to_clock()?;
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::AdvanceToNextTrack { expected_revision } => {
            validate_state_revision(runtime, expected_revision)?;
            runtime
                .deck_source
                .apply_control(SimulationControl::AdvanceLeader)?;
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::SelectTheme { .. }
        | SessionCommand::SelectThemeFromPhrase { .. }
        | SessionCommand::SelectScene { .. }
        | SessionCommand::SetCueLock { .. }
        | SessionCommand::RegeneratePlan { .. } => {}
    }
    let context = command
        .context()
        .ok_or(CommandApplicationError::MissingPlanContext)?;
    let (current, input) = {
        let state = runtime.state.state();
        let Some((deck_id, deck)) = state
            .decks()
            .find(|(_, deck)| deck.track_load_id() == context.track_load_id)
        else {
            return Err(CommandApplicationError::TrackLoadMismatch);
        };
        let Some(current) = state.plan(deck_id) else {
            return Err(CommandApplicationError::PlanUnavailable);
        };
        if current.id() != context.plan_id || current.track_load_id() != context.track_load_id {
            return Err(CommandApplicationError::TrackLoadMismatch);
        }
        if current.revision() != context.expected_revision {
            return Err(CommandApplicationError::RevisionConflict {
                expected: context.expected_revision,
                actual: current.revision(),
            });
        }
        (
            current.clone(),
            PlanningInput {
                deck_id,
                track_load_id: deck.track_load_id(),
                track: planner_track(deck.metadata()),
            },
        )
    };

    let revised = match command {
        SessionCommand::GetSnapshot
        | SessionCommand::QueryLibrary { .. }
        | SessionCommand::OpenLibraryTrackEditor { .. }
        | SessionCommand::CloseLibraryTrackEditor
        | SessionCommand::PreviewDemoSourceRefresh
        | SessionCommand::ReconcileLibrarySource { .. }
        | SessionCommand::EditLibraryTimeline { .. }
        | SessionCommand::SetLibraryPhraseLoopStrategy { .. }
        | SessionCommand::UndoLibraryTimeline { .. }
        | SessionCommand::RedoLibraryTimeline { .. }
        | SessionCommand::RestoreLibraryTimelineRevision { .. }
        | SessionCommand::MutatePhraseRoleCatalog { .. }
        | SessionCommand::MutateAutoloopCatalog { .. }
        | SessionCommand::PublishMidiSource
        | SessionCommand::StopMidiSource
        | SessionCommand::SendMidiLearnPulse
        | SessionCommand::SendMidiAddressLearnPulse { .. }
        | SessionCommand::TriggerMidiAutoloop { .. }
        | SessionCommand::LoadLibraryTrackOnLocalDeck { .. }
        | SessionCommand::UpdateLocalPlaybackTransport { .. }
        | SessionCommand::SetLocalPlaybackLeader { .. }
        | SessionCommand::SelectDeckSourceMode { .. }
        | SessionCommand::LoadDemoSession { .. }
        | SessionCommand::SetOperationState { .. }
        | SessionCommand::SetSimulationSpeed { .. }
        | SessionCommand::SetSimulationPlayback { .. }
        | SessionCommand::AdvanceSimulation { .. }
        | SessionCommand::AdvanceToNextTrack { .. }
        | SessionCommand::ResetDemoSession { .. } => return Ok(()),
        SessionCommand::SelectTheme { theme_id, .. } => runtime
            .planning_worker
            .planner
            .select_theme(&current, theme_id)?,
        SessionCommand::SelectThemeFromPhrase {
            phrase_index,
            theme_id,
            ..
        } => {
            reject_started_live_phrase(runtime.state.state(), current.deck_id(), phrase_index)?;
            runtime.planning_worker.planner.select_theme_from_phrase(
                &current,
                phrase_index,
                theme_id,
            )?
        }
        SessionCommand::SelectScene {
            phrase_index,
            scene_id,
            ..
        } => {
            reject_started_live_phrase(runtime.state.state(), current.deck_id(), phrase_index)?;
            if runtime
                .planning_worker
                .library_context(current.track_load_id())
                .is_some()
            {
                let theme_id = current
                    .cues()
                    .get(usize::from(phrase_index))
                    .and_then(|cue| action_theme_id(cue.action()))
                    .ok_or(CommandApplicationError::PlanUnavailable)?;
                let autoloop_number = u16::try_from(scene_id.value())
                    .map_err(|_| CommandApplicationError::InvalidAutoloopSelection)?;
                runtime
                    .planning_worker
                    .library_contexts
                    .get_mut(&current.track_load_id())
                    .ok_or(CommandApplicationError::PlanUnavailable)?
                    .set_autoloop_override(theme_id, phrase_index, autoloop_number)
                    .map_err(LibraryWorkerError::from)?;
                let materialized = runtime
                    .planning_worker
                    .materialize_library_plan(current.clone())
                    .map_err(CommandApplicationError::Engine)?;
                if materialized.cues() == current.cues() {
                    return Err(PlanMutationError::NoChange.into());
                }
                current
                    .revised(materialized.cues().to_vec())
                    .map_err(PlanMutationError::InvalidPlan)?
            } else {
                runtime
                    .planning_worker
                    .planner
                    .select_scene(&current, phrase_index, scene_id)?
            }
        }
        SessionCommand::SetCueLock {
            phrase_index,
            locked,
            ..
        } => {
            reject_started_live_phrase(runtime.state.state(), current.deck_id(), phrase_index)?;
            runtime
                .planning_worker
                .planner
                .set_cue_lock(&current, phrase_index, locked)?
        }
        SessionCommand::RegeneratePlan { .. } => runtime
            .planning_worker
            .planner
            .regenerate(&current, &input)?,
    };
    let revised = runtime
        .planning_worker
        .materialize_library_plan(revised)
        .map_err(CommandApplicationError::Engine)?;
    runtime
        .planning_worker
        .accept_revised_plan(&mut runtime.state, revised)
        .map_err(CommandApplicationError::Engine)
}

fn validate_state_revision(
    runtime: &EngineRuntime,
    expected: lumi_domain::StateRevision,
) -> Result<(), CommandApplicationError> {
    let actual = runtime.state.state().revision();
    if actual != expected {
        return Err(CommandApplicationError::StateRevisionConflict { expected, actual });
    }
    Ok(())
}

fn reject_started_live_phrase(
    state: &lumi_domain::RuntimeState,
    deck_id: lumi_domain::DeckId,
    phrase_index: u16,
) -> Result<(), CommandApplicationError> {
    if state.leader_deck() == Some(deck_id)
        && state
            .deck(deck_id)
            .and_then(lumi_domain::DeckState::phrase_index)
            .is_some_and(|current| phrase_index <= current)
    {
        return Err(CommandApplicationError::StartedLivePhraseNotEditable);
    }
    Ok(())
}

fn apply_operation_command(
    runtime: &mut EngineRuntime,
    expected_revision: lumi_domain::StateRevision,
    command: OperationCommand,
) -> Result<(), CommandApplicationError> {
    validate_state_revision(runtime, expected_revision)?;
    let from = runtime.state.state().operation();
    let valid = matches!(
        (from, command),
        (OperationState::Off, OperationCommand::Arm)
            | (
                OperationState::Armed | OperationState::Paused,
                OperationCommand::Start
            )
            | (OperationState::Live, OperationCommand::Pause)
            | (_, OperationCommand::Off)
    );
    if !valid {
        return Err(CommandApplicationError::InvalidOperationTransition { from, command });
    }
    runtime.operation_sequence = runtime
        .operation_sequence
        .checked_add(1)
        .ok_or(CommandApplicationError::OperationSequenceOverflow)?;
    process_domain_event(
        &mut runtime.state,
        &mut runtime.output_worker,
        DomainEvent::UserCommand(UserCommandEnvelope {
            client_id: ClientId::new(1),
            sequence: CommandSequence::new(runtime.operation_sequence),
            expected_state_revision: expected_revision,
            issued_at: runtime.clock.now(),
            command,
        }),
    )
    .map_err(CommandApplicationError::Engine)
}

fn application_error_envelope(
    sequence: u64,
    correlation_id: &str,
    error: &CommandApplicationError,
) -> Result<MessageEnvelope, EngineError> {
    match error {
        CommandApplicationError::StateRevisionConflict { actual, .. } => error_envelope(
            sequence,
            correlation_id,
            "revisionConflict",
            "stateRevisionMismatch",
            "The session changed before the command was applied.",
            true,
            None,
        )
        .map(|mut envelope| {
            envelope
                .payload
                .insert("actualStateRevision".to_owned(), json!(actual.value()));
            envelope
        }),
        CommandApplicationError::InvalidOperationTransition { .. } => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "invalidOperationTransition",
            &error.to_string(),
            false,
            None,
        ),
        CommandApplicationError::RevisionConflict { actual, .. } => error_envelope(
            sequence,
            correlation_id,
            "revisionConflict",
            "planRevisionMismatch",
            "The plan changed before the command was applied.",
            true,
            Some(*actual),
        ),
        CommandApplicationError::TrackLoadMismatch => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "trackLoadMismatch",
            "The next track changed before this edit was applied.",
            false,
            None,
        ),
        CommandApplicationError::PlanUnavailable => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "planUnavailable",
            "There is no editable next-track plan.",
            true,
            None,
        ),
        CommandApplicationError::StartedLivePhraseNotEditable => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "startedLivePhraseNotEditable",
            "The selected Live phrase has already started and is locked.",
            false,
            None,
        ),
        CommandApplicationError::InvalidAutoloopSelection => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "invalidAutoloopSelection",
            "The selected Autoloop is not mapped for this Theme and Phrase Type.",
            false,
            None,
        ),
        CommandApplicationError::Mutation(mutation) => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "planMutationRejected",
            &mutation.to_string(),
            false,
            None,
        ),
        CommandApplicationError::Library(LibraryWorkerError::TimelineRevisionConflict {
            actual,
            ..
        }) => error_envelope(
            sequence,
            correlation_id,
            "revisionConflict",
            "timelineRevisionMismatch",
            "The phrase timeline changed before the edit was applied.",
            true,
            None,
        )
        .map(|mut envelope| {
            envelope
                .payload
                .insert("actualTimelineRevision".to_owned(), json!(actual.value()));
            envelope
        }),
        CommandApplicationError::Library(
            LibraryWorkerError::PhraseRoleCatalogRevisionConflict { actual, .. },
        ) => error_envelope(
            sequence,
            correlation_id,
            "revisionConflict",
            "phraseRoleRevisionMismatch",
            "Phrase-role settings changed before the edit was applied.",
            true,
            None,
        )
        .map(|mut envelope| {
            envelope
                .payload
                .insert("actualPhraseRoleRevision".to_owned(), json!(actual));
            envelope
        }),
        CommandApplicationError::Library(LibraryWorkerError::AutoloopCatalogRevisionConflict {
            actual,
            ..
        }) => error_envelope(
            sequence,
            correlation_id,
            "revisionConflict",
            "autoloopCatalogRevisionMismatch",
            "The Autoloop catalog changed before the edit was applied.",
            true,
            None,
        )
        .map(|mut envelope| {
            envelope
                .payload
                .insert("actualAutoloopCatalogRevision".to_owned(), json!(actual));
            envelope
        }),
        CommandApplicationError::Library(
            library_error @ (LibraryWorkerError::TimelineEdit(_)
            | LibraryWorkerError::NothingToUndo
            | LibraryWorkerError::NothingToRedo
            | LibraryWorkerError::UnknownTimelineRevision(_)
            | LibraryWorkerError::EditorTrackMismatch
            | LibraryWorkerError::PhraseRoleCatalog(_)
            | LibraryWorkerError::AutoloopCatalog(_)
            | LibraryWorkerError::UnknownPhraseRole
            | LibraryWorkerError::ArchivedPhraseRole),
        ) => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            if matches!(
                &library_error,
                LibraryWorkerError::PhraseRoleCatalog(_)
                    | LibraryWorkerError::AutoloopCatalog(_)
                    | LibraryWorkerError::UnknownPhraseRole
                    | LibraryWorkerError::ArchivedPhraseRole
            ) {
                if matches!(&library_error, LibraryWorkerError::AutoloopCatalog(_)) {
                    "autoloopCatalogChangeRejected"
                } else {
                    "phraseRoleChangeRejected"
                }
            } else {
                "timelineEditRejected"
            },
            &library_error.to_string(),
            false,
            None,
        ),
        CommandApplicationError::MissingPlanContext
        | CommandApplicationError::OperationSequenceOverflow
        | CommandApplicationError::ClockOverflow
        | CommandApplicationError::Engine(_)
        | CommandApplicationError::Midi(_)
        | CommandApplicationError::Library(_)
        | CommandApplicationError::LocalPlayback(_)
        | CommandApplicationError::WrongDeckSourceMode
        | CommandApplicationError::Simulator(_) => error_envelope(
            sequence,
            correlation_id,
            "commandFailed",
            "internalCommandFailure",
            "The command could not be applied safely.",
            true,
            None,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn error_envelope(
    sequence: u64,
    correlation_id: &str,
    kind: &str,
    code: &str,
    message: &str,
    retryable: bool,
    actual_revision: Option<PlanRevision>,
) -> Result<MessageEnvelope, EngineError> {
    let mut payload = Map::new();
    payload.insert("kind".to_owned(), Value::String(kind.to_owned()));
    payload.insert("code".to_owned(), Value::String(code.to_owned()));
    payload.insert("message".to_owned(), Value::String(message.to_owned()));
    payload.insert("retryable".to_owned(), Value::Bool(retryable));
    if let Some(actual) = actual_revision {
        payload.insert("actualPlanRevision".to_owned(), json!(actual.value()));
    }
    Ok(MessageEnvelope {
        protocol_version: PROTOCOL_VERSION,
        message_type: MessageType::Error,
        message_id: format!("error-{sequence}"),
        sequence,
        correlation_id: correlation_id.to_owned(),
        sent_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        payload,
    })
}

#[derive(Debug, Error)]
enum CommandApplicationError {
    #[error("plan context is missing")]
    MissingPlanContext,
    #[error("plan revision conflict: expected {expected:?}, actual {actual:?}")]
    RevisionConflict {
        expected: PlanRevision,
        actual: PlanRevision,
    },
    #[error("state revision conflict: expected {expected:?}, actual {actual:?}")]
    StateRevisionConflict {
        expected: lumi_domain::StateRevision,
        actual: lumi_domain::StateRevision,
    },
    #[error("operation command {command:?} is invalid from {from:?}")]
    InvalidOperationTransition {
        from: OperationState,
        command: OperationCommand,
    },
    #[error("the operation command sequence overflowed")]
    OperationSequenceOverflow,
    #[error("the simulation clock overflowed")]
    ClockOverflow,
    #[error("the track-load instance no longer matches")]
    TrackLoadMismatch,
    #[error("the plan is unavailable")]
    PlanUnavailable,
    #[error("the selected Live phrase has already started")]
    StartedLivePhraseNotEditable,
    #[error("the selected Autoloop button is invalid")]
    InvalidAutoloopSelection,
    #[error("plan mutation failed: {0}")]
    Mutation(#[from] PlanMutationError),
    #[error("simulator control failed: {0}")]
    Simulator(#[from] SimulatorError),
    #[error("local playback failed: {0}")]
    LocalPlayback(#[from] LocalPlaybackError),
    #[error("the command is not valid for the active deck source")]
    WrongDeckSourceMode,
    #[error("library command failed: {0}")]
    Library(#[from] LibraryWorkerError),
    #[error("MIDI output command failed: {0}")]
    Midi(String),
    #[error("engine failed while accepting a plan revision: {0}")]
    Engine(EngineError),
}

fn snapshot_envelope(
    runtime: &EngineRuntime,
    sequence: u64,
    correlation_id: &str,
) -> Result<MessageEnvelope, EngineError> {
    let state = runtime.state.state();
    let mut payload = Map::new();
    payload.insert("kind".to_owned(), Value::String("stateSnapshot".to_owned()));
    payload.insert("stateRevision".to_owned(), json!(state.revision().value()));
    payload.insert(
        "operationState".to_owned(),
        Value::String(operation_state_name(state.operation()).to_owned()),
    );
    payload.insert(
        "engineVersion".to_owned(),
        Value::String(env!("CARGO_PKG_VERSION").to_owned()),
    );
    payload.insert(
        "runtimeCore".to_owned(),
        json!({
            "model": "singleWriterReducer",
            "health": runtime_health_name(state.health()),
            "queueCapacity": runtime.state.queue_capacity(),
            "queueDepth": runtime.state.queue_depth(),
            "processedEvents": state.processed_events(),
            "lastDecision": state.last_decision().map(decision_reason_name),
        }),
    );
    payload.insert(
        "deckSource".to_owned(),
        json!({
            "providerKind": runtime.deck_source_kind(),
            "mode": match runtime.deck_source_mode {
                DeckSourceMode::ConnectedDecks => "connectedDecks",
                DeckSourceMode::LocalPlayback => "localPlayback",
                DeckSourceMode::Simulator => "internalTest",
            },
            "displayName": match runtime.deck_source_mode {
                DeckSourceMode::ConnectedDecks => "Connected Decks",
                DeckSourceMode::LocalPlayback => "Local Playback",
                DeckSourceMode::Simulator => "Internal Test Source",
            },
            "status": if runtime.deck_source_mode == DeckSourceMode::ConnectedDecks {
                "disconnected"
            } else {
                state
                    .source_statuses()
                    .next()
                    .map(|(_, status)| deck_source_status_name(status))
                    .unwrap_or("starting")
            },
        }),
    );
    if runtime.deck_source_mode == DeckSourceMode::Simulator {
        payload.insert(
            "simulation".to_owned(),
            json!({
                "speed": runtime.deck_source.speed().multiplier(),
                "paused": runtime.deck_source.is_paused(),
            }),
        );
    }
    let midi_output = runtime.output_worker.midi_status();
    payload.insert(
        "midiIntegration".to_owned(),
        json!({
            "state": match midi_output.state {
                MidiSourceState::Stopped => "stopped",
                MidiSourceState::Ready => "ready",
            },
            "sourceName": midi_output.source_name,
            "protocol": "MIDI 1.0 UMP",
            "sentPulseCount": midi_output.sent_pulse_count,
            "lastEvent": midi_output.last_event,
        }),
    );
    payload.insert(
        "leaderDeckId".to_owned(),
        json!(state.leader_deck().map(|deck_id| deck_id.value())),
    );
    let deck_source_kind = runtime.deck_source_kind();
    let decks = state
        .decks()
        .map(|(deck_id, deck)| {
            let metadata = deck.metadata();
            let library_context = runtime
                .planning_worker
                .library_context(deck.track_load_id());
            let waveform_preview = library_context.map_or_else(
                || {
                    if deck_source_kind == "simulator" {
                        simulator_waveform_preview(metadata.id().value())
                    } else {
                        Value::Null
                    }
                },
                LibraryPlanContext::waveform_preview_json,
            );
            let local_playback = if runtime.deck_source_mode == DeckSourceMode::LocalPlayback {
                library_context.map_or(Value::Null, LibraryPlanContext::local_playback_json)
            } else {
                Value::Null
            };
            let plan_eligibility = if library_context.is_some() {
                "readyExact"
            } else if state
                .plan(deck_id)
                .is_some_and(|plan| plan.status() == PlanStatus::Ready)
            {
                "readyTransient"
            } else {
                "autoHeld"
            };
            json!({
                "deckId": deck_id.value(),
                "trackLoadId": deck.track_load_id().value(),
                "beat": deck.beat(),
                "playing": deck.is_playing(),
                "phraseIndex": deck.phrase_index(),
                "planEligibility": plan_eligibility,
                "localPlayback": local_playback,
                "track": {
                    "id": metadata.id().value(),
                    "title": metadata.title(),
                    "artist": metadata.artist(),
                    "bpmMilli": metadata.bpm_milli(),
                    "colorRgb": metadata.color().map(lumi_domain::TrackColor::rgb_u32),
                    "key": {
                        "pitchClass": pitch_class_name(metadata.musical_key().pitch_class()),
                        "mode": key_mode_name(metadata.musical_key().mode()),
                    },
                    "durationBeats": metadata.duration_beats(),
                    "waveformPreview": waveform_preview,
                    "identityFacts": metadata.identity_facts().map(|identity| json!({
                        "matchStatus": "exact",
                        "providerKind": identity.provider_kind(),
                        "sourceId": identity.source_id(),
                        "sourceTrackId": identity.source_track_id(),
                        "analysisRevision": identity.analysis_revision(),
                        "timelineRevision": identity.lumi_timeline_revision(),
                    })),
                    "phrases": metadata.phrases().iter().map(|phrase| {
                        let role = library_context.map_or(Value::Null, |context| {
                            context.phrase_role_json(phrase.index())
                        });
                        json!({
                            "index": phrase.index(),
                            "startBeat": phrase.start_beat(),
                            "endBeat": phrase.end_beat(),
                            "kind": phrase_kind_name(phrase.kind()),
                            "role": role,
                        })
                    }).collect::<Vec<_>>(),
                },
            })
        })
        .collect::<Vec<_>>();
    payload.insert("decks".to_owned(), Value::Array(decks));
    payload.insert(
        "planningOptions".to_owned(),
        planning_options_json(&runtime.planning_worker.options()),
    );
    payload.insert(
        "outputProvider".to_owned(),
        json!({
            "providerKind": runtime.output_worker.provider.provider_kind(),
            "status": "ready",
            "recordCount": runtime.output_worker.provider.records().count(),
        }),
    );
    payload.insert(
        "activePlan".to_owned(),
        state.active_plan().map_or(Value::Null, |plan| {
            json!({
                "planId": plan.id().value().to_string(),
                "planRevision": plan.revision().value(),
                "deckId": plan.deck_id().value(),
                "trackLoadId": plan.track_load_id().value(),
            })
        }),
    );
    payload.insert(
        "outputEffects".to_owned(),
        Value::Array(
            state
                .output_effects()
                .map(|effect| output_effect_json(effect, &runtime.planning_worker))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    payload.insert(
        "timeline".to_owned(),
        Value::Array(
            state
                .timeline()
                .map(|entry| {
                    json!({
                        "sequence": entry.sequence(),
                        "occurredAt": entry.occurred_at().ticks(),
                        "source": timeline_source_name(entry.source()),
                        "type": entry.event_type(),
                        "result": timeline_result_name(entry.result()),
                        "reason": decision_reason_name(entry.reason()),
                    })
                })
                .collect(),
        ),
    );
    let next_plan = state
        .decks()
        .find(|(deck_id, _)| Some(*deck_id) != state.leader_deck())
        .and_then(|(deck_id, _)| state.plan(deck_id))
        .map(|plan| plan_json(plan, &runtime.planning_worker))
        .transpose()?
        .unwrap_or(Value::Null);
    payload.insert("nextPlan".to_owned(), next_plan);
    let live_plan = state
        .active_plan()
        .map(|plan| plan_json(plan, &runtime.planning_worker))
        .transpose()?
        .unwrap_or(Value::Null);
    payload.insert("livePlan".to_owned(), live_plan);
    payload.insert(
        "library".to_owned(),
        runtime.library_worker.snapshot_json()?,
    );

    Ok(MessageEnvelope {
        protocol_version: PROTOCOL_VERSION,
        message_type: MessageType::Snapshot,
        message_id: format!("snapshot-{sequence}"),
        sequence,
        correlation_id: correlation_id.to_owned(),
        sent_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        payload,
    })
}

fn simulator_waveform_preview(track_id: u64) -> Value {
    let points = (0_u64..192)
        .map(|index| {
            let mixed = track_id
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(index.wrapping_mul(1_442_695_040_888_963_407));
            let low = 4 + mixed % 28;
            let mid = 3 + mixed.rotate_left(17) % 29;
            let high = 2 + mixed.rotate_left(37) % 30;
            json!({ "low": low, "mid": mid, "high": high })
        })
        .collect::<Vec<_>>();
    json!({
        "source": "simulator",
        "style": "rgb",
        "points": points,
    })
}

fn output_effect_json(
    result: &OutputEffectResult,
    planning_worker: &PlanningWorker,
) -> Result<Value, LibraryWorkerError> {
    let request = result.request();
    let library_resolution = action_theme_id(request.action())
        .and_then(|theme_id| {
            planning_worker
                .library_context(request.track_load_id())
                .map(|context| {
                    context.resolve(theme_id).map(|cues| {
                        cues.into_iter()
                            .find(|cue| cue.phrase_index == request.phrase_index())
                            .map(|cue| library_resolution_json(&cue))
                            .unwrap_or(Value::Null)
                    })
                })
        })
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(json!({
        "commandId": request.command_id().value(),
        "planId": request.plan_id().value().to_string(),
        "planRevision": request.plan_revision().value(),
        "deckId": request.deck_id().value(),
        "trackLoadId": request.track_load_id().value(),
        "phraseIndex": request.phrase_index(),
        "cueId": request.cue_id().value().to_string(),
        "scheduledAt": request.scheduled_at().ticks(),
        "actualAt": result.actual_at().ticks(),
        "status": output_effect_status_name(result.status()),
        "resultReason": output_effect_reason_name(result.reason()),
        "cueReason": cue_reason_json(request.cue_reason()),
        "action": action_json(request.action()),
        "libraryResolution": library_resolution,
    }))
}

fn plan_json(
    plan: &LightingPlan,
    planning_worker: &PlanningWorker,
) -> Result<Value, LibraryWorkerError> {
    let library_context = planning_worker.library_context(plan.track_load_id());
    let library_track = library_context.map_or(Value::Null, LibraryPlanContext::identity_json);
    let mut library_cues = BTreeMap::new();
    let mut library_choices = BTreeMap::new();
    if let Some(context) = library_context {
        for cue in plan.cues() {
            let Some(theme_id) = action_theme_id(cue.action()) else {
                continue;
            };
            if let Some(resolved) = context
                .resolve(theme_id)?
                .into_iter()
                .find(|resolved| resolved.phrase_index == cue.phrase_index())
            {
                library_cues.insert(cue.phrase_index(), resolved);
            }
            library_choices.insert(
                cue.phrase_index(),
                context.autoloop_choices(theme_id, cue.phrase_index()),
            );
        }
    }
    Ok(json!({
        "planId": plan.id().value().to_string(),
        "deckId": plan.deck_id().value(),
        "trackId": plan.track_id().value(),
        "trackDurationBeats": plan.track_duration_beats(),
        "trackLoadId": plan.track_load_id().value(),
        "revision": plan.revision().value(),
        "configurationRevision": plan.configuration_revision().value(),
        "seed": plan.seed().to_string(),
        "status": plan_status_name(plan.status()),
        "themeDecision": plan.theme_decision().map(|decision| json!({
            "themeId": decision.theme_id().value(),
            "themeName": decision.theme_name(),
            "reason": theme_selection_reason_name(decision.reason()),
            "matchedColorRgb": decision.matched_color(),
        })),
        "libraryTrack": library_track,
        "cues": plan.cues().iter().map(|cue| json!({
            "phraseIndex": cue.phrase_index(),
            "startBeat": cue.start_beat(),
            "endBeat": cue.end_beat(),
            "origin": cue_origin_name(cue.origin()),
            "locked": cue.locked(),
            "reason": cue_reason_json(cue.reason()),
            "action": action_json(cue.action()),
            "libraryResolution": library_cues
                .get(&cue.phrase_index())
                .map_or(Value::Null, |resolution| {
                    library_resolution_with_choices_json(
                        resolution,
                        library_choices
                            .get(&cue.phrase_index())
                            .map_or(&[], Vec::as_slice),
                    )
                }),
        })).collect::<Vec<_>>(),
    }))
}

fn library_resolution_json(cue: &ResolvedLibraryCue) -> Value {
    json!({
        "roleId": cue.role_id,
        "roleName": cue.role_name,
        "strategy": cue.strategy,
        "variantId": cue.variant_id,
        "catalogRevision": cue.catalog_revision,
        "resolutionReason": cue.resolution_reason,
        "dryRunEntry": {
            "id": cue.entry_id,
            "name": cue.entry_name,
        },
        "bankNumber": cue.bank_number,
        "autoloopNumber": cue.autoloop_number,
    })
}

fn library_resolution_with_choices_json(
    cue: &ResolvedLibraryCue,
    choices: &[ResolvedLibraryCue],
) -> Value {
    let mut value = library_resolution_json(cue);
    if let Value::Object(ref mut resolution) = value {
        resolution.insert(
            "choices".to_owned(),
            Value::Array(
                choices
                    .iter()
                    .map(|choice| {
                        json!({
                            "id": choice.autoloop_number,
                            "name": choice.entry_name,
                            "variantId": choice.variant_id,
                            "bankNumber": choice.bank_number,
                        })
                    })
                    .collect(),
            ),
        );
    }
    value
}

fn action_theme_id(action: &SemanticLightingAction) -> Option<lumi_domain::ThemeId> {
    match action {
        SemanticLightingAction::ApplyLook(look) => Some(look.theme_id()),
        SemanticLightingAction::HoldCurrentLook => None,
    }
}

const fn theme_selection_reason_name(reason: lumi_domain::ThemeSelectionReason) -> &'static str {
    match reason {
        lumi_domain::ThemeSelectionReason::GlobalLock => "globalLock",
        lumi_domain::ThemeSelectionReason::PlanInstanceUserChoice => "planInstanceUserChoice",
        lumi_domain::ThemeSelectionReason::ColorForce => "colorForce",
        lumi_domain::ThemeSelectionReason::ColorPrefer => "colorPrefer",
        lumi_domain::ThemeSelectionReason::Rotation => "rotation",
        lumi_domain::ThemeSelectionReason::DefaultTheme => "defaultTheme",
    }
}

fn planning_options_json(options: &PlanningOptions) -> Value {
    json!({
        "themes": options.themes.iter().map(|theme| json!({
            "id": theme.id.value(),
            "name": theme.name,
        })).collect::<Vec<_>>(),
        "scenes": options.scenes.iter().map(|scene| json!({
            "id": scene.id.value(),
            "name": scene.name,
            "category": scene_category_name(scene.category),
            "loopBank": scene.loop_selection.bank(),
            "loopSlot": scene.loop_selection.slot(),
        })).collect::<Vec<_>>(),
    })
}

fn cue_reason_json(reason: CueReason) -> Value {
    match reason {
        CueReason::PhraseCategoryMatched {
            phrase_kind,
            category,
        } => json!({
            "kind": "phraseCategoryMatched",
            "phraseKind": phrase_kind_name(phrase_kind),
            "category": scene_category_name(category),
        }),
        CueReason::MissingPhraseAnalysis => json!({
            "kind": "missingPhraseAnalysis",
        }),
    }
}

fn action_json(action: &SemanticLightingAction) -> Value {
    match action {
        SemanticLightingAction::ApplyLook(look) => json!({
            "kind": "applyLook",
            "themeId": look.theme_id().value(),
            "themeName": look.theme_name(),
            "sceneId": look.scene_id().value(),
            "sceneName": look.scene_name(),
            "category": scene_category_name(look.category()),
            "loopBank": look.loop_selection().bank(),
            "loopSlot": look.loop_selection().slot(),
        }),
        SemanticLightingAction::HoldCurrentLook => json!({
            "kind": "holdCurrentLook",
        }),
    }
}

const fn plan_status_name(status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::Ready => "ready",
        PlanStatus::Fallback => "fallback",
    }
}

const fn output_effect_status_name(status: OutputEffectStatus) -> &'static str {
    match status {
        OutputEffectStatus::Simulated => "simulated",
        OutputEffectStatus::Rejected => "rejected",
        OutputEffectStatus::Skipped => "skipped",
    }
}

const fn output_effect_reason_name(reason: OutputEffectReason) -> &'static str {
    match reason {
        OutputEffectReason::PhraseBoundary => "phraseBoundary",
        OutputEffectReason::ProviderRejected => "providerRejected",
        OutputEffectReason::StaleExecutionContext => "staleExecutionContext",
    }
}

const fn timeline_source_name(source: TimelineSource) -> &'static str {
    match source {
        TimelineSource::Runtime => "runtime",
        TimelineSource::DeckSource => "deckSource",
        TimelineSource::Operation => "operation",
        TimelineSource::Planner => "planner",
        TimelineSource::Output => "output",
    }
}

const fn timeline_result_name(result: TimelineResult) -> &'static str {
    match result {
        TimelineResult::Accepted => "accepted",
        TimelineResult::Ignored => "ignored",
        TimelineResult::Scheduled => "scheduled",
        TimelineResult::Simulated => "simulated",
        TimelineResult::Rejected => "rejected",
        TimelineResult::Skipped => "skipped",
        TimelineResult::Completed => "completed",
    }
}

const fn cue_origin_name(origin: CueOrigin) -> &'static str {
    match origin {
        CueOrigin::Automatic => "automatic",
        CueOrigin::Fallback => "fallback",
        CueOrigin::User => "user",
    }
}

const fn scene_category_name(category: SceneCategory) -> &'static str {
    match category {
        SceneCategory::Ambient => "ambient",
        SceneCategory::Groove => "groove",
        SceneCategory::Build => "build",
        SceneCategory::Impact => "impact",
        SceneCategory::Break => "break",
    }
}

const fn deck_source_status_name(status: DeckSourceStatus) -> &'static str {
    match status {
        DeckSourceStatus::Starting => "starting",
        DeckSourceStatus::Ready => "ready",
        DeckSourceStatus::Degraded => "degraded",
        DeckSourceStatus::Disconnected => "disconnected",
    }
}

const fn pitch_class_name(pitch_class: PitchClass) -> &'static str {
    match pitch_class {
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

const fn key_mode_name(mode: KeyMode) -> &'static str {
    match mode {
        KeyMode::Major => "major",
        KeyMode::Minor => "minor",
    }
}

const fn phrase_kind_name(kind: PhraseKind) -> &'static str {
    match kind {
        PhraseKind::Intro => "intro",
        PhraseKind::Verse => "verse",
        PhraseKind::Build => "build",
        PhraseKind::Drop => "drop",
        PhraseKind::Breakdown => "breakdown",
        PhraseKind::Outro => "outro",
    }
}

const fn operation_state_name(state: OperationState) -> &'static str {
    match state {
        OperationState::Off => "off",
        OperationState::Armed => "armed",
        OperationState::Live => "live",
        OperationState::Paused => "paused",
    }
}

const fn runtime_health_name(health: RuntimeHealth) -> &'static str {
    match health {
        RuntimeHealth::Starting => "starting",
        RuntimeHealth::Ready => "ready",
        RuntimeHealth::Degraded => "degraded",
    }
}

const fn decision_reason_name(reason: DecisionReason) -> &'static str {
    match reason {
        DecisionReason::RuntimeInitialized => "runtimeInitialized",
        DecisionReason::SourceStatusAccepted => "sourceStatusAccepted",
        DecisionReason::TrackLoadAccepted => "trackLoadAccepted",
        DecisionReason::PositionAdvanced => "positionAdvanced",
        DecisionReason::PlaybackStateChanged => "playbackStateChanged",
        DecisionReason::PhraseChanged => "phraseChanged",
        DecisionReason::LeaderChanged => "leaderChanged",
        DecisionReason::PlanActivated => "planActivated",
        DecisionReason::PlanActivationSkipped => "planActivationSkipped",
        DecisionReason::PhraseExecutionScheduled => "phraseExecutionScheduled",
        DecisionReason::PhraseExecutionSkipped => "phraseExecutionSkipped",
        DecisionReason::TrackUnloaded => "trackUnloaded",
        DecisionReason::StaleObservationIgnored => "staleObservationIgnored",
        DecisionReason::ObservationTimeRegressed => "observationTimeRegressed",
        DecisionReason::TrackLoadMismatch => "trackLoadMismatch",
        DecisionReason::PositionRegressed => "positionRegressed",
        DecisionReason::DuplicateCommandIgnored => "duplicateCommandIgnored",
        DecisionReason::OperationTransitionAccepted => "operationTransitionAccepted",
        DecisionReason::DuplicateEffectIgnored => "duplicateEffectIgnored",
        DecisionReason::PlanAccepted => "planAccepted",
        DecisionReason::StalePlanIgnored => "stalePlanIgnored",
        DecisionReason::PlanTrackLoadMismatch => "planTrackLoadMismatch",
        DecisionReason::OutputGateConfirmedClosed => "outputGateConfirmedClosed",
        DecisionReason::OutputEffectRecorded => "outputEffectRecorded",
        DecisionReason::QueueSaturated => "queueSaturated",
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("the app-scoped session token is missing")]
    MissingSessionToken,
    #[error("the app-scoped session token has an invalid length")]
    InvalidSessionToken,
    #[error("timed out waiting for the local app connection")]
    ConnectionTimeout,
    #[error("timed out waiting for session authentication")]
    AuthenticationTimeout,
    #[error("session authentication exceeds the maximum size")]
    AuthenticationOversized,
    #[error("a protocol message exceeds the maximum size")]
    MessageOversized,
    #[error("session authentication is malformed")]
    InvalidAuthentication,
    #[error("session authentication was rejected")]
    AuthenticationRejected,
    #[error("a non-loopback peer was rejected")]
    NonLoopbackPeer,
    #[error("I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("JSON encoding failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("timestamp formatting failed: {0}")]
    TimeFormat(#[from] time::error::Format),
    #[error("domain runtime initialization failed: {0}")]
    DomainRuntime(#[from] SerializedRuntimeError),
    #[error("simulator initialization failed: {0}")]
    Simulator(#[from] SimulatorError),
    #[error("local playback initialization failed: {0}")]
    LocalPlayback(#[from] LocalPlaybackError),
    #[error("planner failed: {0}")]
    Planner(#[from] PlannerError),
    #[error("a Library plan could not be materialized: {0}")]
    PlanMaterialization(#[from] PlanValidationError),
    #[error("a Library phrase has no resolved Autoloop")]
    MissingLibraryAutoloopResolution,
    #[error("a resolved Library Autoloop has no MIDI button address")]
    MissingLibraryAutoloopAddress,
    #[error("the serialized runtime lost an event after accepting it")]
    SubmittedEventMissing,
    #[error("the planning worker effect sequence overflowed")]
    PlanningEffectSequenceOverflow,
    #[error("the output worker effect sequence overflowed")]
    OutputEffectSequenceOverflow,
    #[error("dry-run output failed: {0}")]
    DryRunOutput(#[from] DryRunOutputError),
    #[error("MIDI output failed: {0}")]
    Midi(String),
    #[error("music library failed: {0}")]
    Library(#[from] LibraryWorkerError),
    #[error("the response sequence overflowed")]
    ResponseSequenceOverflow,
    #[error("the command ID cache could not initialize: {0}")]
    CommandCache(#[from] InvalidCacheCapacity),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::PlanCommandContext;
    use lumi_domain::{ClientId, CommandSequence, OperationCommand, ThemeId, UserCommandEnvelope};
    use lumi_output_dry_run::canonical_output_transcript;
    use lumi_simulator::{SimulationControl, SimulationSpeed};

    #[test]
    fn next_plan_is_ready_before_the_initial_leader_event() {
        let mut runtime = match SerializedRuntime::try_new(EVENT_QUEUE_CAPACITY) {
            Ok(runtime) => runtime,
            Err(error) => panic!("test runtime must initialize: {error}"),
        };
        if let Err(error) = submit_and_process(
            &mut runtime,
            DomainEvent::RuntimeStarted {
                at: MonotonicTime::new(0),
            },
        ) {
            panic!("test runtime must start: {error}");
        }
        let mut source = match SimulatorDeckSourceProvider::demo(ManualClock::new(0)) {
            Ok(source) => source,
            Err(error) => panic!("test simulator must initialize: {error}"),
        };
        let events = match source.drain_events() {
            Ok(events) => events,
            Err(error) => panic!("test simulator events must drain: {error}"),
        };
        let mut worker = PlanningWorker::new();
        let mut output_worker = OutputWorker::new();

        for event in events {
            if matches!(
                &event,
                DomainEvent::Observation(lumi_domain::ObservationEnvelope {
                    observation: DeckObservation::LeaderChanged { .. },
                    ..
                })
            ) {
                assert!(runtime.state().plan(lumi_domain::DeckId::new(2)).is_some());
            }
            if let Err(error) = worker.process_source_event(
                &mut runtime,
                &mut output_worker,
                event,
                Some(source.leader_deck_id()),
            ) {
                panic!("test source event must process: {error}");
            }
        }
    }

    #[test]
    fn simulator_snapshot_exposes_normalized_rgb_waveforms_for_both_decks() {
        let runtime = initialized_runtime()
            .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
        let snapshot = snapshot_envelope(&runtime, 1, "waveform-contract")
            .unwrap_or_else(|error| panic!("snapshot must encode: {error}"));
        let decks = snapshot.payload["decks"]
            .as_array()
            .unwrap_or_else(|| panic!("snapshot must contain decks"));

        assert_eq!(decks.len(), 2);
        for deck in decks {
            let waveform = &deck["track"]["waveformPreview"];
            assert_eq!(waveform["source"], "simulator");
            assert_eq!(waveform["style"], "rgb");
            assert_eq!(waveform["points"].as_array().map(Vec::len), Some(192));
        }
    }

    #[test]
    fn live_execution_matches_the_canonical_dry_run_transcript() {
        let clock = ManualClock::new(0);
        let mut runtime = match initialized_runtime_with_clock(clock.clone()) {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_operation(&mut runtime, 1, OperationCommand::Arm);
        apply_operation(&mut runtime, 2, OperationCommand::Start);
        apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
        apply_simulation_control(
            &mut runtime,
            SimulationControl::SetSpeed(SimulationSpeed::SixtyFour),
        );
        assert!(clock.advance(1_000).is_some());
        if let Err(error) = runtime.deck_source.update_to_clock() {
            panic!("test simulator must advance: {error}");
        }
        if let Err(error) = process_pending_source_events(&mut runtime) {
            panic!("test source events must process: {error}");
        }

        let results = runtime
            .state
            .state()
            .output_effects()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 4);
        assert!(
            results
                .iter()
                .all(|result| result.status() == OutputEffectStatus::Simulated)
        );
        let actual = match canonical_output_transcript(&results) {
            Ok(actual) => actual,
            Err(error) => panic!("test transcript must encode: {error}"),
        };
        assert_eq!(
            actual,
            include_bytes!("../../../../fixtures/demo-session-v1/output-effects.json")
        );
    }

    #[test]
    fn library_track_runs_from_exact_timeline_through_next_activation_and_dry_run() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        runtime
            .library_worker
            .open_editor(1)
            .unwrap_or_else(|error| panic!("fixture track must open: {error}"));
        runtime
            .library_worker
            .set_phrase_loop_strategy(
                1,
                1,
                1,
                1,
                lumi_library::PhraseLoopStrategy::FixedVariant(
                    lumi_library::VariantId::try_new("mapping-2")
                        .unwrap_or_else(|error| panic!("fixture variant must be valid: {error}")),
                ),
            )
            .unwrap_or_else(|error| panic!("fixture loop strategy must save: {error}"));
        let expected_state_revision = runtime.state.state().revision();
        apply_session_command(
            &mut runtime,
            SessionCommand::LoadLibraryTrackOnLocalDeck {
                track_id: 1,
                deck_id: lumi_domain::DeckId::new(2),
                expected_timeline_revision: 2,
                expected_state_revision,
            },
        );

        let preview = snapshot_envelope(&runtime, 1, "library-preview")
            .unwrap_or_else(|error| panic!("preview must encode: {error}"));
        assert_eq!(preview.payload["decks"][1]["track"]["id"], 1);
        assert_eq!(
            preview.payload["decks"][1]["track"]["identityFacts"]["timelineRevision"],
            2
        );
        assert_eq!(
            preview.payload["nextPlan"]["libraryTrack"]["matchStatus"],
            "exact"
        );
        assert_eq!(
            preview.payload["nextPlan"]["libraryTrack"]["providerKind"],
            "demo"
        );
        assert!(preview.payload["nextPlan"]["themeDecision"]["reason"].is_string());
        let preview_cues = preview.payload["nextPlan"]["cues"]
            .as_array()
            .unwrap_or_else(|| panic!("next plan must expose cues"));
        assert!(!preview_cues.is_empty());
        assert!(preview_cues.iter().all(|cue| {
            cue["libraryResolution"]["roleId"].is_string()
                && cue["libraryResolution"]["strategy"].is_string()
                && cue["libraryResolution"]["variantId"].is_string()
                && cue["libraryResolution"]["dryRunEntry"]["id"].is_string()
                && cue["libraryResolution"]["choices"]
                    .as_array()
                    .is_some_and(|choices| !choices.is_empty())
                && cue["action"]["sceneId"] == cue["libraryResolution"]["autoloopNumber"]
        }));

        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Arm,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Start,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceToNextTrack { expected_revision }
        });
        let activated_snapshot = snapshot_envelope(&runtime, 2, "library-activated")
            .unwrap_or_else(|error| panic!("activated snapshot must encode: {error}"));
        let active = runtime
            .state
            .state()
            .active_plan()
            .unwrap_or_else(|| panic!("leader change must activate the prepared plan"));
        assert_eq!(active.track_id(), lumi_domain::TrackId::new(1));
        let cue_count = active.cues().len();
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetSimulationSpeed {
                expected_revision,
                speed: SimulationSpeed::SixtyFour,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceSimulation {
                expected_revision,
                elapsed_ticks: 1_000,
            }
        });

        let results = runtime.state.state().output_effects().collect::<Vec<_>>();
        assert_eq!(results.len(), cue_count);
        assert_eq!(
            results
                .iter()
                .map(|result| result.request().phrase_index())
                .collect::<Vec<_>>(),
            (0..u16::try_from(cue_count)
                .unwrap_or_else(|error| panic!("fixture cue count must fit u16: {error}")))
                .collect::<Vec<_>>()
        );
        let completed = snapshot_envelope(&runtime, 3, "library-completed")
            .unwrap_or_else(|error| panic!("completed snapshot must encode: {error}"));
        assert!(
            completed.payload["outputEffects"]
                .as_array()
                .is_some_and(|effects| effects.iter().all(|effect| {
                    effect["status"] == "simulated"
                        && effect["libraryResolution"]["dryRunEntry"]["id"].is_string()
                }))
        );
        let evidence = canonical_library_simulation_evidence(
            &preview.payload,
            &activated_snapshot.payload,
            &completed.payload,
        );
        assert_eq!(
            String::from_utf8_lossy(&evidence),
            String::from_utf8_lossy(include_bytes!(
                "../../../../fixtures/demo-library-v1/simulator-e2e.json"
            ))
        );
    }

    #[test]
    fn armed_and_paused_states_never_call_the_output_provider() {
        let clock = ManualClock::new(0);
        let mut runtime = match initialized_runtime_with_clock(clock.clone()) {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_operation(&mut runtime, 1, OperationCommand::Arm);
        apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
        assert_eq!(runtime.output_worker.provider.records().count(), 0);

        apply_operation(&mut runtime, 2, OperationCommand::Start);
        apply_operation(&mut runtime, 3, OperationCommand::Pause);
        apply_simulation_control(
            &mut runtime,
            SimulationControl::SetSpeed(SimulationSpeed::SixtyFour),
        );
        assert!(clock.advance(1_000).is_some());
        if let Err(error) = runtime.deck_source.update_to_clock() {
            panic!("test simulator must advance: {error}");
        }
        if let Err(error) = process_pending_source_events(&mut runtime) {
            panic!("test source events must process: {error}");
        }
        assert_eq!(runtime.output_worker.provider.records().count(), 0);
        assert_eq!(runtime.state.state().operation(), OperationState::Paused);
    }

    #[test]
    fn accepted_live_plan_revisions_update_future_output_atomically() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
        let active_before = match runtime.state.state().active_plan().cloned() {
            Some(plan) => plan,
            None => panic!("leader change must freeze an active plan"),
        };
        let stored_before = match runtime
            .state
            .state()
            .plan(lumi_domain::DeckId::new(2))
            .cloned()
        {
            Some(plan) => plan,
            None => panic!("stored plan must remain available"),
        };
        let changed = match runtime
            .planning_worker
            .planner
            .select_theme(&stored_before, lumi_domain::ThemeId::new(4))
        {
            Ok(plan) => plan,
            Err(error) => panic!("preview Theme must change: {error}"),
        };
        if let Err(error) = runtime
            .planning_worker
            .accept_revised_plan(&mut runtime.state, changed)
        {
            panic!("revised preview plan must be accepted: {error}");
        }

        let active_after = match runtime.state.state().active_plan() {
            Some(plan) => plan,
            None => panic!("active plan must remain frozen"),
        };
        let stored_after = match runtime.state.state().plan(lumi_domain::DeckId::new(2)) {
            Some(plan) => plan,
            None => panic!("stored plan must remain available"),
        };
        assert_ne!(active_after, &active_before);
        assert_eq!(active_after.revision(), PlanRevision::new(2));
        assert_eq!(stored_after.revision(), PlanRevision::new(2));
        assert_eq!(
            active_after.theme_decision().map(|value| value.theme_id()),
            stored_after.theme_decision().map(|value| value.theme_id())
        );
    }

    #[test]
    fn started_live_phrases_are_locked_but_future_phrases_remain_editable() {
        let mut runtime = initialized_runtime()
            .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
        let active = runtime
            .state
            .state()
            .active_plan()
            .cloned()
            .unwrap_or_else(|| panic!("initial leader must have an active plan"));
        let current_theme = active
            .theme_decision()
            .map(|decision| decision.theme_id())
            .unwrap_or_else(|| panic!("ready plan must have a Theme"));
        let new_theme = [
            ThemeId::new(1),
            ThemeId::new(2),
            ThemeId::new(3),
            ThemeId::new(4),
        ]
        .into_iter()
        .find(|candidate| *candidate != current_theme)
        .unwrap_or_else(|| panic!("fixture must expose an alternate Theme"));
        let context = PlanCommandContext {
            plan_id: active.id(),
            track_load_id: active.track_load_id(),
            expected_revision: active.revision(),
        };

        let started = apply_command(
            &mut runtime,
            SessionCommand::SelectThemeFromPhrase {
                context,
                phrase_index: 0,
                theme_id: new_theme,
            },
        );
        assert!(matches!(
            started,
            Err(CommandApplicationError::StartedLivePhraseNotEditable)
        ));

        apply_session_command(
            &mut runtime,
            SessionCommand::SelectThemeFromPhrase {
                context,
                phrase_index: 1,
                theme_id: new_theme,
            },
        );
        let revised = runtime
            .state
            .state()
            .active_plan()
            .unwrap_or_else(|| panic!("accepted edit must remain active"));
        assert_eq!(revised.revision(), PlanRevision::new(2));
        assert_eq!(revised.cues()[0], active.cues()[0]);
        assert_ne!(revised.cues()[1], active.cues()[1]);
    }

    #[test]
    fn live_apply_look_maps_to_a_sound_switch_bank_and_autoloop_button() {
        let runtime = initialized_runtime()
            .unwrap_or_else(|error| panic!("test engine must initialize: {error}"));
        let cue = runtime
            .state
            .state()
            .active_plan()
            .and_then(|plan| plan.cues().first())
            .unwrap_or_else(|| panic!("initial Live plan must expose a cue"));
        let SemanticLightingAction::ApplyLook(look) = cue.action() else {
            panic!("ready Live cue must apply a look");
        };

        let target = automatic_midi_target(cue.action())
            .unwrap_or_else(|error| panic!("fixture target must fit MIDI: {error}"));
        assert_eq!(
            target,
            Some((
                u8::try_from(look.theme_id().value())
                    .unwrap_or_else(|error| panic!("fixture Theme must fit: {error}")),
                u8::try_from(look.scene_id().value())
                    .unwrap_or_else(|error| panic!("fixture button must fit: {error}")),
            ))
        );
    }

    #[test]
    fn stale_execution_is_skipped_before_the_provider_call() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_operation(&mut runtime, 1, OperationCommand::Arm);
        apply_operation(&mut runtime, 2, OperationCommand::Start);
        apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
        let Some(request) = runtime
            .state
            .state()
            .output_effects()
            .next()
            .map(|result| result.request().clone())
        else {
            panic!("live leader change must create the first output request");
        };
        assert_eq!(runtime.output_worker.provider.records().count(), 1);

        apply_operation(&mut runtime, 3, OperationCommand::Pause);
        if let Err(error) = runtime.output_worker.process_effects(
            &mut runtime.state,
            vec![lumi_domain::Effect::ExecuteCue(request)],
        ) {
            panic!("stale output must be recorded safely: {error}");
        }

        assert_eq!(runtime.output_worker.provider.records().count(), 1);
        let Some(last) = runtime.state.state().output_effects().last() else {
            panic!("skipped output must be retained");
        };
        assert_eq!(last.status(), OutputEffectStatus::Skipped);
        assert_eq!(last.reason(), OutputEffectReason::StaleExecutionContext);
    }

    #[test]
    fn app_commands_complete_the_canonical_demo_and_reset_without_restart() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetSimulationSpeed {
                expected_revision,
                speed: SimulationSpeed::SixtyFour,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Arm,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Start,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceToNextTrack { expected_revision }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceSimulation {
                expected_revision,
                elapsed_ticks: 1_000,
            }
        });

        assert_eq!(runtime.state.state().operation(), OperationState::Live);
        assert_eq!(runtime.output_worker.provider.records().count(), 4);
        assert!(
            runtime
                .state
                .state()
                .timeline()
                .any(|entry| entry.source() == TimelineSource::Operation)
        );
        assert!(
            runtime
                .state
                .state()
                .timeline()
                .any(|entry| entry.result() == TimelineResult::Simulated)
        );

        let reset_revision = runtime.state.state().revision();
        apply_session_command(
            &mut runtime,
            SessionCommand::ResetDemoSession {
                expected_revision: reset_revision,
            },
        );
        let canonical = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("canonical engine must initialize: {error}"),
        };
        assert_eq!(runtime.state.state(), canonical.state.state());
        assert_eq!(
            runtime.deck_source.canonical_snapshot(),
            canonical.deck_source.canonical_snapshot()
        );
        assert_eq!(runtime.output_worker.provider.records().count(), 0);
    }

    #[test]
    fn simulation_speed_preserves_semantic_output_order() {
        let sixteen = semantic_output_order(SimulationSpeed::Sixteen, 4_000);
        let sixty_four = semantic_output_order(SimulationSpeed::SixtyFour, 1_000);
        assert_eq!(sixteen, sixty_four);
        assert_eq!(sixteen.len(), 4);
    }

    #[test]
    fn runtime_timeline_is_bounded_to_the_latest_256_entries() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        for sequence in 1..=300 {
            if let Err(error) = submit_and_process(
                &mut runtime.state,
                DomainEvent::EffectResult(EffectResultEnvelope {
                    effect_id: EffectId::new(sequence),
                    worker_id: WorkerId::new(99),
                    sequence: EffectSequence::new(sequence),
                    completed_at: MonotonicTime::new(sequence),
                    result: EffectResult::OutputGateClosed,
                }),
            ) {
                panic!("timeline event must process: {error}");
            }
        }
        let timeline = runtime.state.state().timeline().collect::<Vec<_>>();
        assert_eq!(timeline.len(), 256);
        assert_eq!(
            timeline.last().map(|entry| entry.occurred_at().ticks()),
            Some(300)
        );
        assert!(
            timeline
                .windows(2)
                .all(|entries| entries[0].sequence() < entries[1].sequence())
        );
    }

    #[test]
    fn canonical_app_to_output_scenario_matches_release_evidence() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        let Some(initial_plan) = runtime
            .state
            .state()
            .plan(lumi_domain::DeckId::new(2))
            .cloned()
        else {
            panic!("demo must generate the next plan");
        };
        let revised = match runtime.planning_worker.planner.select_scene(
            &initial_plan,
            1,
            lumi_domain::SceneId::new(9),
        ) {
            Ok(plan) => plan,
            Err(error) => panic!("canonical scene edit must succeed: {error}"),
        };
        if let Err(error) = runtime
            .planning_worker
            .accept_revised_plan(&mut runtime.state, revised)
        {
            panic!("canonical scene edit must enter the runtime: {error}");
        }
        let Some(revised_plan) = runtime
            .state
            .state()
            .plan(lumi_domain::DeckId::new(2))
            .cloned()
        else {
            panic!("revised plan must remain available");
        };
        let locked = match runtime
            .planning_worker
            .planner
            .set_cue_lock(&revised_plan, 1, true)
        {
            Ok(plan) => plan,
            Err(error) => panic!("canonical cue lock must succeed: {error}"),
        };
        if let Err(error) = runtime
            .planning_worker
            .accept_revised_plan(&mut runtime.state, locked)
        {
            panic!("canonical cue lock must enter the runtime: {error}");
        }

        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetSimulationSpeed {
                expected_revision,
                speed: SimulationSpeed::SixtyFour,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Arm,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Start,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceToNextTrack { expected_revision }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceSimulation {
                expected_revision,
                elapsed_ticks: 250,
            }
        });
        let output_count_before_pause = runtime.output_worker.provider.records().count();
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Pause,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetSimulationPlayback {
                expected_revision,
                playing: false,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceSimulation {
                expected_revision,
                elapsed_ticks: 500,
            }
        });
        assert_eq!(
            runtime.output_worker.provider.records().count(),
            output_count_before_pause
        );
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Start,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetSimulationPlayback {
                expected_revision,
                playing: true,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceSimulation {
                expected_revision,
                elapsed_ticks: 750,
            }
        });

        let actual = canonical_e2e_evidence(&runtime, output_count_before_pause);
        let expected = include_bytes!("../../../../fixtures/demo-session-v1/canonical-e2e.json");
        assert_eq!(
            String::from_utf8_lossy(&actual),
            String::from_utf8_lossy(expected)
        );
    }

    #[test]
    fn queue_overload_forces_pause_and_cannot_emit_new_output() {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_operation(&mut runtime, 1, OperationCommand::Arm);
        apply_operation(&mut runtime, 2, OperationCommand::Start);
        apply_simulation_control(&mut runtime, SimulationControl::AdvanceLeader);
        let records_before_fault = runtime.output_worker.provider.records().count();
        if let Err(error) = process_domain_event(
            &mut runtime.state,
            &mut runtime.output_worker,
            DomainEvent::QueueOverloaded(lumi_domain::QueueOverloadEvent {
                occurred_at: MonotonicTime::new(1),
                rejected_kind: lumi_domain::DomainEventKind::Observation,
                rejected_critical: false,
                occurrences: 1,
            }),
        ) {
            panic!("queue overload must reduce safely: {error}");
        }
        assert_eq!(runtime.state.state().operation(), OperationState::Paused);
        assert_eq!(runtime.state.state().health(), RuntimeHealth::Degraded);
        assert_eq!(
            runtime.output_worker.provider.records().count(),
            records_before_fault
        );

        apply_simulation_control(
            &mut runtime,
            SimulationControl::SetSpeed(SimulationSpeed::SixtyFour),
        );
        assert!(runtime.clock.advance(1_000).is_some());
        if let Err(error) = runtime.deck_source.update_to_clock() {
            panic!("fault playback must remain processable: {error}");
        }
        if let Err(error) = process_pending_source_events(&mut runtime) {
            panic!("fault playback events must drain: {error}");
        }
        assert_eq!(
            runtime.output_worker.provider.records().count(),
            records_before_fault
        );
    }

    fn canonical_e2e_evidence(runtime: &EngineRuntime, paused_output_count: usize) -> Vec<u8> {
        let state = runtime.state.state();
        let Some(plan) = state.plan(lumi_domain::DeckId::new(2)) else {
            panic!("canonical evidence requires the deck 2 plan");
        };
        let Some(locked_cue) = plan.cues().get(1) else {
            panic!("canonical evidence requires the edited cue");
        };
        let timeline = state.timeline().collect::<Vec<_>>();
        let value = json!({
            "scenarioVersion": 1,
            "engineVersion": env!("CARGO_PKG_VERSION"),
            "operationState": operation_state_name(state.operation()),
            "simulation": {
                "speed": runtime.deck_source.speed().multiplier(),
                "paused": runtime.deck_source.is_paused(),
            },
            "leaderDeckId": state.leader_deck().map(lumi_domain::DeckId::value),
            "leaderBeat": state
                .deck(lumi_domain::DeckId::new(2))
                .map(lumi_domain::DeckState::beat),
            "plan": {
                "planId": plan.id().value().to_string(),
                "revision": plan.revision().value(),
                "status": plan_status_name(plan.status()),
                "lockedCue": {
                    "phraseIndex": locked_cue.phrase_index(),
                    "locked": locked_cue.locked(),
                    "origin": cue_origin_name(locked_cue.origin()),
                    "action": action_json(locked_cue.action()),
                },
            },
            "pausedOutputCount": paused_output_count,
            "outputRecordCount": runtime.output_worker.provider.records().count(),
            "outputEffects": state.output_effects().map(|result| {
                let request = result.request();
                json!({
                    "commandId": request.command_id().value(),
                    "phraseIndex": request.phrase_index(),
                    "planRevision": request.plan_revision().value(),
                    "scheduledAt": request.scheduled_at().ticks(),
                    "status": output_effect_status_name(result.status()),
                    "action": action_json(request.action()),
                })
            }).collect::<Vec<_>>(),
            "timeline": {
                "entryCount": timeline.len(),
                "lastSequence": timeline.last().map(|entry| entry.sequence()),
                "simulatedOutputEntries": timeline
                    .iter()
                    .filter(|entry| entry.result() == TimelineResult::Simulated)
                    .count(),
            },
        });
        let mut encoded = match serde_json::to_vec_pretty(&value) {
            Ok(encoded) => encoded,
            Err(error) => panic!("canonical evidence must encode: {error}"),
        };
        encoded.push(b'\n');
        encoded
    }

    fn canonical_library_simulation_evidence(
        preview: &Map<String, Value>,
        activated: &Map<String, Value>,
        completed: &Map<String, Value>,
    ) -> Vec<u8> {
        let next = &preview["nextPlan"];
        let cues = next["cues"]
            .as_array()
            .unwrap_or_else(|| panic!("golden evidence requires preview cues"))
            .iter()
            .map(|cue| {
                json!({
                    "phraseIndex": cue["phraseIndex"],
                    "startBeat": cue["startBeat"],
                    "endBeat": cue["endBeat"],
                    "roleId": cue["libraryResolution"]["roleId"],
                    "strategy": cue["libraryResolution"]["strategy"],
                    "variantId": cue["libraryResolution"]["variantId"],
                    "entryId": cue["libraryResolution"]["dryRunEntry"]["id"],
                })
            })
            .collect::<Vec<_>>();
        let outputs = completed["outputEffects"]
            .as_array()
            .unwrap_or_else(|| panic!("golden evidence requires output effects"))
            .iter()
            .map(|effect| {
                json!({
                    "phraseIndex": effect["phraseIndex"],
                    "status": effect["status"],
                    "entryId": effect["libraryResolution"]["dryRunEntry"]["id"],
                })
            })
            .collect::<Vec<_>>();
        let evidence = json!({
            "scenarioVersion": 1,
            "next": {
                "deckId": next["deckId"],
                "trackId": next["trackId"],
                "trackLoadId": next["trackLoadId"],
                "libraryTrack": next["libraryTrack"],
                "themeDecision": next["themeDecision"],
                "cues": cues,
            },
            "activation": {
                "leaderDeckId": activated["leaderDeckId"],
                "activePlan": activated["activePlan"],
            },
            "outputs": outputs,
        });
        let mut bytes = serde_json::to_vec_pretty(&evidence)
            .unwrap_or_else(|error| panic!("golden evidence must encode: {error}"));
        bytes.push(b'\n');
        bytes
    }

    fn semantic_output_order(speed: SimulationSpeed, elapsed_ticks: u64) -> Vec<(u16, u64)> {
        let mut runtime = match initialized_runtime() {
            Ok(runtime) => runtime,
            Err(error) => panic!("test engine must initialize: {error}"),
        };
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetSimulationSpeed {
                expected_revision,
                speed,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Arm,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::SetOperationState {
                expected_revision,
                command: OperationCommand::Start,
            }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceToNextTrack { expected_revision }
        });
        apply_current_session_command(&mut runtime, |expected_revision| {
            SessionCommand::AdvanceSimulation {
                expected_revision,
                elapsed_ticks,
            }
        });
        runtime
            .output_worker
            .provider
            .records()
            .map(|result| {
                (
                    result.request().phrase_index(),
                    result.request().cue_id().value(),
                )
            })
            .collect()
    }

    fn apply_session_command(runtime: &mut EngineRuntime, command: SessionCommand) {
        if let Err(error) = apply_command(runtime, command) {
            panic!("test session command must apply: {error}");
        }
    }

    fn apply_current_session_command(
        runtime: &mut EngineRuntime,
        command: impl FnOnce(lumi_domain::StateRevision) -> SessionCommand,
    ) {
        let expected_revision = runtime.state.state().revision();
        apply_session_command(runtime, command(expected_revision));
    }

    fn apply_operation(runtime: &mut EngineRuntime, sequence: u64, command: OperationCommand) {
        let expected_state_revision = runtime.state.state().revision();
        let event = DomainEvent::UserCommand(UserCommandEnvelope {
            client_id: ClientId::new(1),
            sequence: CommandSequence::new(sequence),
            expected_state_revision,
            issued_at: MonotonicTime::new(sequence),
            command,
        });
        if let Err(error) =
            process_domain_event(&mut runtime.state, &mut runtime.output_worker, event)
        {
            panic!("test operation must apply: {error}");
        }
    }

    fn apply_simulation_control(runtime: &mut EngineRuntime, control: SimulationControl) {
        if let Err(error) = runtime.deck_source.apply_control(control) {
            panic!("test simulation control must apply: {error}");
        }
        if let Err(error) = process_pending_source_events(runtime) {
            panic!("test source events must process: {error}");
        }
    }
}
