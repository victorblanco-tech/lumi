use std::env;
use std::io::{self, Write as _};
use std::net::Ipv4Addr;
use std::time::Duration;

use lumi_deck_source::DeckSourceProvider as _;
use lumi_domain::{
    DecisionReason, DeckSourceStatus, DomainEvent, KeyMode, MonotonicTime, OperationState,
    PhraseKind, PitchClass, RuntimeHealth, SerializedRuntime, SerializedRuntimeError,
};
use lumi_protocol::{MessageEnvelope, MessageType, PROTOCOL_VERSION};
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

const SESSION_TOKEN_ENVIRONMENT_KEY: &str = "LUMI_SESSION_TOKEN";
const MINIMUM_SESSION_TOKEN_BYTES: usize = 32;
const MAXIMUM_SESSION_TOKEN_BYTES: usize = 256;
const MAXIMUM_AUTHENTICATION_BYTES: usize = 512;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_QUEUE_CAPACITY: usize = 256;

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
    let runtime = initialized_runtime()?;

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let endpoint = listener.local_addr()?;
    write_startup_record(endpoint.port())?;

    let (stream, peer) = timeout(CONNECTION_TIMEOUT, listener.accept())
        .await
        .map_err(|_| EngineError::ConnectionTimeout)??;

    if !peer.ip().is_loopback() {
        return Err(EngineError::NonLoopbackPeer);
    }

    serve_authenticated_client(stream, &session_token, &runtime).await
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
    runtime: &EngineRuntime,
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

    let mut encoded_snapshot = serde_json::to_vec(&initial_snapshot(runtime)?)?;
    encoded_snapshot.push(b'\n');
    writer.write_all(&encoded_snapshot).await?;
    writer.flush().await?;

    let mut buffer = [0_u8; 1024];
    loop {
        if reader.read(&mut buffer).await? == 0 {
            break;
        }
    }

    Ok(())
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
}

fn initialized_runtime() -> Result<EngineRuntime, EngineError> {
    let mut runtime =
        SerializedRuntime::try_new(EVENT_QUEUE_CAPACITY).map_err(SerializedRuntimeError::from)?;
    runtime
        .submit(DomainEvent::RuntimeStarted {
            at: MonotonicTime::new(0),
        })
        .map_err(SerializedRuntimeError::from)?;
    if runtime
        .process_next()
        .map_err(SerializedRuntimeError::from)?
        .is_none()
    {
        return Err(SerializedRuntimeError::StartupEventMissing.into());
    }
    let mut deck_source = SimulatorDeckSourceProvider::demo(ManualClock::new(0))?;
    for event in deck_source.drain_events()? {
        runtime
            .submit(event)
            .map_err(SerializedRuntimeError::from)?;
    }
    while runtime
        .process_next()
        .map_err(SerializedRuntimeError::from)?
        .is_some()
    {}
    Ok(EngineRuntime {
        state: runtime,
        deck_source,
    })
}

fn initial_snapshot(runtime: &EngineRuntime) -> Result<MessageEnvelope, EngineError> {
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

    Ok(MessageEnvelope {
        protocol_version: PROTOCOL_VERSION,
        message_type: MessageType::Snapshot,
        message_id: "snapshot-initial".to_owned(),
        sequence: 1,
        correlation_id: "session-bootstrap".to_owned(),
        sent_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        payload,
    })
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
}
