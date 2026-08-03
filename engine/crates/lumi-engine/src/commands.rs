use std::error::Error;
use std::fmt;

use lumi_domain::{
    OperationCommand, PlanId, PlanRevision, SceneId, StateRevision, ThemeId, TrackLoadId,
};
use lumi_protocol::{MessageEnvelope, MessageType};
use lumi_simulator::SimulationSpeed;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanCommandContext {
    pub plan_id: PlanId,
    pub track_load_id: TrackLoadId,
    pub expected_revision: PlanRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionCommand {
    GetSnapshot,
    QueryLibrary {
        search: String,
        playlist_id: Option<u64>,
        offset: u32,
        limit: u16,
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
        !matches!(self, Self::GetSnapshot | Self::QueryLibrary { .. })
    }

    pub const fn context(&self) -> Option<PlanCommandContext> {
        match self {
            Self::GetSnapshot
            | Self::QueryLibrary { .. }
            | Self::LoadDemoSession { .. }
            | Self::SetOperationState { .. }
            | Self::SetSimulationSpeed { .. }
            | Self::SetSimulationPlayback { .. }
            | Self::AdvanceSimulation { .. }
            | Self::AdvanceToNextTrack { .. }
            | Self::ResetDemoSession { .. } => None,
            Self::SelectTheme { context, .. }
            | Self::SelectScene { context, .. }
            | Self::SetCueLock { context, .. }
            | Self::RegeneratePlan { context } => Some(*context),
        }
    }
}

pub fn decode_command(envelope: &MessageEnvelope) -> Result<SessionCommand, CommandDecodeError> {
    if envelope.message_type != MessageType::Command {
        return Err(CommandDecodeError::WrongMessageType);
    }
    let kind = string(&envelope.payload, "kind")?;
    match kind {
        "getSnapshot" => Ok(SessionCommand::GetSnapshot),
        "queryLibrary" => Ok(SessionCommand::QueryLibrary {
            search: library_search(&envelope.payload)?,
            playlist_id: optional_unsigned(&envelope.payload, "playlistId")?,
            offset: u32::try_from(optional_unsigned(&envelope.payload, "offset")?.unwrap_or(0))
                .map_err(|_| CommandDecodeError::InvalidField("offset"))?,
            limit: library_limit(optional_unsigned(&envelope.payload, "limit")?.unwrap_or(50))?,
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

fn boolean(
    payload: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<bool, CommandDecodeError> {
    payload
        .get(field)
        .and_then(Value::as_bool)
        .ok_or(CommandDecodeError::InvalidField(field))
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
