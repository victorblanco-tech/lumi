use crate::{
    ClientId, CommandSequence, DeckId, DeckSourceStatus, EffectId, EffectSequence, LightingPlan,
    MonotonicTime, OutputEffectResult, SourceId, SourceSequence, StateRevision, TrackLoadId,
    TrackMetadata, WorkerId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckObservation {
    SourceStatusChanged {
        status: DeckSourceStatus,
    },
    TrackLoaded {
        deck_id: DeckId,
        metadata: TrackMetadata,
        track_load_id: TrackLoadId,
    },
    PlaybackPosition {
        deck_id: DeckId,
        track_load_id: TrackLoadId,
        beat: u32,
    },
    TrackUnloaded {
        deck_id: DeckId,
        track_load_id: TrackLoadId,
    },
    PhraseChanged {
        deck_id: DeckId,
        track_load_id: TrackLoadId,
        phrase_index: u16,
    },
    LeaderChanged {
        deck_id: DeckId,
        track_load_id: TrackLoadId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationEnvelope {
    pub source_id: SourceId,
    pub sequence: SourceSequence,
    pub observed_at: MonotonicTime,
    pub observation: DeckObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationCommand {
    Arm,
    Start,
    Pause,
    Off,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserCommandEnvelope {
    pub client_id: ClientId,
    pub sequence: CommandSequence,
    pub expected_state_revision: StateRevision,
    pub issued_at: MonotonicTime,
    pub command: OperationCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectResult {
    PlanGenerated(LightingPlan),
    OutputEffectRecorded(OutputEffectResult),
    OutputGateClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectResultEnvelope {
    pub effect_id: EffectId,
    pub worker_id: WorkerId,
    pub sequence: EffectSequence,
    pub completed_at: MonotonicTime,
    pub result: EffectResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueOverloadEvent {
    pub occurred_at: MonotonicTime,
    pub rejected_kind: DomainEventKind,
    pub rejected_critical: bool,
    pub occurrences: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainEvent {
    RuntimeStarted { at: MonotonicTime },
    Observation(ObservationEnvelope),
    UserCommand(UserCommandEnvelope),
    EffectResult(EffectResultEnvelope),
    QueueOverloaded(QueueOverloadEvent),
}

impl DomainEvent {
    #[must_use]
    pub const fn kind(&self) -> DomainEventKind {
        match self {
            Self::RuntimeStarted { .. } => DomainEventKind::RuntimeStarted,
            Self::Observation(_) => DomainEventKind::Observation,
            Self::UserCommand(_) => DomainEventKind::UserCommand,
            Self::EffectResult(_) => DomainEventKind::EffectResult,
            Self::QueueOverloaded(_) => DomainEventKind::QueueOverloaded,
        }
    }

    #[must_use]
    pub const fn is_critical(&self) -> bool {
        !matches!(self, Self::Observation(_))
    }

    #[must_use]
    pub const fn monotonic_time(&self) -> MonotonicTime {
        match self {
            Self::RuntimeStarted { at } => *at,
            Self::Observation(event) => event.observed_at,
            Self::UserCommand(event) => event.issued_at,
            Self::EffectResult(event) => event.completed_at,
            Self::QueueOverloaded(event) => event.occurred_at,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainEventKind {
    RuntimeStarted,
    Observation,
    UserCommand,
    EffectResult,
    QueueOverloaded,
}
