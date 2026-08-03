use std::env;
use std::io::{self, Write as _};
use std::net::Ipv4Addr;
use std::time::Duration;

use lumi_deck_source::DeckSourceProvider as _;
use lumi_domain::{
    CueOrigin, CueReason, DecisionReason, DeckObservation, DeckSourceStatus, DomainEvent, EffectId,
    EffectResult, EffectResultEnvelope, EffectSequence, KeyMode, LightingPlan, MonotonicTime,
    OperationState, OutputEffectReason, OutputEffectResult, OutputEffectStatus,
    OutputExecutionRequest, PhraseKind, PitchClass, PlanRevision, PlanStatus, RuntimeHealth,
    SceneCategory, SemanticLightingAction, SerializedRuntime, SerializedRuntimeError, WorkerId,
};
use lumi_lighting_output::LightingOutputProvider as _;
use lumi_output_dry_run::{DryRunLightingOutputProvider, DryRunOutputError};
use lumi_planner::{
    DeterministicPlanner, PlanMutationError, PlannerError, PlannerTrack, PlanningInput,
    PlanningOptions, StableChoiceSource,
};
use lumi_protocol::{
    CommandDisposition, CommandIdCache, InvalidCacheCapacity, MAX_MESSAGE_BYTES, MessageDecoder,
    MessageEnvelope, MessageType, PROTOCOL_VERSION,
};
use lumi_simulator::{ManualClock, SimulatorDeckSourceProvider, SimulatorError};
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
use crate::commands::{SessionCommand, decode_command};

const SESSION_TOKEN_ENVIRONMENT_KEY: &str = "LUMI_SESSION_TOKEN";
const MINIMUM_SESSION_TOKEN_BYTES: usize = 32;
const MAXIMUM_SESSION_TOKEN_BYTES: usize = 256;
const MAXIMUM_AUTHENTICATION_BYTES: usize = 512;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_QUEUE_CAPACITY: usize = 256;
const COMMAND_ID_CACHE_CAPACITY: usize = 256;

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
    let mut runtime = initialized_runtime()?;

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
    deck_source: SimulatorDeckSourceProvider<ManualClock>,
    planning_worker: PlanningWorker,
    output_worker: OutputWorker,
}

fn initialized_runtime() -> Result<EngineRuntime, EngineError> {
    initialized_runtime_with_clock(ManualClock::new(0))
}

fn initialized_runtime_with_clock(clock: ManualClock) -> Result<EngineRuntime, EngineError> {
    let mut runtime =
        SerializedRuntime::try_new(EVENT_QUEUE_CAPACITY).map_err(SerializedRuntimeError::from)?;
    submit_and_process(
        &mut runtime,
        DomainEvent::RuntimeStarted {
            at: MonotonicTime::new(0),
        },
    )?;
    let mut deck_source = SimulatorDeckSourceProvider::demo(clock)?;
    let mut planning_worker = PlanningWorker::new();
    let mut output_worker = OutputWorker::new();
    for event in deck_source.drain_events()? {
        planning_worker.process_source_event(
            &mut runtime,
            &mut output_worker,
            event,
            deck_source.leader_deck_id(),
        )?;
    }
    Ok(EngineRuntime {
        state: runtime,
        deck_source,
        planning_worker,
        output_worker,
    })
}

#[cfg(test)]
fn process_pending_source_events(runtime: &mut EngineRuntime) -> Result<(), EngineError> {
    let leader_deck_id = runtime.deck_source.leader_deck_id();
    for event in runtime.deck_source.drain_events()? {
        runtime.planning_worker.process_source_event(
            &mut runtime.state,
            &mut runtime.output_worker,
            event,
            leader_deck_id,
        )?;
    }
    Ok(())
}

struct PlanningWorker {
    planner: DeterministicPlanner<StableChoiceSource>,
    effect_sequence: u64,
}

impl PlanningWorker {
    fn new() -> Self {
        Self {
            planner: DeterministicPlanner::epic_one(),
            effect_sequence: 0,
        }
    }

    fn process_source_event(
        &mut self,
        runtime: &mut SerializedRuntime,
        output_worker: &mut OutputWorker,
        event: DomainEvent,
        leader_deck_id: lumi_domain::DeckId,
    ) -> Result<(), EngineError> {
        let planning_input = match &event {
            DomainEvent::Observation(observation) => match &observation.observation {
                DeckObservation::TrackLoaded {
                    deck_id,
                    metadata,
                    track_load_id,
                } if *deck_id != leader_deck_id => Some(PlanningInput {
                    deck_id: *deck_id,
                    track_load_id: *track_load_id,
                    track: PlannerTrack::analyzed(metadata),
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
            let plan = self.planner.generate(&input)?;
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
    effect_sequence: u64,
}

impl OutputWorker {
    fn new() -> Self {
        Self {
            provider: DryRunLightingOutputProvider::default(),
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
                    let result = if execution_context_is_current(runtime.state(), &request) {
                        self.provider.execute(&request, request.scheduled_at())?
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

    if command.is_mutating() && command_ids.contains(&envelope.message_id) {
        return snapshot_envelope(runtime, response_sequence, &envelope.message_id);
    }

    if let Err(error) = apply_command(runtime, command) {
        return application_error_envelope(response_sequence, &envelope.message_id, &error);
    }
    if command.is_mutating() {
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
    if command == SessionCommand::GetSnapshot {
        return Ok(());
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
                track: PlannerTrack::analyzed(deck.metadata()),
            },
        )
    };

    let revised = match command {
        SessionCommand::GetSnapshot => return Ok(()),
        SessionCommand::SelectTheme { theme_id, .. } => runtime
            .planning_worker
            .planner
            .select_theme(&current, theme_id)?,
        SessionCommand::SelectScene {
            phrase_index,
            scene_id,
            ..
        } => runtime
            .planning_worker
            .planner
            .select_scene(&current, phrase_index, scene_id)?,
        SessionCommand::SetCueLock {
            phrase_index,
            locked,
            ..
        } => runtime
            .planning_worker
            .planner
            .set_cue_lock(&current, phrase_index, locked)?,
        SessionCommand::RegeneratePlan { .. } => runtime
            .planning_worker
            .planner
            .regenerate(&current, &input)?,
    };
    runtime
        .planning_worker
        .accept_revised_plan(&mut runtime.state, revised)
        .map_err(CommandApplicationError::Engine)
}

fn application_error_envelope(
    sequence: u64,
    correlation_id: &str,
    error: &CommandApplicationError,
) -> Result<MessageEnvelope, EngineError> {
    match error {
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
        CommandApplicationError::Mutation(mutation) => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "planMutationRejected",
            &mutation.to_string(),
            false,
            None,
        ),
        CommandApplicationError::MissingPlanContext | CommandApplicationError::Engine(_) => {
            error_envelope(
                sequence,
                correlation_id,
                "commandFailed",
                "internalCommandFailure",
                "The plan edit could not be applied safely.",
                true,
                None,
            )
        }
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
    #[error("the track-load instance no longer matches")]
    TrackLoadMismatch,
    #[error("the plan is unavailable")]
    PlanUnavailable,
    #[error("plan mutation failed: {0}")]
    Mutation(#[from] PlanMutationError),
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
            "providerKind": runtime.deck_source.provider_kind(),
            "status": state
                .source_statuses()
                .next()
                .map(|(_, status)| deck_source_status_name(status))
                .unwrap_or("starting"),
        }),
    );
    payload.insert(
        "leaderDeckId".to_owned(),
        json!(state.leader_deck().map(|deck_id| deck_id.value())),
    );
    let decks = state
        .decks()
        .map(|(deck_id, deck)| {
            let metadata = deck.metadata();
            json!({
                "deckId": deck_id.value(),
                "trackLoadId": deck.track_load_id().value(),
                "beat": deck.beat(),
                "phraseIndex": deck.phrase_index(),
                "track": {
                    "id": metadata.id().value(),
                    "title": metadata.title(),
                    "artist": metadata.artist(),
                    "bpmMilli": metadata.bpm_milli(),
                    "key": {
                        "pitchClass": pitch_class_name(metadata.musical_key().pitch_class()),
                        "mode": key_mode_name(metadata.musical_key().mode()),
                    },
                    "durationBeats": metadata.duration_beats(),
                    "phrases": metadata.phrases().iter().map(|phrase| json!({
                        "index": phrase.index(),
                        "startBeat": phrase.start_beat(),
                        "endBeat": phrase.end_beat(),
                        "kind": phrase_kind_name(phrase.kind()),
                    })).collect::<Vec<_>>(),
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
        Value::Array(state.output_effects().map(output_effect_json).collect()),
    );
    let next_plan = state
        .decks()
        .find(|(deck_id, _)| Some(*deck_id) != state.leader_deck())
        .and_then(|(deck_id, _)| state.plan(deck_id))
        .map(plan_json)
        .unwrap_or(Value::Null);
    payload.insert("nextPlan".to_owned(), next_plan);

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

fn output_effect_json(result: &OutputEffectResult) -> Value {
    let request = result.request();
    json!({
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
    })
}

fn plan_json(plan: &LightingPlan) -> Value {
    json!({
        "planId": plan.id().value().to_string(),
        "deckId": plan.deck_id().value(),
        "trackId": plan.track_id().value(),
        "trackDurationBeats": plan.track_duration_beats(),
        "trackLoadId": plan.track_load_id().value(),
        "revision": plan.revision().value(),
        "configurationRevision": plan.configuration_revision().value(),
        "seed": plan.seed().to_string(),
        "status": plan_status_name(plan.status()),
        "cues": plan.cues().iter().map(|cue| json!({
            "phraseIndex": cue.phrase_index(),
            "startBeat": cue.start_beat(),
            "endBeat": cue.end_beat(),
            "origin": cue_origin_name(cue.origin()),
            "locked": cue.locked(),
            "reason": cue_reason_json(cue.reason()),
            "action": action_json(cue.action()),
        })).collect::<Vec<_>>(),
    })
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
    #[error("planner failed: {0}")]
    Planner(#[from] PlannerError),
    #[error("the serialized runtime lost an event after accepting it")]
    SubmittedEventMissing,
    #[error("the planning worker effect sequence overflowed")]
    PlanningEffectSequenceOverflow,
    #[error("the output worker effect sequence overflowed")]
    OutputEffectSequenceOverflow,
    #[error("dry-run output failed: {0}")]
    DryRunOutput(#[from] DryRunOutputError),
    #[error("the response sequence overflowed")]
    ResponseSequenceOverflow,
    #[error("the command ID cache could not initialize: {0}")]
    CommandCache(#[from] InvalidCacheCapacity),
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumi_domain::{ClientId, CommandSequence, OperationCommand, UserCommandEnvelope};
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
                source.leader_deck_id(),
            ) {
                panic!("test source event must process: {error}");
            }
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
