use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub const MAX_REMOTE_PLAYERS: usize = 2;
pub const MAX_REMOTE_PHRASES: usize = 256;
pub const MAX_REMOTE_HOT_CUES: usize = 16;
pub const MAX_REMOTE_WAVEFORM_POINTS: usize = 16_384;
pub const MAX_REMOTE_BEATS: usize = 16_384;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Off,
    Armed,
    Live,
    Paused,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationHealth {
    Unavailable,
    Starting,
    Ready,
    Degraded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteIntegrationStatus {
    pub pro_dj_link: IntegrationHealth,
    pub light_output: IntegrationHealth,
    pub ableton_link: IntegrationHealth,
    pub ableton_link_enabled: bool,
    pub ableton_link_bpm_milli: Option<u64>,
    pub timing_offset_millis: i16,
    pub pending_timing_offset_millis: Option<i16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLiveProjection {
    pub projection_revision: u64,
    pub state_revision: u64,
    pub engine_version: String,
    pub operation_state: OperationState,
    pub leader_player_number: Option<u8>,
    pub integrations: RemoteIntegrationStatus,
    pub players: Vec<RemotePlayer>,
    pub live_plan: Option<RemoteLightPlan>,
    pub next_plan: Option<RemoteLightPlan>,
    pub theme_options: Vec<RemoteThemeOption>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlayer {
    pub player_number: u8,
    pub hardware_model: Option<String>,
    pub track_load_id: u64,
    pub transport: RemoteTransportAnchor,
    pub track: RemoteTrack,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTransportAnchor {
    pub track_load_id: u64,
    pub beat: u64,
    pub position_millis: Option<u64>,
    pub effective_bpm_milli: u64,
    pub playing: bool,
    pub discontinuity_revision: u64,
    pub observed_at_unix_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTrack {
    pub track_id: Option<u64>,
    pub title: String,
    pub artist: String,
    pub original_bpm_milli: u64,
    pub color_rgb: Option<u32>,
    pub key: String,
    pub duration_beats: u64,
    pub beat_grid: Option<RemoteBeatGrid>,
    pub waveform: Vec<RemoteWaveformPoint>,
    pub hot_cues: Vec<RemoteHotCue>,
    pub phrases: Vec<RemotePhrase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBeatGrid {
    pub beats_per_bar: u8,
    pub duration_millis: u64,
    pub times_millis: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWaveformPoint {
    pub low: u8,
    pub mid: u8,
    pub high: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHotCue {
    pub index: u8,
    pub time_millis: u64,
    pub loop_end_millis: Option<u64>,
    pub color_rgb: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePhrase {
    pub index: u16,
    pub start_beat: u64,
    pub end_beat: u64,
    pub kind: String,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLightPlan {
    pub plan_id: String,
    pub player_number: u8,
    pub track_load_id: u64,
    pub revision: u64,
    pub theme_id: Option<u64>,
    pub theme_name: Option<String>,
    pub cues: Vec<RemotePlanCue>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlanCue {
    pub phrase_index: u16,
    pub start_beat: u64,
    pub end_beat: u64,
    pub locked: bool,
    pub theme_id: Option<u64>,
    pub theme_name: Option<String>,
    pub autoloop_number: Option<u8>,
    pub autoloop_name: Option<String>,
    pub static_look_name: Option<String>,
    pub available_autoloops: Vec<RemoteAutoloopChoice>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteThemeOption {
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAutoloopChoice {
    pub number: u8,
    pub name: String,
    pub bank_number: u8,
}

impl RemoteLiveProjection {
    /// Converts the engine's internal snapshot into an explicit, path-free
    /// Remote Live contract. Unknown internal fields are discarded by the
    /// typed wire structs below rather than forwarded to the LAN.
    pub fn from_engine_snapshot_payload(
        payload: &Map<String, Value>,
        projection_revision: u64,
        observed_at_unix_millis: u64,
    ) -> Result<Self, ProjectionError> {
        let wire: EngineSnapshotWire = serde_json::from_value(Value::Object(payload.clone()))
            .map_err(|error| ProjectionError::InvalidEngineSnapshot(error.to_string()))?;
        if wire.kind != "stateSnapshot" || wire.deck_source.mode != "connectedDecks" {
            return Err(ProjectionError::NotConnectedDecks);
        }
        let players = wire
            .decks
            .into_iter()
            .map(|deck| deck.into_remote(observed_at_unix_millis))
            .collect::<Result<Vec<_>, _>>()?;
        let projection = Self {
            projection_revision,
            state_revision: wire.state_revision,
            engine_version: wire.engine_version,
            operation_state: operation_state(&wire.operation_state)?,
            leader_player_number: wire
                .leader_deck_id
                .map(|value| {
                    u8::try_from(value).map_err(|_| ProjectionError::InvalidPlayerIdentity)
                })
                .transpose()?,
            integrations: RemoteIntegrationStatus {
                pro_dj_link: integration_health(&wire.deck_source.status),
                light_output: wire
                    .midi_integration
                    .as_ref()
                    .map_or(IntegrationHealth::Unavailable, |status| {
                        integration_health(&status.state)
                    }),
                ableton_link: wire
                    .ableton_link_integration
                    .as_ref()
                    .map_or(IntegrationHealth::Unavailable, |status| {
                        integration_health(&status.state)
                    }),
                ableton_link_enabled: wire
                    .ableton_link_integration
                    .as_ref()
                    .is_some_and(|status| status.enabled),
                ableton_link_bpm_milli: wire
                    .ableton_link_integration
                    .as_ref()
                    .and_then(|status| status.bpm_milli),
                timing_offset_millis: wire
                    .midi_integration
                    .as_ref()
                    .map_or(0, |status| status.timing_offset_millis),
                pending_timing_offset_millis: wire
                    .midi_integration
                    .as_ref()
                    .and_then(|status| status.pending_timing_offset_millis),
            },
            players,
            live_plan: wire
                .live_plan
                .map(EnginePlanWire::into_remote)
                .transpose()?,
            next_plan: wire
                .next_plan
                .map(EnginePlanWire::into_remote)
                .transpose()?,
            theme_options: wire
                .planning_options
                .themes
                .into_iter()
                .map(|theme| RemoteThemeOption {
                    id: theme.id,
                    name: theme.name,
                })
                .collect(),
        };
        projection.validate()?;
        Ok(projection)
    }

    pub fn validate(&self) -> Result<(), ProjectionError> {
        validate_text("engineVersion", &self.engine_version, 64, false)?;
        if self.players.len() > MAX_REMOTE_PLAYERS {
            return Err(ProjectionError::TooManyPlayers);
        }
        if self.integrations.timing_offset_millis.unsigned_abs() > 250
            || self
                .integrations
                .pending_timing_offset_millis
                .is_some_and(|value| value.unsigned_abs() > 250)
        {
            return Err(ProjectionError::InvalidTimingOffset);
        }
        if self.leader_player_number.is_some_and(|leader| {
            !self
                .players
                .iter()
                .any(|player| player.player_number == leader)
        }) {
            return Err(ProjectionError::UnknownLeader);
        }
        for player in &self.players {
            player.validate()?;
        }
        for plan in [&self.live_plan, &self.next_plan].into_iter().flatten() {
            plan.validate(&self.players)?;
        }
        for theme in &self.theme_options {
            validate_text("themeOptionName", &theme.name, 128, false)?;
        }
        Ok(())
    }
}

impl RemotePlayer {
    fn validate(&self) -> Result<(), ProjectionError> {
        if !(1..=6).contains(&self.player_number)
            || self.track_load_id == 0
            || self.transport.track_load_id != self.track_load_id
        {
            return Err(ProjectionError::InvalidPlayerIdentity);
        }
        if let Some(model) = &self.hardware_model {
            validate_text("hardwareModel", model, 96, true)?;
        }
        self.track.validate()
    }
}

impl RemoteTrack {
    fn validate(&self) -> Result<(), ProjectionError> {
        validate_text("title", &self.title, 512, true)?;
        validate_text("artist", &self.artist, 512, true)?;
        validate_text("key", &self.key, 32, true)?;
        if self.waveform.len() > MAX_REMOTE_WAVEFORM_POINTS {
            return Err(ProjectionError::WaveformOversized);
        }
        if self.hot_cues.len() > MAX_REMOTE_HOT_CUES {
            return Err(ProjectionError::TooManyHotCues);
        }
        if self.phrases.len() > MAX_REMOTE_PHRASES {
            return Err(ProjectionError::TooManyPhrases);
        }
        if let Some(grid) = &self.beat_grid {
            if grid.times_millis.len() > MAX_REMOTE_BEATS {
                return Err(ProjectionError::BeatGridOversized);
            }
            if !(1..=16).contains(&grid.beats_per_bar)
                || grid.duration_millis == 0
                || grid
                    .times_millis
                    .windows(2)
                    .any(|window| window[0] >= window[1])
                || grid
                    .times_millis
                    .last()
                    .is_some_and(|last| *last > grid.duration_millis)
            {
                return Err(ProjectionError::InvalidBeatGrid);
            }
        }
        let mut prior_end = 0;
        for phrase in &self.phrases {
            validate_text("phraseKind", &phrase.kind, 64, true)?;
            if let Some(role) = &phrase.role_id {
                validate_text("phraseRoleId", role, 128, true)?;
            }
            if let Some(role) = &phrase.role_name {
                validate_text("phraseRoleName", role, 128, true)?;
            }
            if phrase.start_beat < prior_end || phrase.end_beat <= phrase.start_beat {
                return Err(ProjectionError::InvalidPhraseRange);
            }
            prior_end = phrase.end_beat;
        }
        Ok(())
    }
}

impl RemoteLightPlan {
    fn validate(&self, players: &[RemotePlayer]) -> Result<(), ProjectionError> {
        validate_text("planId", &self.plan_id, 128, false)?;
        if !players.iter().any(|player| {
            player.player_number == self.player_number && player.track_load_id == self.track_load_id
        }) {
            return Err(ProjectionError::PlanPlayerMismatch);
        }
        if self.cues.len() > MAX_REMOTE_PHRASES {
            return Err(ProjectionError::TooManyPlanCues);
        }
        for (index, cue) in self.cues.iter().enumerate() {
            if usize::from(cue.phrase_index) != index || cue.end_beat <= cue.start_beat {
                return Err(ProjectionError::InvalidPlanCue);
            }
            if cue
                .autoloop_number
                .is_some_and(|number| !(1..=32).contains(&number))
            {
                return Err(ProjectionError::InvalidPlanCue);
            }
            for (field, value) in [
                ("themeName", cue.theme_name.as_ref()),
                ("autoloopName", cue.autoloop_name.as_ref()),
                ("staticLookName", cue.static_look_name.as_ref()),
            ] {
                if let Some(value) = value {
                    validate_text(field, value, 256, true)?;
                }
            }
            for choice in &cue.available_autoloops {
                if !(1..=32).contains(&choice.number) || !(1..=4).contains(&choice.bank_number) {
                    return Err(ProjectionError::InvalidPlanCue);
                }
                validate_text("autoloopChoiceName", &choice.name, 256, false)?;
            }
        }
        Ok(())
    }
}

fn operation_state(value: &str) -> Result<OperationState, ProjectionError> {
    match value {
        "off" => Ok(OperationState::Off),
        "armed" => Ok(OperationState::Armed),
        "live" => Ok(OperationState::Live),
        "paused" => Ok(OperationState::Paused),
        _ => Err(ProjectionError::InvalidOperationState),
    }
}

fn integration_health(value: &str) -> IntegrationHealth {
    match value {
        "ready" | "running" => IntegrationHealth::Ready,
        "starting" => IntegrationHealth::Starting,
        "degraded" | "error" | "failed" => IntegrationHealth::Degraded,
        _ => IntegrationHealth::Unavailable,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineSnapshotWire {
    kind: String,
    state_revision: u64,
    operation_state: String,
    engine_version: String,
    leader_deck_id: Option<u64>,
    deck_source: EngineDeckSourceWire,
    midi_integration: Option<EngineMidiStatusWire>,
    ableton_link_integration: Option<EngineLinkStatusWire>,
    decks: Vec<EngineDeckWire>,
    live_plan: Option<EnginePlanWire>,
    next_plan: Option<EnginePlanWire>,
    planning_options: EnginePlanningOptionsWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineDeckSourceWire {
    mode: String,
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineMidiStatusWire {
    state: String,
    timing_offset_millis: i16,
    pending_timing_offset_millis: Option<i16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineLinkStatusWire {
    enabled: bool,
    state: String,
    bpm_milli: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineDeckWire {
    deck_id: u64,
    hardware_model: Option<String>,
    track_load_id: u64,
    beat: u64,
    effective_bpm_milli: u64,
    playing: bool,
    playback_position_millis: Option<u64>,
    transport_revision: Option<u64>,
    track: EngineTrackWire,
}

impl EngineDeckWire {
    fn into_remote(self, observed_at_unix_millis: u64) -> Result<RemotePlayer, ProjectionError> {
        let player_number =
            u8::try_from(self.deck_id).map_err(|_| ProjectionError::InvalidPlayerIdentity)?;
        Ok(RemotePlayer {
            player_number,
            hardware_model: self.hardware_model,
            track_load_id: self.track_load_id,
            transport: RemoteTransportAnchor {
                track_load_id: self.track_load_id,
                beat: self.beat,
                position_millis: self.playback_position_millis,
                effective_bpm_milli: self.effective_bpm_milli,
                playing: self.playing,
                discontinuity_revision: self.transport_revision.unwrap_or(0),
                observed_at_unix_millis,
            },
            track: self.track.into_remote()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineTrackWire {
    id: Option<u64>,
    title: String,
    artist: String,
    bpm_milli: u64,
    color_rgb: Option<u32>,
    key: EngineKeyWire,
    duration_beats: u64,
    beat_grid: Option<RemoteBeatGrid>,
    waveform_preview: Option<EngineWaveformWire>,
    #[serde(default)]
    hot_cues: Vec<RemoteHotCue>,
    #[serde(default)]
    phrases: Vec<EnginePhraseWire>,
}

impl EngineTrackWire {
    fn into_remote(self) -> Result<RemoteTrack, ProjectionError> {
        if self
            .waveform_preview
            .as_ref()
            .is_some_and(|waveform| waveform.style != "rgb")
        {
            return Err(ProjectionError::InvalidWaveformStyle);
        }
        Ok(RemoteTrack {
            track_id: self.id,
            title: self.title,
            artist: self.artist,
            original_bpm_milli: self.bpm_milli,
            color_rgb: self.color_rgb,
            key: if self.key.known {
                format!("{} {}", self.key.pitch_class, self.key.mode)
            } else {
                String::new()
            },
            duration_beats: self.duration_beats,
            beat_grid: self.beat_grid,
            waveform: self
                .waveform_preview
                .map_or_else(Vec::new, |waveform| waveform.points),
            hot_cues: self.hot_cues,
            phrases: self
                .phrases
                .into_iter()
                .map(EnginePhraseWire::into_remote)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineKeyWire {
    pitch_class: String,
    mode: String,
    known: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineWaveformWire {
    style: String,
    points: Vec<RemoteWaveformPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePhraseWire {
    index: u64,
    start_beat: u64,
    end_beat: u64,
    kind: String,
    role: Option<EnginePhraseRoleWire>,
}

impl EnginePhraseWire {
    fn into_remote(self) -> Result<RemotePhrase, ProjectionError> {
        Ok(RemotePhrase {
            index: u16::try_from(self.index).map_err(|_| ProjectionError::TooManyPhrases)?,
            start_beat: self.start_beat,
            end_beat: self.end_beat,
            kind: self.kind,
            role_id: self.role.as_ref().map(|role| role.role_id.clone()),
            role_name: self.role.map(|role| role.role_name),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePhraseRoleWire {
    role_id: String,
    role_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePlanningOptionsWire {
    #[serde(default)]
    themes: Vec<EngineThemeWire>,
}

#[derive(Deserialize)]
struct EngineThemeWire {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePlanWire {
    plan_id: String,
    deck_id: u64,
    track_load_id: u64,
    revision: u64,
    theme_decision: Option<EngineThemeDecisionWire>,
    #[serde(default)]
    cues: Vec<EnginePlanCueWire>,
}

impl EnginePlanWire {
    fn into_remote(self) -> Result<RemoteLightPlan, ProjectionError> {
        let player_number =
            u8::try_from(self.deck_id).map_err(|_| ProjectionError::InvalidPlayerIdentity)?;
        Ok(RemoteLightPlan {
            plan_id: self.plan_id,
            player_number,
            track_load_id: self.track_load_id,
            revision: self.revision,
            theme_id: self.theme_decision.as_ref().map(|theme| theme.theme_id),
            theme_name: self.theme_decision.map(|theme| theme.theme_name),
            cues: self
                .cues
                .into_iter()
                .map(EnginePlanCueWire::into_remote)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineThemeDecisionWire {
    theme_id: u64,
    theme_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePlanCueWire {
    phrase_index: u64,
    start_beat: u64,
    end_beat: u64,
    locked: bool,
    action: EnginePlanActionWire,
    library_resolution: Option<EngineLibraryResolutionWire>,
}

impl EnginePlanCueWire {
    fn into_remote(self) -> Result<RemotePlanCue, ProjectionError> {
        let phrase_index =
            u16::try_from(self.phrase_index).map_err(|_| ProjectionError::TooManyPlanCues)?;
        let resolution = self.library_resolution;
        Ok(RemotePlanCue {
            phrase_index,
            start_beat: self.start_beat,
            end_beat: self.end_beat,
            locked: self.locked,
            theme_id: self.action.theme_id,
            theme_name: self.action.theme_name,
            autoloop_number: resolution.as_ref().and_then(|value| value.autoloop_number),
            autoloop_name: resolution
                .as_ref()
                .and_then(|value| value.dry_run_entry.as_ref())
                .map(|entry| entry.name.clone()),
            static_look_name: resolution
                .as_ref()
                .and_then(|value| {
                    value
                        .modifier_choices
                        .iter()
                        .find(|modifier| modifier.kind == "staticLook")
                })
                .map(|modifier| modifier.name.clone()),
            available_autoloops: resolution.map_or_else(Vec::new, |value| {
                value
                    .choices
                    .into_iter()
                    .filter_map(|choice| {
                        Some(RemoteAutoloopChoice {
                            number: u8::try_from(choice.id).ok()?,
                            name: choice.name,
                            bank_number: u8::try_from(choice.bank_number).ok()?,
                        })
                    })
                    .collect()
            }),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePlanActionWire {
    theme_id: Option<u64>,
    theme_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineLibraryResolutionWire {
    autoloop_number: Option<u8>,
    dry_run_entry: Option<EngineNamedEntryWire>,
    #[serde(default)]
    choices: Vec<EngineAutoloopChoiceWire>,
    #[serde(default)]
    modifier_choices: Vec<EngineModifierChoiceWire>,
}

#[derive(Deserialize)]
struct EngineNamedEntryWire {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineAutoloopChoiceWire {
    id: u64,
    name: String,
    bank_number: u64,
}

#[derive(Deserialize)]
struct EngineModifierChoiceWire {
    name: String,
    kind: String,
}

fn validate_text(
    field: &'static str,
    value: &str,
    maximum: usize,
    may_be_empty: bool,
) -> Result<(), ProjectionError> {
    if (!may_be_empty && value.is_empty())
        || value.len() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(ProjectionError::InvalidText(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectionError {
    #[error("remote projection contains more than two Players")]
    TooManyPlayers,
    #[error("remote projection timing offset is outside the safe range")]
    InvalidTimingOffset,
    #[error("remote projection leader does not identify a loaded Player")]
    UnknownLeader,
    #[error("remote projection Player identity is invalid")]
    InvalidPlayerIdentity,
    #[error("remote waveform exceeds its bounded point count")]
    WaveformOversized,
    #[error("remote beatgrid exceeds its bounded beat count")]
    BeatGridOversized,
    #[error("remote beatgrid is invalid or non-monotonic")]
    InvalidBeatGrid,
    #[error("remote projection contains too many Hot Cues")]
    TooManyHotCues,
    #[error("remote projection contains too many phrases")]
    TooManyPhrases,
    #[error("remote phrase range is invalid")]
    InvalidPhraseRange,
    #[error("remote Light Plan does not match its Player load")]
    PlanPlayerMismatch,
    #[error("remote Light Plan contains too many cues")]
    TooManyPlanCues,
    #[error("remote Light Plan cue is invalid")]
    InvalidPlanCue,
    #[error("remote projection field {0} is invalid or oversized")]
    InvalidText(&'static str),
    #[error("engine snapshot cannot be represented by the remote contract: {0}")]
    InvalidEngineSnapshot(String),
    #[error("Remote Live is available only for connected Players")]
    NotConnectedDecks,
    #[error("engine operation state is unsupported")]
    InvalidOperationState,
    #[error("remote waveform must use RGB style")]
    InvalidWaveformStyle,
}

#[cfg(test)]
mod tests {
    use super::{
        IntegrationHealth, OperationState, ProjectionError, RemoteIntegrationStatus,
        RemoteLiveProjection, RemotePlayer, RemoteTrack, RemoteTransportAnchor,
    };

    fn projection() -> RemoteLiveProjection {
        RemoteLiveProjection {
            projection_revision: 8,
            state_revision: 7,
            engine_version: "0.6.0-dev-3".to_owned(),
            operation_state: OperationState::Armed,
            leader_player_number: Some(1),
            integrations: RemoteIntegrationStatus {
                pro_dj_link: IntegrationHealth::Ready,
                light_output: IntegrationHealth::Ready,
                ableton_link: IntegrationHealth::Ready,
                ableton_link_enabled: true,
                ableton_link_bpm_milli: Some(140_000),
                timing_offset_millis: -20,
                pending_timing_offset_millis: None,
            },
            players: vec![RemotePlayer {
                player_number: 1,
                hardware_model: Some("CDJ-1500X".to_owned()),
                track_load_id: 99,
                transport: RemoteTransportAnchor {
                    track_load_id: 99,
                    beat: 65,
                    position_millis: Some(27_429),
                    effective_bpm_milli: 140_000,
                    playing: true,
                    discontinuity_revision: 2,
                    observed_at_unix_millis: 1,
                },
                track: RemoteTrack {
                    track_id: Some(42),
                    title: "Example Track".to_owned(),
                    artist: "Example Artist".to_owned(),
                    original_bpm_milli: 140_000,
                    color_rgb: Some(0x00FF_3366),
                    key: "8A".to_owned(),
                    duration_beats: 512,
                    beat_grid: None,
                    waveform: Vec::new(),
                    hot_cues: Vec::new(),
                    phrases: Vec::new(),
                },
            }],
            live_plan: None,
            next_plan: None,
            theme_options: Vec::new(),
        }
    }

    #[test]
    fn accepts_a_small_live_only_projection() {
        assert_eq!(projection().validate(), Ok(()));
    }

    #[test]
    fn rejects_a_leader_that_is_not_loaded() {
        let mut projection = projection();
        projection.leader_player_number = Some(2);
        assert_eq!(projection.validate(), Err(ProjectionError::UnknownLeader));
    }

    #[test]
    fn serialized_projection_cannot_contain_library_or_file_paths() -> Result<(), serde_json::Error>
    {
        let encoded = serde_json::to_string(&projection())?;
        assert!(!encoded.contains("library"));
        assert!(!encoded.contains("audioURI"));
        assert!(!encoded.contains("/Volumes/"));
        Ok(())
    }

    #[test]
    fn converts_an_internal_snapshot_to_a_live_only_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let value = serde_json::json!({
            "kind": "stateSnapshot",
            "stateRevision": 12,
            "operationState": "live",
            "engineVersion": "0.6.0-dev-3",
            "leaderDeckId": 1,
            "deckSource": { "mode": "connectedDecks", "status": "ready" },
            "midiIntegration": {
                "state": "ready",
                "timingOffsetMillis": -20,
                "pendingTimingOffsetMillis": null,
                "internalPath": "/Volumes/SECRET"
            },
            "abletonLinkIntegration": {
                "enabled": true,
                "state": "running",
                "bpmMilli": 140000
            },
            "decks": [{
                "deckId": 1,
                "hardwareModel": "CDJ-1500X",
                "trackLoadId": 88,
                "beat": 65,
                "effectiveBpmMilli": 140000,
                "playing": true,
                "playbackPositionMillis": 28000,
                "transportRevision": 3,
                "localPlayback": { "audioUri": "/Volumes/SECRET/track.wav" },
                "track": {
                    "id": 42,
                    "title": "Example Track",
                    "artist": "Example Artist",
                    "bpmMilli": 140000,
                    "colorRgb": 16724838,
                    "key": { "pitchClass": "A", "mode": "minor", "known": true },
                    "durationBeats": 512,
                    "beatGrid": { "beatsPerBar": 4, "durationMillis": 219000, "timesMillis": [0, 429] },
                    "waveformPreview": { "source": "localLibrary", "style": "rgb", "points": [{ "low": 255, "mid": 96, "high": 64 }] },
                    "hotCues": [{ "index": 1, "timeMillis": 0, "loopEndMillis": null, "name": "Do not forward", "colorRgb": 16711680 }],
                    "phrases": [{ "index": 0, "startBeat": 0, "endBeat": 32, "kind": "intro", "role": { "roleId": "intro", "roleName": "Intro" } }]
                }
            }],
            "livePlan": {
                "planId": "plan-1",
                "deckId": 1,
                "trackLoadId": 88,
                "revision": 4,
                "themeDecision": { "themeId": 1, "themeName": "Blue Pink" },
                "cues": [{
                    "phraseIndex": 0,
                    "startBeat": 0,
                    "endBeat": 32,
                    "locked": false,
                    "action": { "kind": "applyLook", "themeId": 1, "themeName": "Blue Pink" },
                    "libraryResolution": {
                        "autoloopNumber": 2,
                        "dryRunEntry": { "name": "Intro Blue Pink" },
                        "choices": [{ "id": 2, "name": "Intro Blue Pink", "bankNumber": 1 }],
                        "modifierChoices": [{ "name": "Moving Heads OFF", "kind": "staticLook" }]
                    }
                }]
            },
            "nextPlan": null,
            "planningOptions": { "themes": [{ "id": 1, "name": "Blue Pink" }], "scenes": [] },
            "library": { "databasePath": "/Users/example/SECRET.sqlite" }
        });
        let payload = value.as_object().ok_or("test payload must be an object")?;
        let projection = RemoteLiveProjection::from_engine_snapshot_payload(payload, 1, 10)?;
        let encoded = serde_json::to_string(&projection)?;
        assert_eq!(projection.players[0].player_number, 1);
        assert_eq!(
            projection
                .live_plan
                .as_ref()
                .and_then(|plan| plan.cues[0].autoloop_number),
            Some(2)
        );
        assert!(!encoded.contains("/Volumes/"));
        assert!(!encoded.contains("SECRET"));
        assert!(!encoded.contains("databasePath"));
        assert!(!encoded.contains("audioUri"));
        Ok(())
    }
}
