use std::error::Error;
use std::fmt;

use crate::{
    DeckObservation, DomainEvent, DomainEventKind, EffectResult, MonotonicTime, OperationCommand,
    OperationState, PlanRevision, RuntimeHealth, RuntimeState, StateRevision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionReason {
    RuntimeInitialized,
    SourceStatusAccepted,
    TrackLoadAccepted,
    PositionAdvanced,
    PhraseChanged,
    LeaderChanged,
    TrackUnloaded,
    StaleObservationIgnored,
    ObservationTimeRegressed,
    TrackLoadMismatch,
    PositionRegressed,
    DuplicateCommandIgnored,
    OperationTransitionAccepted,
    DuplicateEffectIgnored,
    PlanAccepted,
    StalePlanIgnored,
    PlanTrackLoadMismatch,
    OutputGateConfirmedClosed,
    QueueSaturated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    Information,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub sequence: u64,
    pub severity: DiagnosticSeverity,
    pub reason: DecisionReason,
    pub event_kind: DomainEventKind,
    pub occurred_at: MonotonicTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    EnsureOutputClosed { reason: DecisionReason },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reduction {
    state: RuntimeState,
    decision: DecisionReason,
    effects: Vec<Effect>,
    state_changed: bool,
}

impl Reduction {
    #[must_use]
    pub const fn state(&self) -> &RuntimeState {
        &self.state
    }

    #[must_use]
    pub const fn decision(&self) -> DecisionReason {
        self.decision
    }

    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    #[must_use]
    pub const fn state_changed(&self) -> bool {
        self.state_changed
    }

    pub(crate) fn into_parts(self) -> (RuntimeState, DecisionReason, Vec<Effect>) {
        (self.state, self.decision, self.effects)
    }
}

pub fn reduce(state: &RuntimeState, event: &DomainEvent) -> Result<Reduction, ReducerError> {
    let mut next = state.clone();
    let (decision, effects, state_changed) = match event {
        DomainEvent::RuntimeStarted { .. } => reduce_runtime_started(&mut next)?,
        DomainEvent::Observation(observation) => reduce_observation(&mut next, observation),
        DomainEvent::UserCommand(command) => reduce_command(&mut next, command)?,
        DomainEvent::EffectResult(result) => reduce_effect_result(&mut next, result)?,
        DomainEvent::QueueOverloaded(overload) => {
            next.health = RuntimeHealth::Degraded;
            if next.operation == OperationState::Live {
                next.operation = OperationState::Paused;
            }
            next.push_diagnostic(Diagnostic {
                sequence: next.processed_events.saturating_add(1),
                severity: DiagnosticSeverity::Error,
                reason: DecisionReason::QueueSaturated,
                event_kind: event.kind(),
                occurred_at: overload.occurred_at,
            });
            (
                DecisionReason::QueueSaturated,
                vec![Effect::EnsureOutputClosed {
                    reason: DecisionReason::QueueSaturated,
                }],
                true,
            )
        }
    };

    next.processed_events = next
        .processed_events
        .checked_add(1)
        .ok_or(ReducerError::ProcessedEventOverflow)?;
    next.last_decision = Some(decision);
    if state_changed {
        next.revision = next
            .revision
            .checked_next()
            .ok_or(ReducerError::StateRevisionOverflow)?;
    }

    Ok(Reduction {
        state: next,
        decision,
        effects,
        state_changed,
    })
}

fn reduce_runtime_started(
    state: &mut RuntimeState,
) -> Result<(DecisionReason, Vec<Effect>, bool), ReducerError> {
    if state.health != RuntimeHealth::Starting {
        return Err(ReducerError::RuntimeAlreadyStarted);
    }
    state.health = RuntimeHealth::Ready;
    Ok((DecisionReason::RuntimeInitialized, Vec::new(), true))
}

fn reduce_observation(
    state: &mut RuntimeState,
    event: &crate::ObservationEnvelope,
) -> (DecisionReason, Vec<Effect>, bool) {
    if state
        .source_sequences
        .get(&event.source_id)
        .is_some_and(|last| event.sequence <= *last)
    {
        return (DecisionReason::StaleObservationIgnored, Vec::new(), false);
    }
    state
        .source_sequences
        .insert(event.source_id, event.sequence);

    if state
        .source_times
        .get(&event.source_id)
        .is_some_and(|last| event.observed_at < *last)
    {
        return (DecisionReason::ObservationTimeRegressed, Vec::new(), false);
    }
    state
        .source_times
        .insert(event.source_id, event.observed_at);

    let decision = match &event.observation {
        DeckObservation::SourceStatusChanged { status } => {
            state.source_statuses.insert(event.source_id, *status);
            DecisionReason::SourceStatusAccepted
        }
        DeckObservation::TrackLoaded {
            deck_id,
            metadata,
            track_load_id,
        } => {
            state.load_track(
                *deck_id,
                metadata.clone(),
                *track_load_id,
                event.observed_at,
            );
            DecisionReason::TrackLoadAccepted
        }
        DeckObservation::PlaybackPosition {
            deck_id,
            track_load_id,
            beat,
        } => match state.decks.get_mut(deck_id) {
            Some(deck) if deck.track_load_id() != *track_load_id => {
                DecisionReason::TrackLoadMismatch
            }
            Some(deck) if *beat < deck.beat() => DecisionReason::PositionRegressed,
            Some(deck) => {
                deck.beat = *beat;
                deck.last_observed_at = event.observed_at;
                DecisionReason::PositionAdvanced
            }
            None => DecisionReason::TrackLoadMismatch,
        },
        DeckObservation::TrackUnloaded {
            deck_id,
            track_load_id,
        } => match state.decks.get(deck_id) {
            Some(deck) if deck.track_load_id() == *track_load_id => {
                state.decks.remove(deck_id);
                state.plans.remove(deck_id);
                if state.leader_deck == Some(*deck_id) {
                    state.leader_deck = None;
                }
                DecisionReason::TrackUnloaded
            }
            _ => DecisionReason::TrackLoadMismatch,
        },
        DeckObservation::PhraseChanged {
            deck_id,
            track_load_id,
            phrase_index,
        } => match state.decks.get_mut(deck_id) {
            Some(deck)
                if deck.track_load_id() == *track_load_id
                    && deck.metadata().phrase(*phrase_index).is_some() =>
            {
                deck.phrase_index = Some(*phrase_index);
                deck.last_observed_at = event.observed_at;
                DecisionReason::PhraseChanged
            }
            _ => DecisionReason::TrackLoadMismatch,
        },
        DeckObservation::LeaderChanged {
            deck_id,
            track_load_id,
        } => match state.decks.get(deck_id) {
            Some(deck) if deck.track_load_id() == *track_load_id => {
                state.leader_deck = Some(*deck_id);
                DecisionReason::LeaderChanged
            }
            _ => DecisionReason::TrackLoadMismatch,
        },
    };

    let state_changed = matches!(
        decision,
        DecisionReason::SourceStatusAccepted
            | DecisionReason::TrackLoadAccepted
            | DecisionReason::PositionAdvanced
            | DecisionReason::TrackUnloaded
            | DecisionReason::PhraseChanged
            | DecisionReason::LeaderChanged
    );
    (decision, Vec::new(), state_changed)
}

fn reduce_command(
    state: &mut RuntimeState,
    event: &crate::UserCommandEnvelope,
) -> Result<(DecisionReason, Vec<Effect>, bool), ReducerError> {
    if state
        .command_sequences
        .get(&event.client_id)
        .is_some_and(|last| event.sequence <= *last)
    {
        return Ok((DecisionReason::DuplicateCommandIgnored, Vec::new(), false));
    }
    if event.expected_state_revision != state.revision {
        return Err(ReducerError::RevisionConflict {
            expected: event.expected_state_revision,
            actual: state.revision,
        });
    }

    let target = match event.command {
        OperationCommand::Arm if state.operation == OperationState::Off => OperationState::Armed,
        OperationCommand::Start
            if matches!(
                state.operation,
                OperationState::Armed | OperationState::Paused
            ) =>
        {
            OperationState::Live
        }
        OperationCommand::Pause if state.operation == OperationState::Live => {
            OperationState::Paused
        }
        OperationCommand::Off => OperationState::Off,
        command => {
            return Err(ReducerError::InvalidOperationTransition {
                from: state.operation,
                command,
            });
        }
    };

    state
        .command_sequences
        .insert(event.client_id, event.sequence);
    state.operation = target;
    Ok((
        DecisionReason::OperationTransitionAccepted,
        Vec::new(),
        true,
    ))
}

fn reduce_effect_result(
    state: &mut RuntimeState,
    event: &crate::EffectResultEnvelope,
) -> Result<(DecisionReason, Vec<Effect>, bool), ReducerError> {
    if state
        .effect_sequences
        .get(&event.worker_id)
        .is_some_and(|last| event.sequence <= *last)
    {
        return Ok((DecisionReason::DuplicateEffectIgnored, Vec::new(), false));
    }
    state
        .effect_sequences
        .insert(event.worker_id, event.sequence);

    match &event.result {
        EffectResult::PlanGenerated(plan) => {
            let Some(deck) = state.decks.get(&plan.deck_id()) else {
                return Ok((DecisionReason::PlanTrackLoadMismatch, Vec::new(), false));
            };
            if deck.track_load_id() != plan.track_load_id() || deck.track_id() != plan.track_id() {
                return Ok((DecisionReason::PlanTrackLoadMismatch, Vec::new(), false));
            }

            let expected_revision = match state.plans.get(&plan.deck_id()) {
                Some(current) => current
                    .revision()
                    .checked_next()
                    .ok_or(ReducerError::PlanRevisionOverflow)?,
                None => PlanRevision::initial(),
            };
            if plan.revision() < expected_revision {
                return Ok((DecisionReason::StalePlanIgnored, Vec::new(), false));
            }
            if plan.revision() != expected_revision {
                return Err(ReducerError::PlanRevisionGap {
                    expected: expected_revision,
                    actual: plan.revision(),
                });
            }

            state.plans.insert(plan.deck_id(), plan.clone());
            Ok((DecisionReason::PlanAccepted, Vec::new(), true))
        }
        EffectResult::OutputGateClosed => {
            Ok((DecisionReason::OutputGateConfirmedClosed, Vec::new(), false))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReducerError {
    RuntimeAlreadyStarted,
    RevisionConflict {
        expected: StateRevision,
        actual: StateRevision,
    },
    InvalidOperationTransition {
        from: OperationState,
        command: OperationCommand,
    },
    PlanRevisionGap {
        expected: PlanRevision,
        actual: PlanRevision,
    },
    StateRevisionOverflow,
    PlanRevisionOverflow,
    ProcessedEventOverflow,
}

impl fmt::Display for ReducerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeAlreadyStarted => formatter.write_str("the runtime is already started"),
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "state revision conflict: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::InvalidOperationTransition { from, command } => {
                write!(
                    formatter,
                    "operation command {command:?} is invalid from {from:?}"
                )
            }
            Self::PlanRevisionGap { expected, actual } => write!(
                formatter,
                "plan revision gap: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::StateRevisionOverflow => formatter.write_str("state revision overflow"),
            Self::PlanRevisionOverflow => formatter.write_str("plan revision overflow"),
            Self::ProcessedEventOverflow => formatter.write_str("processed event counter overflow"),
        }
    }
}

impl Error for ReducerError {}
