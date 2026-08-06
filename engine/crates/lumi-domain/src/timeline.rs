use crate::{DecisionReason, DomainEvent, EffectResult, MonotonicTime, OutputEffectStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineSource {
    Runtime,
    DeckSource,
    Operation,
    Planner,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineResult {
    Accepted,
    Ignored,
    Scheduled,
    Simulated,
    Rejected,
    Skipped,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEntry {
    sequence: u64,
    occurred_at: MonotonicTime,
    source: TimelineSource,
    event_type: &'static str,
    result: TimelineResult,
    reason: DecisionReason,
}

impl TimelineEntry {
    pub(crate) fn from_event(sequence: u64, event: &DomainEvent, reason: DecisionReason) -> Self {
        let (source, event_type) = match event {
            DomainEvent::RuntimeStarted { .. } => (TimelineSource::Runtime, "runtimeStarted"),
            DomainEvent::Observation(observation) => (
                TimelineSource::DeckSource,
                match &observation.observation {
                    crate::DeckObservation::SourceStatusChanged { .. } => "sourceStatusChanged",
                    crate::DeckObservation::TrackLoaded { .. } => "trackLoaded",
                    crate::DeckObservation::PlaybackPosition { .. } => "playbackPosition",
                    crate::DeckObservation::PlaybackTempoChanged { .. } => "playbackTempoChanged",
                    crate::DeckObservation::PlaybackStateChanged { .. } => "playbackStateChanged",
                    crate::DeckObservation::TrackUnloaded { .. } => "trackUnloaded",
                    crate::DeckObservation::PhraseChanged { .. } => "phraseChanged",
                    crate::DeckObservation::LeaderChanged { .. } => "leaderChanged",
                },
            ),
            DomainEvent::UserCommand(command) => (
                TimelineSource::Operation,
                match command.command {
                    crate::OperationCommand::Arm => "arm",
                    crate::OperationCommand::Start => "start",
                    crate::OperationCommand::Pause => "pause",
                    crate::OperationCommand::Off => "off",
                },
            ),
            DomainEvent::EffectResult(effect) => match &effect.result {
                EffectResult::PlanGenerated(_) => (TimelineSource::Planner, "planGenerated"),
                EffectResult::OutputEffectRecorded(_) => {
                    (TimelineSource::Output, "outputEffectRecorded")
                }
                EffectResult::OutputGateClosed => (TimelineSource::Output, "outputGateClosed"),
            },
            DomainEvent::QueueOverloaded(_) => (TimelineSource::Runtime, "queueOverloaded"),
        };
        let result = match event {
            DomainEvent::EffectResult(effect) => match &effect.result {
                EffectResult::OutputEffectRecorded(output) => match output.status() {
                    OutputEffectStatus::Simulated => TimelineResult::Simulated,
                    OutputEffectStatus::Rejected => TimelineResult::Rejected,
                    OutputEffectStatus::Skipped => TimelineResult::Skipped,
                },
                EffectResult::OutputGateClosed => TimelineResult::Completed,
                EffectResult::PlanGenerated(_) => TimelineResult::Accepted,
            },
            _ => match reason {
                DecisionReason::PhraseExecutionScheduled => TimelineResult::Scheduled,
                DecisionReason::StaleObservationIgnored
                | DecisionReason::ObservationTimeRegressed
                | DecisionReason::TrackLoadMismatch
                | DecisionReason::PositionRegressed
                | DecisionReason::DuplicateCommandIgnored
                | DecisionReason::DuplicateEffectIgnored
                | DecisionReason::StalePlanIgnored
                | DecisionReason::PlanTrackLoadMismatch
                | DecisionReason::PlanActivationSkipped
                | DecisionReason::PhraseExecutionSkipped => TimelineResult::Ignored,
                _ => TimelineResult::Accepted,
            },
        };
        Self {
            sequence,
            occurred_at: event.monotonic_time(),
            source,
            event_type,
            result,
            reason,
        }
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn occurred_at(&self) -> MonotonicTime {
        self.occurred_at
    }

    #[must_use]
    pub const fn source(&self) -> TimelineSource {
        self.source
    }

    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        self.event_type
    }

    #[must_use]
    pub const fn result(&self) -> TimelineResult {
        self.result
    }

    #[must_use]
    pub const fn reason(&self) -> DecisionReason {
        self.reason
    }
}
