use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write as _};
use std::net::Ipv4Addr;
#[cfg(not(test))]
use std::path::PathBuf;
use std::time::{Duration, Instant};

use lumi_blt_midi::{BltMidiDeckSourceProvider, BltMidiError};
use lumi_deck_source::DeckSourceProvider as _;
use lumi_domain::{
    ClientId, CommandSequence, CueOrigin, CueReason, DecisionReason, DeckObservation,
    DeckSourceStatus, DomainEvent, EffectId, EffectResult, EffectResultEnvelope, EffectSequence,
    KeyMode, LightingLook, LightingPlan, MonotonicTime, OperationCommand, OperationState,
    OutputEffectReason, OutputEffectResult, OutputEffectStatus, OutputExecutionRequest, PhraseKind,
    PitchClass, PlanConfigurationRevision, PlanRevision, PlanStatus, PlanValidationError,
    RuntimeHealth, SceneCategory, SceneId, SemanticLightingAction, SerializedRuntime,
    SerializedRuntimeError, TimelineResult, TimelineSource, TrackLoadId, TrackMetadata,
    UserCommandEnvelope, WorkerId,
};
use lumi_library::AutoloopCatalog;
use lumi_lighting_output::LightingOutputProvider as _;
use lumi_local_playback::{LocalPlaybackDeckSourceProvider, LocalPlaybackError};
#[cfg(not(test))]
use lumi_midi_coremidi::DECK_INPUT_DESTINATION_NAME;
#[cfg(test)]
use lumi_midi_coremidi::MidiChannelVoiceMessage;
use lumi_midi_coremidi::{
    CoreMidiDestinationProvider, CoreMidiSourceProvider, MidiDestinationState,
};
use lumi_midi_output::{
    BANK_SETTLE_DELAY, MidiClockController, MidiClockState, MidiClockSync, MidiOutputController,
    MidiSourceState,
};
use lumi_output_dry_run::{DryRunLightingOutputProvider, DryRunOutputError};
use lumi_planner::{
    DeterministicPlanner, PlanMutationError, PlannerError, PlannerTrack, PlanningConfiguration,
    PlanningInput, PlanningOptions, StableChoiceSource, ThemeOption, ThemeSelectionContext,
};
#[cfg(not(test))]
use lumi_prolink_input::{
    BridgeLaunchConfiguration, BridgeProcessSupervisor, BridgeSupervisorError,
    ensure_prolink_network_available,
};
use lumi_prolink_input::{ProLinkDeckSourceProvider, ProLinkProviderError};
use lumi_protocol::{
    CommandDisposition, CommandIdCache, InvalidCacheCapacity, MAX_MESSAGE_BYTES, MessageDecoder,
    MessageEnvelope, MessageType, PROTOCOL_VERSION,
};
use lumi_simulator::{
    ManualClock, MonotonicClock as _, SimulationControl, SimulatorDeckSourceProvider,
    SimulatorError,
};
use lumi_timing_output::{
    CarabinerConfiguration, CarabinerTimingOutput, TimingAnchor, TimingDiscontinuity,
    TimingOutputProvider as _, TimingOutputState, TimingSourceKind,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWriteExt as _, BufReader,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{MissedTickBehavior, timeout};

use crate::StartupReady;
use crate::commands::{DeckSourceSelection, SessionCommand, decode_command};
use crate::library::{LibraryPlanContext, LibraryWorker, LibraryWorkerError, ResolvedLibraryCue};

const SESSION_TOKEN_ENVIRONMENT_KEY: &str = "LUMI_SESSION_TOKEN";
#[cfg(not(test))]
const DECK_INPUT_NAME_ENVIRONMENT_KEY: &str = "LUMI_DECK_INPUT_DESTINATION_NAME";
#[cfg(not(test))]
const DECK_INPUT_DISABLED_ENVIRONMENT_KEY: &str = "LUMI_DECK_INPUT_DISABLED";
#[cfg(not(test))]
const AUTO_PUBLISH_MIDI_ENVIRONMENT_KEY: &str = "LUMI_AUTO_PUBLISH_MIDI";
const MINIMUM_SESSION_TOKEN_BYTES: usize = 32;
const MAXIMUM_SESSION_TOKEN_BYTES: usize = 256;
const MAXIMUM_AUTHENTICATION_BYTES: usize = 512;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const INTEGRATION_PUMP_INTERVAL: Duration = Duration::from_millis(20);
const MINIMUM_PROLINK_TIMING_STALE_AFTER: Duration = Duration::from_millis(1_250);
const MAXIMUM_PROLINK_TIMING_STALE_AFTER: Duration = Duration::from_secs(5);
const EVENT_QUEUE_CAPACITY: usize = 256;
const COMMAND_ID_CACHE_CAPACITY: usize = 256;
const LIBRARY_CONTEXT_CAPACITY: usize = 256;
#[cfg(not(test))]
const PROLINK_JAVA_ENVIRONMENT_KEY: &str = "LUMI_PROLINK_JAVA";
#[cfg(not(test))]
const PROLINK_BRIDGE_JAR_ENVIRONMENT_KEY: &str = "LUMI_PROLINK_BRIDGE_JAR";
#[cfg(not(test))]
const CARABINER_EXECUTABLE_ENVIRONMENT_KEY: &str = "LUMI_CARABINER_EXECUTABLE";

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
    let mut command_reader = BufReader::new(reader);
    let mut command_buffer = Vec::with_capacity(256);
    let mut integration_pump = tokio::time::interval(INTEGRATION_PUMP_INTERVAL);
    integration_pump.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // The first interval tick is immediately ready. Consume it so deck input
    // cadence starts after one complete interval rather than racing bootstrap.
    integration_pump.tick().await;

    loop {
        tokio::select! {
            biased;
            _ = integration_pump.tick() => {
                // Pro DJ Link timing is a realtime engine responsibility. It
                // must keep flowing when SwiftUI is busy, hidden or not asking
                // for snapshots.
                runtime.integration_pump_metrics.record(Instant::now());
                process_deck_input_messages(runtime)?;
                runtime
                    .output_worker
                    .service_pending_autoloop(runtime.state.state(), false);
            }
            command = read_command_line(&mut command_reader, &mut command_buffer) => {
                let Some(command_bytes) = command? else { break };
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
        }
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

async fn read_command_line<R>(
    reader: &mut R,
    bytes: &mut Vec<u8>,
) -> Result<Option<Vec<u8>>, EngineError>
where
    R: AsyncBufRead + Unpin,
{
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "command ended before a newline",
            )));
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let payload_length = newline.unwrap_or(available.len());
        if bytes.len().saturating_add(payload_length) > MAX_MESSAGE_BYTES {
            return Err(EngineError::MessageOversized);
        }
        bytes.extend_from_slice(&available[..payload_length]);
        let consumed = payload_length.saturating_add(usize::from(newline.is_some()));
        reader.consume(consumed);
        if newline.is_some() {
            let line = bytes.clone();
            bytes.clear();
            return Ok(Some(line));
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
    connected_deck_source: BltMidiDeckSourceProvider,
    direct_deck_source: ProLinkDeckSourceProvider,
    #[cfg(not(test))]
    prolink_bridge: Option<BridgeProcessSupervisor>,
    prolink_start_error: Option<String>,
    #[cfg(not(test))]
    prolink_recovery_pending: bool,
    #[cfg(not(test))]
    last_prolink_restart_attempt: Option<Instant>,
    #[cfg(not(test))]
    prolink_restart_count: u64,
    deck_source_mode: DeckSourceMode,
    planning_worker: PlanningWorker,
    output_worker: OutputWorker,
    deck_input: CoreMidiDestinationProvider,
    library_worker: LibraryWorker,
    operation_sequence: u64,
    local_transports: BTreeMap<lumi_domain::DeckId, LocalTransportObservation>,
    integration_pump_metrics: IntegrationPumpMetrics,
}

#[derive(Clone, Copy, Debug)]
struct LocalTransportObservation {
    track_load_id: TrackLoadId,
    position_millis: u64,
    playing: bool,
    observed_at: Instant,
}

#[derive(Clone, Copy, Debug)]
struct IntegrationPumpMetrics {
    last_tick: Option<Instant>,
    tick_count: u64,
    starvation_count: u64,
    max_lateness_micros: u64,
}

impl IntegrationPumpMetrics {
    const fn new() -> Self {
        Self {
            last_tick: None,
            tick_count: 0,
            starvation_count: 0,
            max_lateness_micros: 0,
        }
    }

    fn record(&mut self, now: Instant) {
        self.tick_count = self.tick_count.saturating_add(1);
        if let Some(previous) = self.last_tick {
            let elapsed = now.saturating_duration_since(previous);
            let lateness = elapsed.saturating_sub(INTEGRATION_PUMP_INTERVAL);
            self.max_lateness_micros = self
                .max_lateness_micros
                .max(u64::try_from(lateness.as_micros()).unwrap_or(u64::MAX));
            if elapsed >= INTEGRATION_PUMP_INTERVAL.saturating_mul(2) {
                self.starvation_count = self.starvation_count.saturating_add(1);
            }
        }
        self.last_tick = Some(now);
    }
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
            DeckSourceMode::ConnectedDecks if self.uses_direct_prolink() => "directProDjLink",
            DeckSourceMode::ConnectedDecks => "beatLinkTriggerMidi",
            DeckSourceMode::LocalPlayback => "localPlayback",
            DeckSourceMode::Simulator => "simulator",
        }
    }

    fn leader_deck_id(&self) -> Option<lumi_domain::DeckId> {
        match self.deck_source_mode {
            DeckSourceMode::ConnectedDecks if self.uses_direct_prolink() => {
                self.direct_deck_source.leader_deck_id()
            }
            DeckSourceMode::ConnectedDecks => self.connected_deck_source.leader_deck_id(),
            DeckSourceMode::LocalPlayback => self.local_deck_source.leader_deck_id(),
            DeckSourceMode::Simulator => Some(self.deck_source.leader_deck_id()),
        }
    }

    fn direct_prolink_active(&self) -> bool {
        #[cfg(not(test))]
        {
            self.prolink_bridge.is_some()
        }
        #[cfg(test)]
        {
            false
        }
    }

    fn uses_direct_prolink(&self) -> bool {
        self.direct_prolink_active() || self.prolink_recovery_pending()
    }

    fn prolink_recovery_pending(&self) -> bool {
        #[cfg(not(test))]
        {
            self.prolink_recovery_pending
        }
        #[cfg(test)]
        {
            false
        }
    }

    fn prolink_restart_count(&self) -> u64 {
        #[cfg(not(test))]
        {
            self.prolink_restart_count
        }
        #[cfg(test)]
        {
            0
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
    let mut connected_deck_source = BltMidiDeckSourceProvider::new(clock.now())?;
    let mut direct_deck_source = ProLinkDeckSourceProvider::new(clock.now())?;
    #[cfg(not(test))]
    let (prolink_bridge, prolink_start_error) =
        if deck_source_mode == DeckSourceMode::ConnectedDecks {
            launch_prolink_bridge()
        } else {
            (None, None)
        };
    #[cfg(test)]
    let prolink_start_error = None;
    let mut output_worker = OutputWorker::new();
    #[cfg(not(test))]
    if env::var(AUTO_PUBLISH_MIDI_ENVIRONMENT_KEY).as_deref() != Ok("0") {
        let _ = output_worker.enable_midi_auto_publish();
    }
    #[cfg(not(test))]
    let mut deck_input = CoreMidiDestinationProvider::new();
    #[cfg(test)]
    let deck_input = CoreMidiDestinationProvider::new();
    #[cfg(not(test))]
    if env::var(DECK_INPUT_DISABLED_ENVIRONMENT_KEY).as_deref() != Ok("1") {
        deck_input
            .publish(
                &env::var(DECK_INPUT_NAME_ENVIRONMENT_KEY)
                    .unwrap_or_else(|_| DECK_INPUT_DESTINATION_NAME.to_owned()),
            )
            .map_err(|error| EngineError::Midi(error.to_string()))?;
    }
    let library_worker = LibraryWorker::demo()?;
    let autoloop_catalog = library_worker.autoloop_catalog()?;
    let mut planning_worker = PlanningWorker::new(&autoloop_catalog);
    match deck_source_mode {
        DeckSourceMode::ConnectedDecks => {
            #[cfg(not(test))]
            let direct_active = prolink_bridge.is_some();
            #[cfg(test)]
            let direct_active = false;
            if direct_active {
                for event in direct_deck_source.drain_events()? {
                    planning_worker.process_source_event(
                        &mut runtime,
                        &mut output_worker,
                        event,
                        direct_deck_source.leader_deck_id(),
                    )?;
                }
            } else {
                for event in connected_deck_source.drain_events()? {
                    planning_worker.process_source_event(
                        &mut runtime,
                        &mut output_worker,
                        event,
                        connected_deck_source.leader_deck_id(),
                    )?;
                }
            }
        }
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
        connected_deck_source,
        direct_deck_source,
        #[cfg(not(test))]
        prolink_bridge,
        prolink_start_error,
        #[cfg(not(test))]
        prolink_recovery_pending: false,
        #[cfg(not(test))]
        last_prolink_restart_attempt: None,
        #[cfg(not(test))]
        prolink_restart_count: 0,
        deck_source_mode,
        planning_worker,
        output_worker,
        deck_input,
        library_worker,
        operation_sequence: 0,
        local_transports: BTreeMap::new(),
        integration_pump_metrics: IntegrationPumpMetrics::new(),
    })
}

#[cfg(not(test))]
fn launch_prolink_bridge() -> (Option<BridgeProcessSupervisor>, Option<String>) {
    let Some(configuration) = prolink_bridge_configuration() else {
        return (
            None,
            Some("The bundled Direct Pro DJ Link bridge is unavailable.".to_owned()),
        );
    };
    if let Err(error) = ensure_prolink_network_available() {
        return (None, Some(error.to_string()));
    }
    match BridgeProcessSupervisor::spawn(&configuration) {
        Ok(supervisor) => (Some(supervisor), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

#[cfg(not(test))]
fn prolink_bridge_configuration() -> Option<BridgeLaunchConfiguration> {
    if let (Ok(java), Ok(jar)) = (
        env::var(PROLINK_JAVA_ENVIRONMENT_KEY),
        env::var(PROLINK_BRIDGE_JAR_ENVIRONMENT_KEY),
    ) {
        let java = PathBuf::from(java);
        let jar = PathBuf::from(jar);
        if java.is_file() && jar.is_file() {
            return Some(BridgeLaunchConfiguration::java_jar(java, jar));
        }
    }

    let helper_directory = env::current_exe().ok()?.parent()?.to_path_buf();
    let bundled_java = helper_directory.join("../Resources/prolink-runtime/bin/java");
    let bundled_jar = helper_directory.join("../Resources/prolink/lumi-prolink-bridge.jar");
    if bundled_java.is_file() && bundled_jar.is_file() {
        return Some(BridgeLaunchConfiguration::java_jar(
            bundled_java,
            bundled_jar,
        ));
    }
    // Compatibility with locally built app bundles created before the runtime
    // moved out of `Contents/Helpers` into the code-signing-safe Resources lane.
    let legacy_bundled_java = helper_directory.join("prolink-runtime/bin/java");
    let legacy_bundled_jar = helper_directory.join("prolink/lumi-prolink-bridge.jar");
    if legacy_bundled_java.is_file() && legacy_bundled_jar.is_file() {
        return Some(BridgeLaunchConfiguration::java_jar(
            legacy_bundled_java,
            legacy_bundled_jar,
        ));
    }

    #[cfg(debug_assertions)]
    {
        let development_jar = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../bridges/prolink/target/lumi-prolink-bridge.jar");
        for development_java in [
            PathBuf::from("/opt/homebrew/opt/openjdk@21/bin/java"),
            PathBuf::from("/usr/local/opt/openjdk@21/bin/java"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../build/package-toolchains/temurin-21-macos-aarch64/Contents/Home/bin/java",
            ),
        ] {
            if development_java.is_file() && development_jar.is_file() {
                return Some(BridgeLaunchConfiguration::java_jar(
                    development_java,
                    development_jar,
                ));
            }
        }
    }
    None
}

#[cfg(not(test))]
fn carabiner_configuration() -> CarabinerConfiguration {
    let executable = env::var(CARABINER_EXECUTABLE_ENVIRONMENT_KEY)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            let helper_directory = env::current_exe().ok()?.parent()?.to_path_buf();
            let bundled = helper_directory.join("../Resources/link/Carabiner");
            bundled.is_file().then_some(bundled)
        })
        .or_else(|| {
            #[cfg(debug_assertions)]
            {
                let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../build/carabiner-runtime/Carabiner");
                return development.is_file().then_some(development);
            }
            #[allow(unreachable_code)]
            None
        });
    CarabinerConfiguration {
        executable,
        ..CarabinerConfiguration::default()
    }
}

fn process_pending_source_events(runtime: &mut EngineRuntime) -> Result<(), EngineError> {
    let leader_deck_id = runtime.leader_deck_id();
    match runtime.deck_source_mode {
        DeckSourceMode::ConnectedDecks => {
            if runtime.uses_direct_prolink() {
                for event in runtime.direct_deck_source.drain_events()? {
                    let event = hydrate_direct_library_event(runtime, event)?;
                    runtime.planning_worker.process_source_event(
                        &mut runtime.state,
                        &mut runtime.output_worker,
                        event,
                        leader_deck_id,
                    )?;
                }
            } else {
                for event in runtime.connected_deck_source.drain_events()? {
                    let event = hydrate_connected_library_event(runtime, event)?;
                    runtime.planning_worker.process_source_event(
                        &mut runtime.state,
                        &mut runtime.output_worker,
                        event,
                        leader_deck_id,
                    )?;
                }
            }
        }
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

fn hydrate_direct_library_event(
    runtime: &mut EngineRuntime,
    event: DomainEvent,
) -> Result<DomainEvent, EngineError> {
    let DomainEvent::Observation(mut envelope) = event else {
        return Ok(event);
    };
    let DeckObservation::TrackLoaded {
        deck_id,
        track_load_id,
        ..
    } = envelope.observation
    else {
        return Ok(DomainEvent::Observation(envelope));
    };
    let Some(identity) = runtime.direct_deck_source.track_identity(track_load_id) else {
        return Ok(DomainEvent::Observation(envelope));
    };
    let Some(connected) = runtime
        .library_worker
        .connected_track(identity.rekordbox_id, 0)?
    else {
        return Ok(DomainEvent::Observation(envelope));
    };
    let (metadata, context) = connected.prepared.into_parts();
    let _ = runtime
        .direct_deck_source
        .hydrate_track_metadata(track_load_id, metadata.clone());
    runtime
        .planning_worker
        .register_library_context(track_load_id, context);
    envelope.observation = DeckObservation::TrackLoaded {
        deck_id,
        metadata,
        track_load_id,
    };
    Ok(DomainEvent::Observation(envelope))
}

fn hydrate_connected_library_event(
    runtime: &mut EngineRuntime,
    event: DomainEvent,
) -> Result<DomainEvent, EngineError> {
    let DomainEvent::Observation(mut envelope) = event else {
        return Ok(event);
    };
    let DeckObservation::TrackLoaded {
        deck_id,
        track_load_id,
        ..
    } = envelope.observation
    else {
        return Ok(DomainEvent::Observation(envelope));
    };
    let Some(identity) = runtime.connected_deck_source.track_identity(track_load_id) else {
        return Ok(DomainEvent::Observation(envelope));
    };
    let Some(connected) = runtime
        .library_worker
        .connected_track(identity.rekordbox_id, identity.simulator_signature)?
    else {
        return Ok(DomainEvent::Observation(envelope));
    };
    let (metadata, context) = connected.prepared.into_parts();
    runtime
        .planning_worker
        .register_library_context(track_load_id, context);
    envelope.observation = DeckObservation::TrackLoaded {
        deck_id,
        metadata,
        track_load_id,
    };
    Ok(DomainEvent::Observation(envelope))
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
    fn new(catalog: &AutoloopCatalog) -> Self {
        Self {
            planner: planner_for_catalog(catalog),
            effect_sequence: 0,
            recent_theme_ids: Vec::new(),
            library_contexts: BTreeMap::new(),
        }
    }

    fn synchronize_themes(&mut self, catalog: &AutoloopCatalog) {
        self.planner = planner_for_catalog(catalog);
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
        leader_deck_id: Option<lumi_domain::DeckId>,
    ) -> Result<(), EngineError> {
        let activates_pending_timing = matches!(
            &event,
            DomainEvent::Observation(lumi_domain::ObservationEnvelope {
                observation: DeckObservation::PhraseChanged { deck_id, .. },
                ..
            }) if Some(*deck_id) == leader_deck_id
                && runtime.state().operation() == OperationState::Live
                && runtime.state().deck(*deck_id).is_some_and(lumi_domain::DeckState::is_playing)
        );
        let planning_input = match &event {
            DomainEvent::Observation(observation) => match &observation.observation {
                DeckObservation::TrackLoaded {
                    deck_id,
                    metadata,
                    track_load_id,
                } if !metadata.phrases().is_empty()
                    || self.library_context(*track_load_id).is_some() =>
                {
                    Some(PlanningInput {
                        deck_id: *deck_id,
                        track_load_id: *track_load_id,
                        track: planner_track(metadata),
                    })
                }
                _ => None,
            },
            _ => None,
        };
        let observed_at = event.monotonic_time();
        if activates_pending_timing {
            // Timing is engine-owned and changes only at an authoritative Live
            // phrase boundary. A UI adjustment can therefore never shift the
            // active phrase or replay its already executed cue.
            output_worker.activate_pending_timing_offset();
        }
        process_domain_event(runtime, output_worker, event)?;
        if let Some(input) = planning_input {
            self.effect_sequence = self
                .effect_sequence
                .checked_add(1)
                .ok_or(EngineError::PlanningEffectSequenceOverflow)?;
            let context = ThemeSelectionContext::new(self.recent_theme_ids.clone());
            let generated = if let Some(library_context) = self.library_context(input.track_load_id)
            {
                let themes = library_context.executable_themes();
                let planner = planner_for_themes(library_context.catalog_revision(), themes)?;
                planner.generate_with_context(&input, &context)?
            } else {
                self.planner.generate_with_context(&input, &context)?
            };
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

fn planner_for_catalog(catalog: &AutoloopCatalog) -> DeterministicPlanner<StableChoiceSource> {
    let themes = catalog
        .themes()
        .iter()
        .map(|theme| ThemeOption {
            id: theme.id(),
            name: theme.display_name().to_owned(),
        })
        .collect::<Vec<_>>();
    planner_for_theme_options(catalog.revision(), themes)
        .unwrap_or_else(|_| DeterministicPlanner::epic_one())
}

fn planner_for_themes(
    catalog_revision: u64,
    themes: Vec<(lumi_domain::ThemeId, String)>,
) -> Result<DeterministicPlanner<StableChoiceSource>, EngineError> {
    planner_for_theme_options(
        catalog_revision,
        themes
            .into_iter()
            .map(|(id, name)| ThemeOption { id, name })
            .collect(),
    )
}

fn planner_for_theme_options(
    catalog_revision: u64,
    themes: Vec<ThemeOption>,
) -> Result<DeterministicPlanner<StableChoiceSource>, EngineError> {
    let Some(default_theme) = themes.first().map(|theme| theme.id) else {
        return Err(EngineError::NoExecutableLibraryTheme);
    };
    let configuration = PlanningConfiguration::epic_one()
        .with_themes(
            PlanConfigurationRevision::new(catalog_revision.max(1)),
            themes,
        )
        .with_default_theme(default_theme);
    Ok(DeterministicPlanner::new(configuration, StableChoiceSource))
}

struct OutputWorker {
    provider: DryRunLightingOutputProvider,
    midi_output: MidiOutputController<CoreMidiSourceProvider>,
    midi_clock: MidiClockController<CoreMidiSourceProvider>,
    link_timing: CarabinerTimingOutput,
    link_enabled: bool,
    effect_sequence: u64,
    midi_auto_publish_enabled: bool,
    timing_offset_millis: i16,
    pending_timing_offset_millis: Option<i16>,
    last_midi_publish_attempt: Option<Instant>,
    local_timing_generation: u64,
    last_prolink_timing_at: Option<Instant>,
    last_prolink_bpm_milli: Option<u32>,
    last_prolink_playing: Option<bool>,
    prolink_timing_stale: bool,
    precise_autoloop_fallback: bool,
    scheduled_prearm: Option<ScheduledPrearm>,
    prearmed_autoloop: Option<PrearmedAutoloop>,
    pending_autoloop: Option<PendingAutoloop>,
    autoloop_requested_count: u64,
    autoloop_prearmed_count: u64,
    autoloop_emitted_count: u64,
    autoloop_cancelled_count: u64,
    autoloop_late_count: u64,
    autoloop_beat_fallback_count: u64,
}

#[derive(Clone, Debug)]
struct PrearmedAutoloop {
    bank_number: u8,
    selected_at: Instant,
    deck_id: lumi_domain::DeckId,
    track_load_id: TrackLoadId,
    plan_revision: PlanRevision,
    phrase_index: u16,
}

#[derive(Clone, Debug)]
struct ScheduledPrearm {
    bank_number: u8,
    due_at: Instant,
    deck_id: lumi_domain::DeckId,
    track_load_id: TrackLoadId,
    plan_revision: PlanRevision,
    phrase_index: u16,
}

#[derive(Clone, Debug)]
struct PendingAutoloop {
    request: OutputExecutionRequest,
    bank_number: u8,
    autoloop_number: u8,
    selected_at: Instant,
    requires_precise_beat: bool,
}

impl OutputWorker {
    fn new() -> Self {
        #[cfg(not(test))]
        let link_configuration = carabiner_configuration();
        #[cfg(test)]
        let link_configuration = CarabinerConfiguration::default();
        Self {
            provider: DryRunLightingOutputProvider::default(),
            midi_output: MidiOutputController::new(CoreMidiSourceProvider::new()),
            midi_clock: MidiClockController::new(CoreMidiSourceProvider::new),
            link_timing: CarabinerTimingOutput::new(link_configuration),
            link_enabled: false,
            effect_sequence: 0,
            midi_auto_publish_enabled: false,
            timing_offset_millis: 0,
            pending_timing_offset_millis: None,
            last_midi_publish_attempt: None,
            local_timing_generation: 0,
            last_prolink_timing_at: None,
            last_prolink_bpm_milli: None,
            last_prolink_playing: None,
            prolink_timing_stale: false,
            precise_autoloop_fallback: false,
            scheduled_prearm: None,
            prearmed_autoloop: None,
            pending_autoloop: None,
            autoloop_requested_count: 0,
            autoloop_prearmed_count: 0,
            autoloop_emitted_count: 0,
            autoloop_cancelled_count: 0,
            autoloop_late_count: 0,
            autoloop_beat_fallback_count: 0,
        }
    }

    fn set_precise_autoloop_fallback(&mut self, enabled: bool) {
        self.precise_autoloop_fallback = enabled;
        if !enabled {
            self.scheduled_prearm = None;
            self.prearmed_autoloop = None;
        }
    }

    fn schedule_autoloop(
        &mut self,
        state: &lumi_domain::RuntimeState,
        request: &OutputExecutionRequest,
        bank_number: u8,
        autoloop_number: u8,
    ) {
        self.autoloop_requested_count = self.autoloop_requested_count.saturating_add(1);
        let now = Instant::now();
        let is_safely_prearmed = self.prearmed_autoloop.as_ref().is_some_and(|prearmed| {
            prearmed.bank_number == bank_number
                && prearmed.deck_id == request.deck_id()
                && prearmed.track_load_id == request.track_load_id()
                && prearmed.plan_revision == request.plan_revision()
                && prearmed.phrase_index == request.phrase_index()
                && now.saturating_duration_since(prearmed.selected_at) >= BANK_SETTLE_DELAY
        });
        if is_safely_prearmed {
            if self
                .midi_output
                .trigger_autoloop_button(autoloop_number)
                .is_ok()
            {
                self.autoloop_emitted_count = self.autoloop_emitted_count.saturating_add(1);
                self.pending_autoloop = None;
            }
            self.prearmed_autoloop = None;
            return;
        }

        self.scheduled_prearm = None;
        if self.precise_autoloop_fallback {
            self.autoloop_late_count = self.autoloop_late_count.saturating_add(1);
        }
        if self.pending_autoloop.take().is_some() {
            self.autoloop_cancelled_count = self.autoloop_cancelled_count.saturating_add(1);
        }
        if self.midi_output.select_bank(bank_number).is_err() {
            return;
        }
        self.prearmed_autoloop = None;
        self.pending_autoloop = Some(PendingAutoloop {
            request: request.clone(),
            bank_number,
            autoloop_number,
            selected_at: now,
            requires_precise_beat: self.precise_autoloop_fallback,
        });
        self.cancel_stale_autoloop(state);
    }

    fn cancel_stale_autoloop(&mut self, state: &lumi_domain::RuntimeState) {
        let stale = self.pending_autoloop.as_ref().is_some_and(|pending| {
            !execution_context_is_current(state, &pending.request)
                || state
                    .deck(pending.request.deck_id())
                    .and_then(lumi_domain::DeckState::phrase_index)
                    != Some(pending.request.phrase_index())
        });
        if stale {
            self.pending_autoloop = None;
            self.autoloop_cancelled_count = self.autoloop_cancelled_count.saturating_add(1);
        }
    }

    fn service_pending_autoloop(&mut self, state: &lumi_domain::RuntimeState, precise_beat: bool) {
        self.service_scheduled_prearm(state);
        self.cancel_stale_autoloop(state);
        let ready = self.pending_autoloop.as_ref().is_some_and(|pending| {
            (!pending.requires_precise_beat || precise_beat)
                && pending.selected_at.elapsed() >= BANK_SETTLE_DELAY
        });
        if !ready {
            return;
        }
        let Some(pending) = self.pending_autoloop.take() else {
            return;
        };
        if self
            .midi_output
            .trigger_autoloop_button(pending.autoloop_number)
            .is_ok()
        {
            self.autoloop_emitted_count = self.autoloop_emitted_count.saturating_add(1);
            if pending.requires_precise_beat {
                self.autoloop_beat_fallback_count =
                    self.autoloop_beat_fallback_count.saturating_add(1);
            }
        }
    }

    fn service_scheduled_prearm(&mut self, state: &lumi_domain::RuntimeState) {
        let stale = self.scheduled_prearm.as_ref().is_some_and(|scheduled| {
            state.operation() != OperationState::Live
                || state.leader_deck() != Some(scheduled.deck_id)
                || state
                    .deck(scheduled.deck_id)
                    .is_none_or(|deck| deck.track_load_id() != scheduled.track_load_id)
                || state.active_plan().is_none_or(|plan| {
                    plan.track_load_id() != scheduled.track_load_id
                        || plan.revision() != scheduled.plan_revision
                })
        });
        if stale {
            self.scheduled_prearm = None;
            self.autoloop_cancelled_count = self.autoloop_cancelled_count.saturating_add(1);
            return;
        }
        let ready = self
            .scheduled_prearm
            .as_ref()
            .is_some_and(|scheduled| Instant::now() >= scheduled.due_at);
        if !ready {
            return;
        }
        let Some(scheduled) = self.scheduled_prearm.take() else {
            return;
        };
        if self.ensure_lighting_midi().is_err()
            || self.midi_output.status().state != MidiSourceState::Ready
            || self.midi_output.select_bank(scheduled.bank_number).is_err()
        {
            return;
        }
        self.prearmed_autoloop = Some(PrearmedAutoloop {
            bank_number: scheduled.bank_number,
            selected_at: Instant::now(),
            deck_id: scheduled.deck_id,
            track_load_id: scheduled.track_load_id,
            plan_revision: scheduled.plan_revision,
            phrase_index: scheduled.phrase_index,
        });
        self.autoloop_prearmed_count = self.autoloop_prearmed_count.saturating_add(1);
    }

    #[cfg(not(test))]
    fn on_precise_prolink_beat(
        &mut self,
        state: &lumi_domain::RuntimeState,
        observation: lumi_prolink_input::ProLinkTimingObservation,
    ) {
        self.service_pending_autoloop(state, true);
        if state.operation() != OperationState::Live
            || state.leader_deck() != Some(observation.deck_id)
            || !observation.playing
        {
            self.prearmed_autoloop = None;
            return;
        }
        let Some(plan) = state.active_plan() else {
            self.prearmed_autoloop = None;
            return;
        };
        let Some(cue) = plan
            .cues()
            .iter()
            .find(|cue| cue.start_beat() == observation.absolute_beat.saturating_add(1))
        else {
            return;
        };
        let Ok(Some((bank_number, _))) = automatic_midi_target(cue.action()) else {
            return;
        };
        self.scheduled_prearm = Some(ScheduledPrearm {
            bank_number,
            due_at: Instant::now() + prolink_prearm_delay(observation.effective_bpm_milli),
            deck_id: observation.deck_id,
            track_load_id: plan.track_load_id(),
            plan_revision: plan.revision(),
            phrase_index: cue.phrase_index(),
        });
    }

    fn prearm_current_or_first_cue(&mut self, state: &lumi_domain::RuntimeState) {
        if state.operation() != OperationState::Live {
            return;
        }
        let Some(deck_id) = state.leader_deck() else {
            return;
        };
        let Some(deck) = state.deck(deck_id) else {
            return;
        };
        let Some(plan) = state.active_plan() else {
            return;
        };
        let phrase_index = deck.phrase_index().unwrap_or(0);
        let Some(cue) = plan.cues().get(usize::from(phrase_index)) else {
            return;
        };
        let Ok(Some((bank_number, _))) = automatic_midi_target(cue.action()) else {
            return;
        };
        if self.ensure_lighting_midi().is_err()
            || self.midi_output.status().state != MidiSourceState::Ready
            || self.midi_output.select_bank(bank_number).is_err()
        {
            return;
        }
        self.prearmed_autoloop = Some(PrearmedAutoloop {
            bank_number,
            selected_at: Instant::now(),
            deck_id,
            track_load_id: plan.track_load_id(),
            plan_revision: plan.revision(),
            phrase_index,
        });
        self.scheduled_prearm = None;
        self.autoloop_prearmed_count = self.autoloop_prearmed_count.saturating_add(1);
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
                        if let Some((bank_number, autoloop_number)) =
                            automatic_midi_target(request.action())?
                        {
                            let _ = self.ensure_lighting_midi();
                            if self.midi_output.status().state == MidiSourceState::Ready {
                                // Hardware output fails closed and reports through Tech status;
                                // it must never stall the authoritative transport/runtime lane.
                                self.schedule_autoloop(
                                    runtime.state(),
                                    &request,
                                    bank_number,
                                    autoloop_number,
                                );
                            }
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

    fn midi_clock_status(&self) -> lumi_midi_output::MidiClockStatus {
        self.midi_clock.status()
    }

    fn link_timing_status(&self) -> lumi_timing_output::TimingOutputStatus {
        self.link_timing.status()
    }

    fn link_enabled(&self) -> bool {
        self.link_enabled
    }

    fn set_link_enabled(&mut self, enabled: bool) -> Result<(), EngineError> {
        if enabled == self.link_enabled {
            return Ok(());
        }
        if enabled {
            self.link_timing
                .publish()
                .map_err(|error| EngineError::Timing(error.to_string()))?;
            self.link_enabled = true;
        } else {
            self.link_timing
                .stop()
                .map_err(|error| EngineError::Timing(error.to_string()))?;
            self.link_enabled = false;
            self.last_prolink_timing_at = None;
            self.last_prolink_bpm_milli = None;
            self.last_prolink_playing = None;
            self.prolink_timing_stale = false;
        }
        Ok(())
    }

    fn midi_auto_publish_enabled(&self) -> bool {
        self.midi_auto_publish_enabled
    }

    fn timing_offset_millis(&self) -> i16 {
        self.timing_offset_millis
    }

    fn pending_timing_offset_millis(&self) -> Option<i16> {
        self.pending_timing_offset_millis
    }

    fn request_timing_offset_millis(&mut self, millis: i16, defer_until_phrase: bool) {
        let requested = millis.clamp(-250, 250);
        if defer_until_phrase && requested != self.timing_offset_millis {
            self.pending_timing_offset_millis = Some(requested);
        } else {
            self.timing_offset_millis = requested;
            self.pending_timing_offset_millis = None;
        }
    }

    fn activate_pending_timing_offset(&mut self) {
        if let Some(pending) = self.pending_timing_offset_millis.take() {
            self.timing_offset_millis = pending;
        }
    }

    #[cfg(not(test))]
    fn enable_midi_auto_publish(&mut self) -> Result<(), EngineError> {
        self.midi_auto_publish_enabled = true;
        self.last_midi_publish_attempt = Some(Instant::now());
        self.publish_midi()
    }

    fn test_link_helper(&mut self) -> Result<(), EngineError> {
        self.link_timing
            .self_test_helper()
            .map(|_| ())
            .map_err(|error| EngineError::Timing(error.to_string()))
    }

    fn ensure_lighting_midi(&mut self) -> Result<(), EngineError> {
        if self.midi_auto_publish_enabled
            && self.midi_output.status().state != MidiSourceState::Ready
        {
            let now = Instant::now();
            if self.last_midi_publish_attempt.is_some_and(|attempt| {
                now.saturating_duration_since(attempt) < Duration::from_secs(1)
            }) {
                return Ok(());
            }
            self.last_midi_publish_attempt = Some(now);
            self.midi_output
                .publish()
                .map_err(|error| EngineError::Midi(error.to_string()))?;
        }
        Ok(())
    }

    fn publish_midi(&mut self) -> Result<(), EngineError> {
        self.midi_auto_publish_enabled = true;
        self.last_midi_publish_attempt = Some(Instant::now());
        self.midi_output
            .publish()
            .map_err(|error| EngineError::Midi(error.to_string()))?;
        if let Err(error) = self.midi_clock.publish() {
            return Err(EngineError::Midi(error.to_string()));
        }
        Ok(())
    }

    fn stop_midi(&mut self) -> Result<(), EngineError> {
        self.midi_auto_publish_enabled = false;
        self.last_midi_publish_attempt = None;
        self.midi_output.stop();
        self.midi_clock
            .stop()
            .map_err(|error| EngineError::Midi(error.to_string()))
    }

    #[cfg(not(test))]
    fn synchronize_prolink_timing(
        &mut self,
        observation: lumi_prolink_input::ProLinkTimingObservation,
        operation: OperationState,
    ) -> Result<(), EngineError> {
        if !self.link_enabled {
            return Ok(());
        }
        self.last_prolink_timing_at = Some(Instant::now());
        self.last_prolink_bpm_milli = Some(observation.effective_bpm_milli);
        self.last_prolink_playing = Some(observation.playing);
        self.prolink_timing_stale = false;
        self.link_timing
            .synchronize(prolink_timing_anchor(observation, operation))
            .map_err(|error| EngineError::Timing(error.to_string()))
    }

    #[cfg(not(test))]
    fn reconcile_prolink_timing_freshness(
        &mut self,
        source_status: DeckSourceStatus,
    ) -> Result<(), EngineError> {
        if !self.link_enabled || self.prolink_timing_stale {
            return Ok(());
        }
        let Some(last_timing_at) = self.last_prolink_timing_at else {
            return Ok(());
        };
        let source_unavailable = !matches!(source_status, DeckSourceStatus::Ready);
        // A stopped master intentionally emits no beat packets. Its last
        // authoritative hold remains valid while the bridge itself is healthy;
        // otherwise every normal pause would be misreported as stale timing.
        if !source_unavailable && self.last_prolink_playing == Some(false) {
            return Ok(());
        }
        let stale_after = prolink_timing_stale_after(self.last_prolink_bpm_milli);
        let age = last_timing_at.elapsed();
        if !source_unavailable && age <= stale_after {
            return Ok(());
        }
        let reason = if source_unavailable {
            format!(
                "Pro DJ Link source is {}; Link transport was held fail-closed",
                deck_source_status_name(source_status)
            )
        } else {
            format!(
                "Pro DJ Link timing is stale ({} ms without an anchor); Link transport was held fail-closed",
                age.as_millis()
            )
        };
        self.fail_prolink_timing(reason)
    }

    #[cfg(not(test))]
    fn fail_prolink_timing(&mut self, reason: impl Into<String>) -> Result<(), EngineError> {
        if !self.link_enabled || self.prolink_timing_stale {
            return Ok(());
        }
        self.link_timing
            .fail_closed(reason)
            .map_err(|error| EngineError::Timing(error.to_string()))?;
        self.prolink_timing_stale = true;
        Ok(())
    }

    fn synchronize_local_link_timing(
        &mut self,
        deck_id: lumi_domain::DeckId,
        anchor: crate::library::LocalPlaybackClockAnchor,
        playing: bool,
        force_rephase: bool,
    ) -> Result<(), EngineError> {
        if !self.link_enabled {
            return Ok(());
        }
        self.last_prolink_timing_at = None;
        self.last_prolink_bpm_milli = None;
        self.last_prolink_playing = None;
        self.prolink_timing_stale = false;
        if force_rephase {
            self.local_timing_generation =
                self.local_timing_generation.checked_add(1).ok_or_else(|| {
                    EngineError::Timing("local timing generation overflow".to_owned())
                })?;
        }
        self.link_timing
            .synchronize(TimingAnchor {
                source: TimingSourceKind::LocalPlayback,
                deck_number: Some(deck_id.value()),
                bpm_milli: anchor.bpm_milli,
                beat_within_bar: u8::try_from(anchor.beat_index % 4)
                    .unwrap_or(0)
                    .saturating_add(1),
                playing,
                generation: self.local_timing_generation,
                discontinuity: if force_rephase {
                    TimingDiscontinuity::Seeked
                } else {
                    TimingDiscontinuity::Continuous
                },
                observed_at_micros: None,
            })
            .map_err(|error| EngineError::Timing(error.to_string()))
    }
}

fn prolink_timing_stale_after(bpm_milli: Option<u32>) -> Duration {
    let three_beats = bpm_milli
        .filter(|bpm| *bpm > 0)
        .map(|bpm| Duration::from_micros(180_000_000_000_u64 / u64::from(bpm)))
        .unwrap_or(MINIMUM_PROLINK_TIMING_STALE_AFTER);
    three_beats.clamp(
        MINIMUM_PROLINK_TIMING_STALE_AFTER,
        MAXIMUM_PROLINK_TIMING_STALE_AFTER,
    )
}

fn prolink_prearm_delay(bpm_milli: u32) -> Duration {
    let beat_duration = Duration::from_micros(60_000_000_000_u64 / u64::from(bpm_milli.max(1)));
    beat_duration.saturating_sub(BANK_SETTLE_DELAY.saturating_add(INTEGRATION_PUMP_INTERVAL))
}

fn position_with_timing_offset(position_millis: u64, offset_millis: i32) -> u64 {
    if offset_millis >= 0 {
        position_millis.saturating_add(u64::from(offset_millis.unsigned_abs()))
    } else {
        position_millis.saturating_sub(u64::from(offset_millis.unsigned_abs()))
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

fn local_transport_is_discontinuous(
    previous: Option<LocalTransportObservation>,
    track_load_id: TrackLoadId,
    position_millis: u64,
    playing: bool,
    now: Instant,
) -> bool {
    let Some(previous) = previous else {
        return playing;
    };
    if previous.track_load_id != track_load_id || (!previous.playing && playing) {
        return true;
    }
    if !previous.playing || !playing {
        return false;
    }
    let expected_delta = i128::try_from(
        now.saturating_duration_since(previous.observed_at)
            .as_millis(),
    )
    .unwrap_or(i128::MAX);
    let actual_delta = i128::from(position_millis) - i128::from(previous.position_millis);
    actual_delta.saturating_sub(expected_delta).abs() > 250
}

fn reconcile_local_midi_clock(
    runtime: &mut EngineRuntime,
    force_rephase: bool,
) -> Result<(), EngineError> {
    let mut link_sync = None;
    let sync = if runtime.deck_source_mode == DeckSourceMode::LocalPlayback {
        runtime.state.state().leader_deck().and_then(|deck_id| {
            let deck = runtime.state.state().deck(deck_id)?;
            let transport = runtime.local_transports.get(&deck_id)?;
            if transport.track_load_id != deck.track_load_id() {
                return None;
            }
            let context = runtime
                .planning_worker
                .library_context(deck.track_load_id())?;
            let anchor = context
                .clock_anchor_at_millis(transport.position_millis, deck.effective_bpm_milli())?;
            let playing = !matches!(
                runtime.state.state().operation(),
                OperationState::Off | OperationState::Paused
            ) && deck.is_playing()
                && transport.playing;
            link_sync = Some((deck_id, anchor, playing));
            Some(MidiClockSync {
                bpm_milli: anchor.bpm_milli,
                playing: runtime.state.state().operation() == OperationState::Live && playing,
                song_position_16th: anchor.song_position_16th,
                delay_to_next_tick: anchor.delay_to_next_tick,
                rephase: force_rephase,
            })
        })
    } else {
        None
    }
    .unwrap_or(MidiClockSync {
        bpm_milli: 120_000,
        playing: false,
        song_position_16th: 0,
        delay_to_next_tick: Duration::ZERO,
        rephase: false,
    });
    runtime
        .output_worker
        .midi_clock
        .synchronize(sync)
        .map_err(|error| EngineError::Midi(error.to_string()))?;
    if let Some((deck_id, anchor, playing)) = link_sync {
        runtime.output_worker.synchronize_local_link_timing(
            deck_id,
            anchor,
            playing,
            force_rephase,
        )?;
    }
    Ok(())
}

fn prolink_timing_anchor(
    observation: lumi_prolink_input::ProLinkTimingObservation,
    operation: OperationState,
) -> TimingAnchor {
    TimingAnchor {
        source: TimingSourceKind::ProDjLink,
        deck_number: Some(observation.deck_id.value()),
        bpm_milli: observation.effective_bpm_milli,
        beat_within_bar: observation.beat_within_bar,
        // Lumi's lighting gate owns transport, not tempo. Off and Pause keep
        // Link stopped while still publishing the selected master's current
        // BPM and phase so a peer such as SoundSwitch never falls back to an
        // unrelated/default tempo.
        playing: observation.playing
            && !matches!(operation, OperationState::Off | OperationState::Paused),
        generation: observation.generation,
        discontinuity: if observation.discontinuity {
            TimingDiscontinuity::MasterChanged
        } else {
            TimingDiscontinuity::Continuous
        },
        observed_at_micros: Some(observation.observed_at_nanos / 1_000),
    }
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
    process_deck_input_messages(runtime)?;
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
    let is_transport_update = matches!(
        &command,
        SessionCommand::UpdateLocalPlaybackTransport { .. }
    );
    let waveform_detail_track_id = match &command {
        SessionCommand::GetLibraryTrackWaveform { track_id } => Some(*track_id),
        _ => None,
    };
    let includes_device_inspection = matches!(
        &command,
        SessionCommand::InspectRekordboxDevice { .. } | SessionCommand::SyncRekordboxDevice { .. }
    );
    if is_mutating && command_ids.contains(&envelope.message_id) {
        if is_transport_update {
            return transport_ack_envelope(runtime, response_sequence, &envelope.message_id);
        }
        return if includes_device_inspection {
            snapshot_envelope_with_device_inspection(
                runtime,
                response_sequence,
                &envelope.message_id,
            )
        } else {
            snapshot_envelope(runtime, response_sequence, &envelope.message_id)
        };
    }

    if let Err(error) = apply_command(runtime, command) {
        return application_error_envelope(response_sequence, &envelope.message_id, &error);
    }
    if is_mutating {
        let disposition = command_ids.observe(&envelope.message_id);
        debug_assert_eq!(disposition, CommandDisposition::FirstSeen);
    }
    if is_transport_update {
        return transport_ack_envelope(runtime, response_sequence, &envelope.message_id);
    }
    let mut response = if includes_device_inspection {
        snapshot_envelope_with_device_inspection(runtime, response_sequence, &envelope.message_id)?
    } else {
        snapshot_envelope(runtime, response_sequence, &envelope.message_id)?
    };
    if let Some(track_id) = waveform_detail_track_id {
        response.payload.insert(
            "waveformDetail".to_owned(),
            runtime.library_worker.waveform_detail_json(track_id)?,
        );
    }
    Ok(response)
}

fn transport_ack_envelope(
    runtime: &EngineRuntime,
    sequence: u64,
    correlation_id: &str,
) -> Result<MessageEnvelope, EngineError> {
    let mut payload = Map::new();
    payload.insert(
        "kind".to_owned(),
        Value::String("localPlaybackTransportAccepted".to_owned()),
    );
    payload.insert(
        "stateRevision".to_owned(),
        json!(runtime.state.state().revision().value()),
    );
    Ok(MessageEnvelope {
        protocol_version: PROTOCOL_VERSION,
        message_type: MessageType::Event,
        message_id: format!("event-{sequence}"),
        sequence,
        correlation_id: correlation_id.to_owned(),
        sent_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        payload,
    })
}

fn process_deck_input_messages(runtime: &mut EngineRuntime) -> Result<(), EngineError> {
    runtime.output_worker.set_precise_autoloop_fallback(
        runtime.deck_source_mode == DeckSourceMode::ConnectedDecks && runtime.uses_direct_prolink(),
    );
    let messages = runtime.deck_input.drain_messages();
    if runtime.deck_source_mode != DeckSourceMode::ConnectedDecks {
        #[cfg(not(test))]
        maintain_direct_prolink_bridge(runtime)?;
        return Ok(());
    }
    let at = runtime.clock.now();
    #[cfg(not(test))]
    maintain_direct_prolink_bridge(runtime)?;
    if !runtime.uses_direct_prolink() {
        #[cfg(not(test))]
        if runtime.prolink_recovery_pending {
            process_pending_source_events(runtime)?;
            return Ok(());
        }
        for message in messages {
            runtime.connected_deck_source.ingest(message, at)?;
        }
        runtime.connected_deck_source.expire_stale(
            Instant::now(),
            Duration::from_millis(2_500),
            at,
        )?;
    }
    process_pending_source_events(runtime)?;
    Ok(())
}

#[cfg(not(test))]
fn maintain_direct_prolink_bridge(runtime: &mut EngineRuntime) -> Result<(), EngineError> {
    if runtime.prolink_recovery_pending && runtime.prolink_bridge.is_none() {
        let retry_due = runtime
            .last_prolink_restart_attempt
            .is_none_or(|last_attempt| last_attempt.elapsed() >= Duration::from_secs(1));
        if retry_due {
            runtime.last_prolink_restart_attempt = Some(Instant::now());
            let (bridge, error) = launch_prolink_bridge();
            runtime.prolink_start_error = error;
            if let Some(bridge) = bridge {
                runtime.prolink_bridge = Some(bridge);
                runtime.direct_deck_source = ProLinkDeckSourceProvider::new(runtime.clock.now())?;
                runtime.prolink_recovery_pending = false;
                runtime.prolink_restart_count = runtime.prolink_restart_count.saturating_add(1);
                runtime.prolink_start_error = None;
            }
        }
    }

    let Some(bridge) = runtime.prolink_bridge.as_mut() else {
        return Ok(());
    };
    let messages = match bridge.drain_messages() {
        Ok(messages) => messages,
        Err(error) => {
            fail_direct_prolink_bridge(runtime, error.to_string())?;
            return Ok(());
        }
    };
    let Some(bridge) = runtime.prolink_bridge.as_mut() else {
        return Ok(());
    };
    let bridge_running = match bridge.diagnostics() {
        Ok(diagnostics) => diagnostics.running,
        Err(error) => {
            fail_direct_prolink_bridge(runtime, error.to_string())?;
            return Ok(());
        }
    };
    if !bridge_running {
        fail_direct_prolink_bridge(
            runtime,
            "Pro DJ Link bridge stopped unexpectedly".to_owned(),
        )?;
        return Ok(());
    }

    let at = runtime.clock.now();
    for message in messages {
        if let Err(error) = runtime.direct_deck_source.ingest(message, at) {
            fail_direct_prolink_bridge(runtime, error.to_string())?;
            return Ok(());
        }
    }
    // Domain events must reach planning/output before their matching precise
    // beat is consumed. This lets a hotcue seek replace a stale trigger and
    // then use that same beat observation as its safe beat-aligned fallback.
    process_pending_source_events(runtime)?;
    let operation = runtime.state.state().operation();
    for observation in runtime.direct_deck_source.drain_timing_observations() {
        runtime
            .output_worker
            .synchronize_prolink_timing(observation, operation)?;
        runtime
            .output_worker
            .on_precise_prolink_beat(runtime.state.state(), observation);
    }
    runtime.output_worker.reconcile_prolink_timing_freshness(
        runtime.direct_deck_source.diagnostics().source_status,
    )?;
    Ok(())
}

#[cfg(not(test))]
fn fail_direct_prolink_bridge(
    runtime: &mut EngineRuntime,
    message: String,
) -> Result<(), EngineError> {
    let actionable = format!("{message}; Lumi will retry the Pro DJ Link bridge automatically");
    runtime.prolink_start_error = Some(actionable.clone());
    runtime
        .output_worker
        .fail_prolink_timing(actionable.clone())?;
    let at = runtime.clock.now();
    runtime.direct_deck_source.mark_degraded(actionable, at)?;
    runtime.direct_deck_source.clear(at)?;
    runtime.prolink_bridge.take();
    runtime.prolink_recovery_pending = true;
    runtime.last_prolink_restart_attempt = Some(Instant::now());
    Ok(())
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
        SessionCommand::GetLibraryTrackWaveform { .. } => return Ok(()),
        SessionCommand::CloseLibraryTrackEditor => {
            runtime.library_worker.close_editor();
            return Ok(());
        }
        SessionCommand::PreviewDemoSourceRefresh => {
            runtime.library_worker.preview_demo_source_refresh()?;
            return Ok(());
        }
        SessionCommand::PreviewRekordboxXmlSync {
            folder,
            followed_paths,
            include_future_child_playlists,
        } => {
            runtime.library_worker.preview_rekordbox_xml_sync(
                folder,
                followed_paths,
                include_future_child_playlists,
            )?;
            return Ok(());
        }
        SessionCommand::ApplyRekordboxXmlSync {
            folder,
            followed_paths,
            include_future_child_playlists,
            expected_content_sha256,
        } => {
            runtime.library_worker.apply_rekordbox_xml_sync(
                folder,
                followed_paths,
                include_future_child_playlists,
                &expected_content_sha256,
            )?;
            return Ok(());
        }
        SessionCommand::ImportRekordboxAnalysis {
            folder,
            followed_paths,
            include_future_child_playlists,
            expected_content_sha256,
        } => {
            runtime.library_worker.import_rekordbox_analysis(
                folder,
                followed_paths,
                include_future_child_playlists,
                &expected_content_sha256,
            )?;
            return Ok(());
        }
        SessionCommand::InspectRekordboxDevice { root, source_id } => {
            runtime
                .library_worker
                .inspect_rekordbox_device(root, source_id.as_deref())?;
            return Ok(());
        }
        SessionCommand::SyncRekordboxDevice {
            root,
            source_id,
            playlist_ids,
        } => {
            runtime.library_worker.sync_rekordbox_device(
                root,
                source_id.as_deref(),
                &playlist_ids,
            )?;
            return Ok(());
        }
        SessionCommand::PreviewLibraryReset { preserve_track_ids } => {
            if runtime.state.state().operation() != OperationState::Off {
                return Err(CommandApplicationError::DataManagementRequiresOff);
            }
            runtime
                .library_worker
                .preview_library_reset(&preserve_track_ids)?;
            return Ok(());
        }
        SessionCommand::ApplyLibraryReset {
            expected_token,
            backup_database_path,
        } => {
            if runtime.state.state().operation() != OperationState::Off {
                return Err(CommandApplicationError::DataManagementRequiresOff);
            }
            runtime
                .library_worker
                .apply_library_reset(&expected_token, &backup_database_path)?;
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
            let catalog = runtime.library_worker.autoloop_catalog()?;
            runtime.planning_worker.synchronize_themes(&catalog);
            return Ok(());
        }
        SessionCommand::PublishMidiSource => {
            runtime
                .output_worker
                .publish_midi()
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            reconcile_local_midi_clock(runtime, true)
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            return Ok(());
        }
        SessionCommand::StopMidiSource => {
            runtime
                .output_worker
                .stop_midi()
                .map_err(|error| CommandApplicationError::Midi(error.to_string()))?;
            return Ok(());
        }
        SessionCommand::SetAbletonLinkEnabled { enabled } => {
            runtime
                .output_worker
                .set_link_enabled(enabled)
                .map_err(CommandApplicationError::Engine)?;
            if enabled && runtime.deck_source_mode == DeckSourceMode::LocalPlayback {
                reconcile_local_midi_clock(runtime, true)
                    .map_err(CommandApplicationError::Engine)?;
            }
            return Ok(());
        }
        SessionCommand::TestAbletonLinkHelper => {
            if runtime.state.state().operation() != OperationState::Off
                || runtime.output_worker.link_enabled()
            {
                return Err(CommandApplicationError::TimingTestRequiresOff);
            }
            runtime
                .output_worker
                .test_link_helper()
                .map_err(CommandApplicationError::Engine)?;
            return Ok(());
        }
        SessionCommand::SetOutputTimingOffset { millis } => {
            let defer_until_phrase = runtime.state.state().operation() == OperationState::Live
                && runtime
                    .leader_deck_id()
                    .and_then(|deck_id| runtime.state.state().deck(deck_id))
                    .is_some_and(lumi_domain::DeckState::is_playing);
            runtime
                .output_worker
                .request_timing_offset_millis(millis, defer_until_phrase);
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
            runtime.local_transports.insert(
                deck_id,
                LocalTransportObservation {
                    track_load_id,
                    position_millis: 0,
                    playing: false,
                    observed_at: Instant::now(),
                },
            );
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            reconcile_local_midi_clock(runtime, false).map_err(CommandApplicationError::Engine)?;
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
            let _ = runtime.output_worker.ensure_lighting_midi();
            let output_position_millis = if playing {
                position_with_timing_offset(
                    position_millis,
                    i32::from(runtime.output_worker.timing_offset_millis())
                        + i32::try_from(BANK_SETTLE_DELAY.as_millis()).unwrap_or(50),
                )
            } else {
                position_millis
            };
            let beat = runtime
                .planning_worker
                .library_context(track_load_id)
                .ok_or(CommandApplicationError::TrackLoadMismatch)?
                .beat_at_millis(output_position_millis);
            let at = runtime
                .clock
                .advance(1)
                .ok_or(CommandApplicationError::ClockOverflow)?;
            let observed_at = Instant::now();
            let rephase = local_transport_is_discontinuous(
                runtime.local_transports.get(&deck_id).copied(),
                track_load_id,
                position_millis,
                playing,
                observed_at,
            );
            runtime.local_deck_source.update_transport(
                deck_id,
                track_load_id,
                beat,
                playing,
                at,
            )?;
            runtime.local_transports.insert(
                deck_id,
                LocalTransportObservation {
                    track_load_id,
                    position_millis,
                    playing,
                    observed_at,
                },
            );
            process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
            let leader_rephase = rephase && runtime.state.state().leader_deck() == Some(deck_id);
            reconcile_local_midi_clock(runtime, leader_rephase)
                .map_err(CommandApplicationError::Engine)?;
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
            reconcile_local_midi_clock(runtime, true).map_err(CommandApplicationError::Engine)?;
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
                #[cfg(not(test))]
                let pending_prolink_bridge = if target == DeckSourceMode::ConnectedDecks
                    && !runtime.direct_prolink_active()
                {
                    let (bridge, start_error) = launch_prolink_bridge();
                    runtime.prolink_start_error = start_error;
                    let Some(bridge) = bridge else {
                        return Err(CommandApplicationError::ProLinkUnavailable(
                            runtime.prolink_start_error.clone().unwrap_or_else(|| {
                                "Pro DJ Link could not be started safely.".to_owned()
                            }),
                        ));
                    };
                    Some(bridge)
                } else {
                    None
                };
                if runtime.deck_source_mode == DeckSourceMode::LocalPlayback {
                    let at = runtime
                        .clock
                        .advance(1)
                        .ok_or(CommandApplicationError::ClockOverflow)?;
                    runtime.local_deck_source.clear(at)?;
                    runtime.local_transports.clear();
                    process_pending_source_events(runtime)
                        .map_err(CommandApplicationError::Engine)?;
                } else if runtime.deck_source_mode == DeckSourceMode::ConnectedDecks {
                    let at = runtime
                        .clock
                        .advance(1)
                        .ok_or(CommandApplicationError::ClockOverflow)?;
                    if runtime.uses_direct_prolink() {
                        runtime.direct_deck_source.clear(at).map_err(|error| {
                            CommandApplicationError::Engine(EngineError::ProLinkProvider(error))
                        })?;
                    } else {
                        runtime.connected_deck_source.clear(at)?;
                    }
                    process_pending_source_events(runtime)
                        .map_err(CommandApplicationError::Engine)?;
                    #[cfg(not(test))]
                    {
                        runtime.prolink_bridge.take();
                        runtime.prolink_start_error = None;
                        runtime.prolink_recovery_pending = false;
                        runtime.last_prolink_restart_attempt = None;
                    }
                }
                #[cfg(not(test))]
                if let Some(bridge) = pending_prolink_bridge {
                    runtime.prolink_bridge = Some(bridge);
                    runtime.prolink_recovery_pending = false;
                    runtime.last_prolink_restart_attempt = None;
                }
                runtime.deck_source_mode = target;
                process_pending_source_events(runtime).map_err(CommandApplicationError::Engine)?;
                reconcile_local_midi_clock(runtime, true)
                    .map_err(CommandApplicationError::Engine)?;
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
            if matches!(command, OperationCommand::Off | OperationCommand::Pause)
                && runtime.output_worker.link_enabled()
            {
                runtime.output_worker.link_timing.hold().map_err(|error| {
                    CommandApplicationError::Engine(EngineError::Timing(error.to_string()))
                })?;
            }
            reconcile_local_midi_clock(runtime, true).map_err(CommandApplicationError::Engine)?;
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
        | SessionCommand::GetLibraryTrackWaveform { .. }
        | SessionCommand::CloseLibraryTrackEditor
        | SessionCommand::PreviewDemoSourceRefresh
        | SessionCommand::PreviewRekordboxXmlSync { .. }
        | SessionCommand::ApplyRekordboxXmlSync { .. }
        | SessionCommand::ImportRekordboxAnalysis { .. }
        | SessionCommand::InspectRekordboxDevice { .. }
        | SessionCommand::SyncRekordboxDevice { .. }
        | SessionCommand::PreviewLibraryReset { .. }
        | SessionCommand::ApplyLibraryReset { .. }
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
        | SessionCommand::SetAbletonLinkEnabled { .. }
        | SessionCommand::TestAbletonLinkHelper
        | SessionCommand::SetOutputTimingOffset { .. }
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
    .map_err(CommandApplicationError::Engine)?;
    if command == OperationCommand::Start {
        runtime
            .output_worker
            .prearm_current_or_first_cue(runtime.state.state());
    }
    Ok(())
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
        CommandApplicationError::DataManagementRequiresOff => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "dataManagementRequiresOff",
            "Set Lumi to Off before creating, restoring, or resetting library data.",
            false,
            None,
        ),
        CommandApplicationError::TimingTestRequiresOff => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "timingTestRequiresOff",
            "Set Lumi to Off and disable Ableton Link before testing the helper.",
            false,
            None,
        ),
        CommandApplicationError::ProLinkUnavailable(message) => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "proDjLinkUnavailable",
            message,
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
        CommandApplicationError::Library(library_error @ LibraryWorkerError::RekordboxXml(_)) => {
            error_envelope(
                sequence,
                correlation_id,
                "validationFailed",
                "rekordboxXmlPreviewRejected",
                &library_error.to_string(),
                false,
                None,
            )
        }
        CommandApplicationError::Library(
            library_error @ (LibraryWorkerError::RekordboxDevice(_)
            | LibraryWorkerError::InvalidRekordboxDeviceRoot),
        ) => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "rekordboxDeviceSyncRejected",
            &library_error.to_string(),
            false,
            None,
        ),
        CommandApplicationError::Library(
            library_error @ (LibraryWorkerError::RekordboxResolver(_)
            | LibraryWorkerError::RekordboxAnalysis(_)
            | LibraryWorkerError::RekordboxInstallationUnavailable
            | LibraryWorkerError::RekordboxPreviewChanged
            | LibraryWorkerError::IncompleteRekordboxResolution { .. }
            | LibraryWorkerError::IncompleteRekordboxAnalysis { .. }
            | LibraryWorkerError::MissingRekordboxTrackAnalysis(_)
            | LibraryWorkerError::InvalidRekordboxMetadata { .. }
            | LibraryWorkerError::IncompleteRekordboxBeatGrid
            | LibraryWorkerError::IncompleteRekordboxPhrases
            | LibraryWorkerError::InvalidRekordboxBeatGrid(_)
            | LibraryWorkerError::InvalidRekordboxTrack(_)
            | LibraryWorkerError::InvalidRekordboxBaseline(_)),
        ) => error_envelope(
            sequence,
            correlation_id,
            "validationFailed",
            "rekordboxAnalysisImportRejected",
            &library_error.to_string(),
            false,
            None,
        ),
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
        | CommandApplicationError::BltMidi(_)
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
    #[error("Beat Link Trigger MIDI input failed: {0}")]
    BltMidi(#[from] BltMidiError),
    #[error("the command is not valid for the active deck source")]
    WrongDeckSourceMode,
    #[error("Pro DJ Link is unavailable: {0}")]
    ProLinkUnavailable(String),
    #[error("library backup and reset operations require Lumi to be Off")]
    DataManagementRequiresOff,
    #[error("Ableton Link helper testing requires Lumi to be Off with Link disabled")]
    TimingTestRequiresOff,
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
    snapshot_envelope_internal(runtime, sequence, correlation_id, false)
}

fn snapshot_envelope_with_device_inspection(
    runtime: &EngineRuntime,
    sequence: u64,
    correlation_id: &str,
) -> Result<MessageEnvelope, EngineError> {
    snapshot_envelope_internal(runtime, sequence, correlation_id, true)
}

fn snapshot_envelope_internal(
    runtime: &EngineRuntime,
    sequence: u64,
    correlation_id: &str,
    include_device_inspection: bool,
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
                if runtime.uses_direct_prolink() {
                    deck_source_status_name(runtime.direct_deck_source.diagnostics().source_status)
                } else if runtime.connected_deck_source.diagnostics().committed_frame_count > 0 {
                    "ready"
                } else {
                    "disconnected"
                }
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
            "lastError": midi_output.last_error,
            "activeBank": midi_output.active_bank,
            "autoPublishEnabled": runtime.output_worker.midi_auto_publish_enabled(),
            "timingOffsetMillis": runtime.output_worker.timing_offset_millis(),
            "pendingTimingOffsetMillis": runtime.output_worker.pending_timing_offset_millis(),
            "bankPreRollMillis": BANK_SETTLE_DELAY.as_millis(),
            "realtimeScheduler": {
                "mode": if runtime.output_worker.precise_autoloop_fallback {
                    "preciseBeatFallback"
                } else {
                    "settleDeadline"
                },
                "prearmScheduled": runtime.output_worker.scheduled_prearm.as_ref().map(|scheduled| json!({
                    "deckNumber": scheduled.deck_id.value(),
                    "phraseIndex": scheduled.phrase_index,
                    "bankNumber": scheduled.bank_number,
                })),
                "pending": runtime.output_worker.pending_autoloop.as_ref().map(|pending| json!({
                    "deckNumber": pending.request.deck_id().value(),
                    "phraseIndex": pending.request.phrase_index(),
                    "bankNumber": pending.bank_number,
                    "autoloopNumber": pending.autoloop_number,
                    "requiresPreciseBeat": pending.requires_precise_beat,
                })),
                "requestedCount": runtime.output_worker.autoloop_requested_count,
                "prearmedCount": runtime.output_worker.autoloop_prearmed_count,
                "emittedCount": runtime.output_worker.autoloop_emitted_count,
                "cancelledCount": runtime.output_worker.autoloop_cancelled_count,
                "lateCount": runtime.output_worker.autoloop_late_count,
                "beatFallbackCount": runtime.output_worker.autoloop_beat_fallback_count,
            },
        }),
    );
    let midi_clock = runtime.output_worker.midi_clock_status();
    payload.insert(
        "midiClockIntegration".to_owned(),
        json!({
            "state": match midi_clock.state {
                MidiClockState::Stopped => "stopped",
                MidiClockState::Ready => "ready",
                MidiClockState::Running => "running",
            },
            "sourceName": midi_clock.source_name,
            "protocol": "MIDI Clock · 24 PPQN",
            "bpmMilli": midi_clock.bpm_milli,
            "sentTickCount": midi_clock.sent_tick_count,
            "sentTransportCount": midi_clock.sent_transport_count,
            "lastEvent": midi_clock.last_event,
            "lastError": midi_clock.last_error,
        }),
    );
    let link_timing = runtime.output_worker.link_timing_status();
    payload.insert(
        "abletonLinkIntegration".to_owned(),
        json!({
            "enabled": runtime.output_worker.link_enabled(),
            "state": match link_timing.state {
                TimingOutputState::Stopped => "stopped",
                TimingOutputState::Starting => "starting",
                TimingOutputState::Ready => "ready",
                TimingOutputState::Running => "running",
                TimingOutputState::Degraded => "degraded",
            },
            "provider": link_timing.provider,
            "helperVersion": link_timing.helper_version,
            "peers": link_timing.peers,
            "source": link_timing.source.map(|source| match source {
                TimingSourceKind::LocalPlayback => "localPlayback",
                TimingSourceKind::ProDjLink => "proDjLink",
            }),
            "deckNumber": link_timing.deck_number,
            "bpmMilli": link_timing.bpm_milli,
            "beatWithinBar": link_timing.beat_within_bar,
            "playing": link_timing.playing,
            "generation": link_timing.generation,
            "lastBeatAgeMillis": link_timing.last_anchor_age_millis,
            "phaseErrorMicros": link_timing.phase_error_micros,
            "receivedAnchorCount": link_timing.received_anchor_count,
            "appliedAnchorCount": link_timing.applied_anchor_count,
            "coalescedAnchorCount": link_timing.coalesced_anchor_count,
            "hardReanchorCount": link_timing.hard_reanchor_count,
            "softCorrectionCount": link_timing.soft_correction_count,
            "failClosedCount": link_timing.fail_closed_count,
            "failureCount": link_timing.failure_count,
            "maxAbsPhaseErrorMicros": link_timing.max_abs_phase_error_micros,
            "enginePumpCount": runtime.integration_pump_metrics.tick_count,
            "enginePumpStarvationCount": runtime.integration_pump_metrics.starvation_count,
            "enginePumpMaxLatenessMicros": runtime.integration_pump_metrics.max_lateness_micros,
            "lastReanchor": link_timing.last_reanchor.map(|reason| match reason {
                TimingDiscontinuity::Continuous => "continuous",
                TimingDiscontinuity::Started => "started",
                TimingDiscontinuity::Resumed => "resumed",
                TimingDiscontinuity::Seeked => "seeked",
                TimingDiscontinuity::TrackChanged => "trackChanged",
                TimingDiscontinuity::MasterChanged => "masterChanged",
            }),
            "lastEvent": link_timing.last_event,
            "lastError": link_timing.last_error,
        }),
    );
    if runtime.uses_direct_prolink() {
        let diagnostics = runtime.direct_deck_source.diagnostics();
        payload.insert(
            "deckInputIntegration".to_owned(),
            json!({
                "state": if diagnostics.source_status == DeckSourceStatus::Ready {
                    "ready"
                } else {
                    "stopped"
                },
                "sourceState": deck_source_status_name(diagnostics.source_status),
                "destinationName": Value::Null,
                "protocol": lumi_prolink_input::PROTOCOL_NAME,
                "protocolVersion": lumi_prolink_input::PROTOCOL_VERSION,
                "receivedMessageCount": diagnostics.received_message_count,
                "invalidWordCount": 0,
                "lastMessage": Value::Null,
                "committedFrameCount": diagnostics.received_message_count,
                "ignoredMessageCount": diagnostics.ignored_message_count,
                "duplicateFrameCount": 0,
                "lastDeckId": runtime.direct_deck_source.leader_deck_id()
                    .map(lumi_domain::DeckId::value),
                "lastFrameSequence": diagnostics.last_bridge_sequence,
                "bridgeVersion": diagnostics.bridge_version,
                "beatLinkVersion": diagnostics.beat_link_version,
                "recoveryPending": runtime.prolink_recovery_pending(),
                "restartCount": runtime.prolink_restart_count(),
                "discoveredPlayers": diagnostics.discovered_devices.iter()
                    .map(|(number, device)| json!({
                        "playerNumber": number,
                        "name": device.name,
                        "address": device.address,
                    }))
                    .collect::<Vec<_>>(),
                "lastError": diagnostics.last_error
                    .or_else(|| runtime.prolink_start_error.clone()),
            }),
        );
    } else {
        let deck_input = runtime.deck_input.status();
        let blt_diagnostics = runtime.connected_deck_source.diagnostics();
        payload.insert(
            "deckInputIntegration".to_owned(),
            json!({
                "state": match deck_input.state {
                    MidiDestinationState::Stopped => "stopped",
                    MidiDestinationState::Ready => "ready",
                },
                "destinationName": deck_input.destination_name,
                "protocol": lumi_blt_midi::PROTOCOL_NAME,
                "protocolVersion": lumi_blt_midi::PROTOCOL_VERSION,
                "receivedMessageCount": deck_input.received_message_count,
                "invalidWordCount": deck_input.invalid_word_count,
                "lastMessage": deck_input.last_message.map(|message| json!({
                    "status": message.status,
                    "channel": message.channel,
                    "dataOne": message.data_one,
                    "dataTwo": message.data_two,
                })),
                "committedFrameCount": blt_diagnostics.committed_frame_count,
                "ignoredMessageCount": blt_diagnostics.ignored_message_count,
                "duplicateFrameCount": blt_diagnostics.duplicate_frame_count,
                "lastDeckId": blt_diagnostics.last_deck_id.map(lumi_domain::DeckId::value),
                "lastFrameSequence": blt_diagnostics.last_frame_sequence,
            }),
        );
    }
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
            let connected_playback_position_millis =
                if runtime.deck_source_mode == DeckSourceMode::ConnectedDecks {
                    if runtime.uses_direct_prolink() {
                        runtime
                            .direct_deck_source
                            .transport(deck.track_load_id())
                            .and_then(|transport| {
                                library_context
                                    .and_then(|context| context.millis_at_beat(transport.beat))
                            })
                    } else {
                        runtime
                            .connected_deck_source
                            .transport(deck.track_load_id())
                            .and_then(|transport| transport.position_millis.map(u64::from))
                    }
                } else {
                    None
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
                "effectiveBpmMilli": deck.effective_bpm_milli(),
                "playing": deck.is_playing(),
                "playbackPositionMillis": connected_playback_position_millis,
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
                        "known": library_context.is_some()
                            || deck_source_kind != "beatLinkTriggerMidi",
                    },
                    "durationBeats": metadata.duration_beats(),
                    "beatGrid": library_context.map_or(
                        Value::Null,
                        LibraryPlanContext::beat_grid_json,
                    ),
                    "waveformPreview": waveform_preview,
                    "hotCues": library_context.map_or(
                        Value::Array(Vec::new()),
                        LibraryPlanContext::hot_cues_json,
                    ),
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
        if include_device_inspection {
            runtime.library_worker.snapshot_json()?
        } else {
            runtime.library_worker.status_snapshot_json()?
        },
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
        DecisionReason::PositionSeeked => "positionSeeked",
        DecisionReason::PlaybackTempoChanged => "playbackTempoChanged",
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
    #[error("Beat Link Trigger MIDI adapter failed: {0}")]
    BltMidi(#[from] BltMidiError),
    #[error("Direct Pro DJ Link adapter failed: {0}")]
    ProLinkProvider(#[from] ProLinkProviderError),
    #[cfg(not(test))]
    #[error("Direct Pro DJ Link bridge failed: {0}")]
    ProLinkBridge(#[from] BridgeSupervisorError),
    #[error("planner failed: {0}")]
    Planner(#[from] PlannerError),
    #[error("a Library plan could not be materialized: {0}")]
    PlanMaterialization(#[from] PlanValidationError),
    #[error("a Library phrase has no resolved Autoloop")]
    MissingLibraryAutoloopResolution,
    #[error("a resolved Library Autoloop has no MIDI button address")]
    MissingLibraryAutoloopAddress,
    #[error("the Library track has no fully mapped executable SoundSwitch Theme")]
    NoExecutableLibraryTheme,
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
    #[error("musical timing output failed: {0}")]
    Timing(String),
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
    use lumi_library::AutoloopTheme;
    use lumi_output_dry_run::canonical_output_transcript;
    use lumi_simulator::{SimulationControl, SimulationSpeed};

    #[test]
    fn signed_output_timing_offset_advances_or_delays_without_underflow() {
        assert_eq!(position_with_timing_offset(1_000, 35), 1_035);
        assert_eq!(position_with_timing_offset(1_000, -35), 965);
        assert_eq!(position_with_timing_offset(10, -35), 0);
        assert_eq!(position_with_timing_offset(u64::MAX - 5, 35), u64::MAX);
    }

    #[test]
    fn pro_dj_link_tempo_remains_authoritative_while_output_transport_is_held() {
        let observation = lumi_prolink_input::ProLinkTimingObservation {
            deck_id: lumi_domain::DeckId::new(2),
            observed_at_nanos: 12_345_000,
            absolute_beat: 42,
            effective_bpm_milli: 155_250,
            beat_within_bar: 3,
            playing: true,
            generation: 7,
            discontinuity: false,
        };

        for operation in [OperationState::Off, OperationState::Paused] {
            let anchor = prolink_timing_anchor(observation, operation);
            assert_eq!(anchor.bpm_milli, 155_250);
            assert_eq!(anchor.beat_within_bar, 3);
            assert_eq!(anchor.deck_number, Some(2));
            assert!(!anchor.playing);
        }

        assert!(prolink_timing_anchor(observation, OperationState::Armed).playing);
        assert!(prolink_timing_anchor(observation, OperationState::Live).playing);
    }

    #[test]
    fn pro_dj_link_stale_window_tracks_three_beats_with_safe_bounds() {
        assert_eq!(
            prolink_timing_stale_after(Some(140_000)),
            Duration::from_micros(1_285_714)
        );
        assert_eq!(
            prolink_timing_stale_after(Some(300_000)),
            MINIMUM_PROLINK_TIMING_STALE_AFTER
        );
        assert_eq!(
            prolink_timing_stale_after(Some(20_000)),
            MAXIMUM_PROLINK_TIMING_STALE_AFTER
        );
        assert_eq!(
            prolink_timing_stale_after(None),
            MINIMUM_PROLINK_TIMING_STALE_AFTER
        );
    }

    #[test]
    fn pro_dj_link_bank_prearm_keeps_settle_time_and_one_engine_tick() {
        assert_eq!(
            prolink_prearm_delay(140_000),
            Duration::from_micros(358_571)
        );
        let beat_duration = Duration::from_micros(60_000_000_000_u64 / 300_000);
        assert_eq!(
            beat_duration.saturating_sub(prolink_prearm_delay(300_000)),
            BANK_SETTLE_DELAY.saturating_add(INTEGRATION_PUMP_INTERVAL)
        );
    }

    #[test]
    fn integration_pump_metrics_detect_starvation_without_unbounded_samples() {
        let mut metrics = IntegrationPumpMetrics::new();
        let started = Instant::now();
        metrics.record(started);
        metrics.record(started + INTEGRATION_PUMP_INTERVAL);
        metrics.record(started + INTEGRATION_PUMP_INTERVAL.saturating_mul(4));

        assert_eq!(metrics.tick_count, 3);
        assert_eq!(metrics.starvation_count, 1);
        assert_eq!(
            metrics.max_lateness_micros,
            u64::try_from(INTEGRATION_PUMP_INTERVAL.saturating_mul(2).as_micros())
                .unwrap_or(u64::MAX)
        );
    }

    #[tokio::test]
    async fn command_reader_retains_partial_input_when_timing_tick_cancels_the_read() {
        let (mut client, server) = tokio::io::duplex(128);
        let mut reader = BufReader::new(server);
        let mut buffer = Vec::new();
        client
            .write_all(b"{\"partial\":")
            .await
            .unwrap_or_else(|error| panic!("partial command should write: {error}"));

        let interrupted = tokio::time::timeout(
            Duration::from_millis(5),
            read_command_line(&mut reader, &mut buffer),
        )
        .await;
        assert!(interrupted.is_err());
        assert_eq!(buffer, b"{\"partial\":");

        client
            .write_all(b"true}\n")
            .await
            .unwrap_or_else(|error| panic!("remaining command should write: {error}"));
        let line = read_command_line(&mut reader, &mut buffer)
            .await
            .unwrap_or_else(|error| panic!("command should complete: {error}"));
        assert_eq!(line.as_deref(), Some(b"{\"partial\":true}".as_slice()));
        assert!(buffer.is_empty());
    }

    #[test]
    fn pro_dj_link_preflight_failure_is_actionable_and_not_retryable() {
        let message = "Close rekordbox before starting Pro DJ Link.";
        let result = application_error_envelope(
            1,
            "select-live-decks",
            &CommandApplicationError::ProLinkUnavailable(message.to_owned()),
        );
        let Ok(envelope) = result else {
            panic!("preflight error must serialize");
        };

        assert_eq!(
            envelope.payload.get("code"),
            Some(&json!("proDjLinkUnavailable"))
        );
        assert_eq!(envelope.payload.get("message"), Some(&json!(message)));
        assert_eq!(envelope.payload.get("retryable"), Some(&json!(false)));
    }

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
        let catalog = AutoloopCatalog::try_new(
            1,
            0,
            (1_u16..=4)
                .map(|number| {
                    AutoloopTheme::try_new(
                        lumi_domain::ThemeId::new(u64::from(number)),
                        format!("Bank {number}"),
                        number,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_else(|error| panic!("test themes must be valid: {error}")),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_or_else(|error| panic!("test catalog must be valid: {error}"));
        let mut worker = PlanningWorker::new(&catalog);
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
            preview.payload["decks"][1]["track"]["beatGrid"]["beatsPerBar"],
            4
        );
        assert!(
            preview.payload["decks"][1]["track"]["beatGrid"]["timesMillis"]
                .as_array()
                .is_some_and(|markers| !markers.is_empty() && markers[0].is_number())
        );
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
    fn local_playback_executes_first_phrase_once_and_activates_a_paused_seek_on_resume() {
        let mut runtime =
            initialized_runtime_for_mode(ManualClock::new(0), DeckSourceMode::LocalPlayback)
                .unwrap_or_else(|error| panic!("local product runtime must initialize: {error}"));
        apply_current_session_command(&mut runtime, |expected_state_revision| {
            SessionCommand::LoadLibraryTrackOnLocalDeck {
                track_id: 1,
                deck_id: lumi_domain::DeckId::new(1),
                expected_timeline_revision: 1,
                expected_state_revision,
            }
        });
        let track_load_id = runtime
            .state
            .state()
            .deck(lumi_domain::DeckId::new(1))
            .map(lumi_domain::DeckState::track_load_id)
            .unwrap_or_else(|| panic!("local deck must be loaded"));
        let second_phrase_beat = runtime
            .state
            .state()
            .deck(lumi_domain::DeckId::new(1))
            .and_then(|deck| deck.metadata().phrases().get(1))
            .map(|phrase| phrase.start_beat())
            .unwrap_or_else(|| panic!("fixture track must have a second phrase"));
        let second_phrase_millis = runtime
            .planning_worker
            .library_context(track_load_id)
            .and_then(|context| {
                (0..120_000_u64)
                    .step_by(100)
                    .find(|position| context.beat_at_millis(*position) >= second_phrase_beat)
            })
            .unwrap_or_else(|| panic!("fixture beat grid must reach its second phrase"));

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
        apply_session_command(
            &mut runtime,
            SessionCommand::UpdateLocalPlaybackTransport {
                deck_id: lumi_domain::DeckId::new(1),
                track_load_id,
                position_millis: 0,
                playing: true,
            },
        );
        assert_eq!(runtime.output_worker.provider.records().count(), 1);

        apply_session_command(
            &mut runtime,
            SessionCommand::SetOutputTimingOffset { millis: 20 },
        );
        assert_eq!(runtime.output_worker.timing_offset_millis(), 0);
        assert_eq!(
            runtime.output_worker.pending_timing_offset_millis(),
            Some(20)
        );

        for playing in [false, true] {
            apply_session_command(
                &mut runtime,
                SessionCommand::UpdateLocalPlaybackTransport {
                    deck_id: lumi_domain::DeckId::new(1),
                    track_load_id,
                    position_millis: 0,
                    playing,
                },
            );
        }
        assert_eq!(runtime.output_worker.provider.records().count(), 1);
        assert_eq!(runtime.output_worker.timing_offset_millis(), 0);
        assert_eq!(
            runtime.output_worker.pending_timing_offset_millis(),
            Some(20)
        );

        apply_session_command(
            &mut runtime,
            SessionCommand::UpdateLocalPlaybackTransport {
                deck_id: lumi_domain::DeckId::new(1),
                track_load_id,
                position_millis: second_phrase_millis,
                playing: false,
            },
        );
        assert_eq!(runtime.output_worker.provider.records().count(), 1);
        assert_eq!(runtime.output_worker.timing_offset_millis(), 0);
        assert_eq!(
            runtime.output_worker.pending_timing_offset_millis(),
            Some(20)
        );
        apply_session_command(
            &mut runtime,
            SessionCommand::UpdateLocalPlaybackTransport {
                deck_id: lumi_domain::DeckId::new(1),
                track_load_id,
                position_millis: second_phrase_millis,
                playing: true,
            },
        );
        assert_eq!(runtime.output_worker.provider.records().count(), 2);
        assert_eq!(runtime.output_worker.timing_offset_millis(), 20);
        assert_eq!(runtime.output_worker.pending_timing_offset_millis(), None);
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

    #[test]
    fn committed_blt_midi_frame_enters_the_product_deck_source_port() {
        let mut runtime =
            match initialized_runtime_for_mode(ManualClock::new(0), DeckSourceMode::ConnectedDecks)
            {
                Ok(runtime) => runtime,
                Err(error) => panic!("connected test engine must initialize: {error}"),
            };
        let fields = [
            (16, 31),
            (17, 42),
            (18, 0),
            (19, 0),
            (20, 0),
            (21, 2),
            (22, 1),
            (23, 80),
            (24, 119),
            (25, 7),
            (26, 80),
            (27, 0),
            (28, 0),
            (29, 58),
            (30, 1),
            (31, 0),
            (32, 7),
            (33, 80),
            (34, 119),
            (35, 7),
            (36, 0),
            (37, 0),
            (38, 0),
            (39, 0),
            (40, 0),
            (41, 10),
            (42, 68),
            (43, 4),
            (119, lumi_blt_midi::PROTOCOL_VERSION),
        ];
        for (controller, value) in fields {
            if let Err(error) = runtime.connected_deck_source.ingest(
                MidiChannelVoiceMessage {
                    status: 0xb,
                    channel: 2,
                    data_one: controller,
                    data_two: value,
                },
                MonotonicTime::new(1),
            ) {
                panic!("BLT frame must ingest: {error}");
            }
        }
        if let Err(error) = process_pending_source_events(&mut runtime) {
            panic!("BLT events must enter the engine: {error}");
        }
        assert_eq!(
            runtime.state.state().leader_deck(),
            Some(lumi_domain::DeckId::new(2))
        );
        let deck = runtime
            .state
            .state()
            .deck(lumi_domain::DeckId::new(2))
            .unwrap_or_else(|| panic!("BLT Deck 2 must be loaded"));
        assert_eq!(deck.metadata().title(), "External track 42");
        assert_eq!(deck.beat(), 80);
        assert_eq!(deck.effective_bpm_milli(), 130_000);
        assert!(deck.is_playing());
        assert!(
            runtime
                .state
                .state()
                .plan(lumi_domain::DeckId::new(2))
                .is_none()
        );
        assert_eq!(runtime.output_worker.provider.records().count(), 0);

        let tempo_update = [
            (16, 31),
            (17, 42),
            (18, 0),
            (19, 0),
            (20, 0),
            (21, 2),
            (22, 1),
            (23, 80),
            (24, 119),
            (25, 7),
            (26, 80),
            (27, 0),
            (28, 0),
            (29, 58),
            (30, 1),
            (31, 0),
            (32, 8),
            (33, 100),
            (34, 1),
            (35, 8),
            (36, 0),
            (37, 0),
            (38, 0),
            (39, 0),
            (40, 0),
            (41, 10),
            (42, 68),
            (43, 4),
            (119, lumi_blt_midi::PROTOCOL_VERSION),
        ];
        for (controller, value) in tempo_update {
            if let Err(error) = runtime.connected_deck_source.ingest(
                MidiChannelVoiceMessage {
                    status: 0xb,
                    channel: 2,
                    data_one: controller,
                    data_two: value,
                },
                MonotonicTime::new(2),
            ) {
                panic!("BLT tempo update must ingest: {error}");
            }
        }
        if let Err(error) = process_pending_source_events(&mut runtime) {
            panic!("BLT tempo update must enter the engine: {error}");
        }
        let deck = runtime
            .state
            .state()
            .deck(lumi_domain::DeckId::new(2))
            .unwrap_or_else(|| panic!("BLT Deck 2 must remain loaded"));
        assert_eq!(deck.metadata().bpm_milli(), 130_000);
        assert_eq!(deck.effective_bpm_milli(), 131_300);
        assert_eq!(
            runtime
                .connected_deck_source
                .transport(deck.track_load_id())
                .and_then(|transport| transport.position_millis),
            Some(74_250)
        );
        assert_eq!(
            runtime
                .connected_deck_source
                .diagnostics()
                .committed_frame_count,
            2
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
