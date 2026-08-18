use std::error::Error;
use std::fmt;

use crate::{
    DeckObservation, DomainEvent, DomainEventKind, EffectResult, MonotonicTime, OperationCommand,
    OperationState, OutputCommandId, OutputExecutionRequest, PlanRevision, PlanStatus,
    RuntimeHealth, RuntimeState, StateRevision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionReason {
    RuntimeInitialized,
    SourceStatusAccepted,
    TrackLoadAccepted,
    PositionAdvanced,
    PositionSeeked,
    PlaybackTempoChanged,
    PlaybackStateChanged,
    PhraseChanged,
    LeaderChanged,
    PlanActivated,
    PlanActivationSkipped,
    PhraseExecutionScheduled,
    PhraseExecutionSkipped,
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
    OutputEffectRecorded,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    EnsureOutputClosed { reason: DecisionReason },
    ExecuteCue(OutputExecutionRequest),
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
        DomainEvent::Observation(observation) => reduce_observation(&mut next, observation)?,
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
    next.push_timeline(crate::TimelineEntry::from_event(
        next.processed_events,
        event,
        decision,
    ));
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
) -> Result<(DecisionReason, Vec<Effect>, bool), ReducerError> {
    if state
        .source_sequences
        .get(&event.source_id)
        .is_some_and(|last| event.sequence <= *last)
    {
        return Ok((DecisionReason::StaleObservationIgnored, Vec::new(), false));
    }
    state
        .source_sequences
        .insert(event.source_id, event.sequence);

    if state
        .source_times
        .get(&event.source_id)
        .is_some_and(|last| event.observed_at < *last)
    {
        return Ok((DecisionReason::ObservationTimeRegressed, Vec::new(), false));
    }
    state
        .source_times
        .insert(event.source_id, event.observed_at);

    let mut effects = Vec::new();
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
        DeckObservation::PlaybackPositionSeeked {
            deck_id,
            track_load_id,
            beat,
        } => {
            let accepted = match state.decks.get_mut(deck_id) {
                Some(deck) if deck.track_load_id() != *track_load_id => None,
                Some(deck) => {
                    let phrase_index = phrase_index_at_beat(deck.metadata(), *beat);
                    deck.beat = *beat;
                    deck.phrase_index = phrase_index;
                    deck.last_observed_at = event.observed_at;
                    Some((deck.is_playing(), phrase_index))
                }
                None => None,
            };
            match accepted {
                None => DecisionReason::TrackLoadMismatch,
                Some((playing, phrase_index)) => {
                    // A confirmed landing starts a new transport execution
                    // context. Even when it lands inside the same phrase, the
                    // current SoundSwitch AutoLoop must be eligible exactly
                    // once; the following PhraseChanged observation is then a
                    // harmless duplicate.
                    if state
                        .last_scheduled_cue
                        .is_some_and(|scheduled| scheduled.0 == *deck_id)
                    {
                        state.last_scheduled_cue = None;
                    }
                    if state.operation == OperationState::Live
                        && state.leader_deck == Some(*deck_id)
                        && playing
                    {
                        if let Some(phrase_index) = phrase_index
                            && let Some(effect) = execution_effect(
                                state,
                                *deck_id,
                                *track_load_id,
                                phrase_index,
                                event.observed_at,
                            )?
                        {
                            effects.push(effect);
                            DecisionReason::PhraseExecutionScheduled
                        } else {
                            DecisionReason::PositionSeeked
                        }
                    } else {
                        DecisionReason::PositionSeeked
                    }
                }
            }
        }
        DeckObservation::PlaybackTempoChanged {
            deck_id,
            track_load_id,
            bpm_milli,
        } => match state.decks.get_mut(deck_id) {
            Some(deck)
                if deck.track_load_id() == *track_load_id
                    && (20_000..=300_000).contains(bpm_milli) =>
            {
                deck.effective_bpm_milli = *bpm_milli;
                deck.last_observed_at = event.observed_at;
                DecisionReason::PlaybackTempoChanged
            }
            _ => DecisionReason::TrackLoadMismatch,
        },
        DeckObservation::PlaybackStateChanged {
            deck_id,
            track_load_id,
            playing,
        } => {
            let accepted = match state.decks.get_mut(deck_id) {
                Some(deck) if deck.track_load_id() == *track_load_id => {
                    let started = !deck.is_playing() && *playing;
                    deck.playing = *playing;
                    deck.last_observed_at = event.observed_at;
                    Some((started, deck.phrase_index))
                }
                _ => None,
            };
            match accepted {
                None => DecisionReason::TrackLoadMismatch,
                Some((_, _)) if !playing => {
                    // A real deck transport stop closes the current playback
                    // generation. SoundSwitch may also have stopped its active
                    // AutoLoop, so the same Lumi cue must be eligible for the
                    // next stopped -> playing edge, including from a Hot Cue.
                    if state
                        .last_scheduled_cue
                        .is_some_and(|scheduled| scheduled.0 == *deck_id)
                    {
                        state.last_scheduled_cue = None;
                    }
                    DecisionReason::PlaybackStateChanged
                }
                Some((true, Some(phrase_index))) if state.operation == OperationState::Live => {
                    if let Some(effect) = execution_effect(
                        state,
                        *deck_id,
                        *track_load_id,
                        phrase_index,
                        event.observed_at,
                    )? {
                        effects.push(effect);
                        DecisionReason::PhraseExecutionScheduled
                    } else {
                        DecisionReason::PlaybackStateChanged
                    }
                }
                Some(_) => DecisionReason::PlaybackStateChanged,
            }
        }
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
                if state
                    .active_plan
                    .as_ref()
                    .is_some_and(|plan| plan.deck_id() == *deck_id)
                {
                    state.active_plan = None;
                }
                if state
                    .last_scheduled_cue
                    .is_some_and(|scheduled| scheduled.0 == *deck_id)
                {
                    state.last_scheduled_cue = None;
                }
                DecisionReason::TrackUnloaded
            }
            _ => DecisionReason::TrackLoadMismatch,
        },
        DeckObservation::PhraseChanged {
            deck_id,
            track_load_id,
            phrase_index,
        } => {
            let accepted = match state.decks.get_mut(deck_id) {
                Some(deck)
                    if deck.track_load_id() == *track_load_id
                        && deck.metadata().phrase(*phrase_index).is_some() =>
                {
                    deck.phrase_index = Some(*phrase_index);
                    deck.last_observed_at = event.observed_at;
                    true
                }
                _ => false,
            };
            if !accepted {
                DecisionReason::TrackLoadMismatch
            } else if state.operation != OperationState::Live
                || !state
                    .decks
                    .get(deck_id)
                    .is_some_and(|deck| deck.is_playing())
            {
                DecisionReason::PhraseChanged
            } else if let Some(effect) = execution_effect(
                state,
                *deck_id,
                *track_load_id,
                *phrase_index,
                event.observed_at,
            )? {
                effects.push(effect);
                DecisionReason::PhraseExecutionScheduled
            } else {
                DecisionReason::PhraseExecutionSkipped
            }
        }
        DeckObservation::LeaderChanged {
            deck_id,
            track_load_id,
        } => {
            let accepted = state.decks.get(deck_id).and_then(|deck| {
                (deck.track_load_id() == *track_load_id).then_some((
                    deck.is_playing(),
                    deck.phrase_index(),
                    deck.track_id(),
                ))
            });
            match accepted {
                Some((playing, phrase_index, track_id)) => {
                    state.leader_deck = Some(*deck_id);
                    state.active_plan = state
                        .plans
                        .get(deck_id)
                        .filter(|plan| {
                            plan.status() == PlanStatus::Ready
                                && plan.track_load_id() == *track_load_id
                                && plan.track_id() == track_id
                        })
                        .cloned();
                    if state.active_plan.is_some()
                        && state.operation == OperationState::Live
                        && playing
                    {
                        if let Some(phrase_index) = phrase_index
                            && let Some(effect) = execution_effect(
                                state,
                                *deck_id,
                                *track_load_id,
                                phrase_index,
                                event.observed_at,
                            )?
                        {
                            effects.push(effect);
                            DecisionReason::PhraseExecutionScheduled
                        } else {
                            DecisionReason::PlanActivated
                        }
                    } else if state.active_plan.is_some() {
                        DecisionReason::PlanActivated
                    } else {
                        DecisionReason::PlanActivationSkipped
                    }
                }
                None => {
                    state.active_plan = None;
                    DecisionReason::TrackLoadMismatch
                }
            }
        }
    };

    let state_changed = matches!(
        decision,
        DecisionReason::SourceStatusAccepted
            | DecisionReason::TrackLoadAccepted
            | DecisionReason::PositionAdvanced
            | DecisionReason::PositionSeeked
            | DecisionReason::PlaybackTempoChanged
            | DecisionReason::PlaybackStateChanged
            | DecisionReason::TrackUnloaded
            | DecisionReason::PhraseChanged
            | DecisionReason::PlanActivated
            | DecisionReason::PlanActivationSkipped
            | DecisionReason::PhraseExecutionScheduled
            | DecisionReason::PhraseExecutionSkipped
    );
    Ok((decision, effects, state_changed))
}

fn phrase_index_at_beat(metadata: &crate::TrackMetadata, beat: u32) -> Option<u16> {
    metadata
        .phrases()
        .iter()
        .find(|phrase| beat >= phrase.start_beat() && beat < phrase.end_beat())
        .map(|phrase| phrase.index())
}

fn execution_effect(
    state: &mut RuntimeState,
    deck_id: crate::DeckId,
    track_load_id: crate::TrackLoadId,
    phrase_index: u16,
    scheduled_at: MonotonicTime,
) -> Result<Option<Effect>, ReducerError> {
    if state.leader_deck != Some(deck_id) {
        return Ok(None);
    }
    let Some(plan) = state.active_plan.as_ref() else {
        return Ok(None);
    };
    if plan.deck_id() != deck_id || plan.track_load_id() != track_load_id {
        return Ok(None);
    }
    let Some(cue) = plan
        .cues()
        .get(usize::from(phrase_index))
        .filter(|cue| cue.phrase_index() == phrase_index)
    else {
        return Ok(None);
    };
    let execution_identity = (
        deck_id,
        track_load_id,
        phrase_index,
        cue.id(),
        plan.revision(),
    );
    if state.last_scheduled_cue == Some(execution_identity) {
        return Ok(None);
    }
    let command_sequence = state
        .output_command_sequence
        .checked_add(1)
        .ok_or(ReducerError::OutputCommandSequenceOverflow)?;
    let request = OutputExecutionRequest::new(
        OutputCommandId::new(command_sequence),
        plan.id(),
        plan.revision(),
        deck_id,
        track_load_id,
        phrase_index,
        cue.id(),
        cue.action().clone(),
        cue.reason(),
        scheduled_at,
    );
    state.output_command_sequence = command_sequence;
    state.last_scheduled_cue = Some(execution_identity);
    Ok(Some(Effect::ExecuteCue(request)))
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
    if matches!(target, OperationState::Paused | OperationState::Off) {
        // Pause and Off close the physical output gate, but the prepared plan
        // remains visible and ready. Starting again must restore the current
        // AutoLoop immediately; waiting for a later phrase boundary would leave
        // the show dark after an explicitly requested output pause.
        state.last_scheduled_cue = None;
    }
    let effects = if matches!(target, OperationState::Paused | OperationState::Off) {
        vec![Effect::EnsureOutputClosed {
            reason: DecisionReason::OperationTransitionAccepted,
        }]
    } else if target == OperationState::Live {
        let current = state.leader_deck.and_then(|deck_id| {
            let deck = state.decks.get(&deck_id)?;
            deck.is_playing()
                .then_some((deck_id, deck.track_load_id(), deck.phrase_index?))
        });
        current
            .map(|(deck_id, track_load_id, phrase_index)| {
                execution_effect(state, deck_id, track_load_id, phrase_index, event.issued_at)
            })
            .transpose()?
            .into_iter()
            .flatten()
            .collect()
    } else {
        Vec::new()
    };
    Ok((DecisionReason::OperationTransitionAccepted, effects, true))
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
            if state.leader_deck == Some(plan.deck_id()) {
                state.active_plan = Some(plan.clone());
            }
            Ok((DecisionReason::PlanAccepted, Vec::new(), true))
        }
        EffectResult::OutputEffectRecorded(result) => {
            state.push_output_effect(result.clone());
            Ok((DecisionReason::OutputEffectRecorded, Vec::new(), true))
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
    OutputCommandSequenceOverflow,
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
            Self::OutputCommandSequenceOverflow => {
                formatter.write_str("output command sequence overflow")
            }
            Self::ProcessedEventOverflow => formatter.write_str("processed event counter overflow"),
        }
    }
}

impl Error for ReducerError {}
