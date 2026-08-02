use std::error::Error;
use std::fmt;

use crate::{
    BoundedEventQueue, DecisionReason, DomainEvent, DomainEventKind, Effect, IngressError,
    IngressOutcome, InvalidQueueCapacity, ReducerError, RuntimeState, StateRevision, reduce,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializedRuntime {
    state: RuntimeState,
    ingress: BoundedEventQueue,
}

impl SerializedRuntime {
    pub fn try_new(queue_capacity: usize) -> Result<Self, InvalidQueueCapacity> {
        Ok(Self {
            state: RuntimeState::default(),
            ingress: BoundedEventQueue::try_new(queue_capacity)?,
        })
    }

    #[must_use]
    pub const fn state(&self) -> &RuntimeState {
        &self.state
    }

    #[must_use]
    pub const fn queue_capacity(&self) -> usize {
        self.ingress.capacity()
    }

    #[must_use]
    pub fn queue_depth(&self) -> usize {
        self.ingress.len()
    }

    pub fn submit(&mut self, event: DomainEvent) -> Result<IngressOutcome, IngressError> {
        self.ingress.push(event)
    }

    pub fn process_next(&mut self) -> Result<Option<ProcessResult>, ReducerError> {
        let Some(event) = self.ingress.pop() else {
            return Ok(None);
        };
        let event_kind = event.kind();
        let reduction = reduce(&self.state, &event)?;
        let (state, decision, effects) = reduction.into_parts();
        self.state = state;
        Ok(Some(ProcessResult {
            event_kind,
            state_revision: self.state.revision(),
            decision,
            effects,
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessResult {
    pub event_kind: DomainEventKind,
    pub state_revision: StateRevision,
    pub decision: DecisionReason,
    pub effects: Vec<Effect>,
}

#[derive(Debug)]
pub enum SerializedRuntimeError {
    InvalidCapacity(InvalidQueueCapacity),
    Ingress(IngressError),
    Reducer(ReducerError),
    StartupEventMissing,
}

impl fmt::Display for SerializedRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity(error) => error.fmt(formatter),
            Self::Ingress(error) => error.fmt(formatter),
            Self::Reducer(error) => error.fmt(formatter),
            Self::StartupEventMissing => {
                formatter.write_str("runtime startup event was not processed")
            }
        }
    }
}

impl Error for SerializedRuntimeError {}

impl From<InvalidQueueCapacity> for SerializedRuntimeError {
    fn from(error: InvalidQueueCapacity) -> Self {
        Self::InvalidCapacity(error)
    }
}

impl From<IngressError> for SerializedRuntimeError {
    fn from(error: IngressError) -> Self {
        Self::Ingress(error)
    }
}

impl From<ReducerError> for SerializedRuntimeError {
    fn from(error: ReducerError) -> Self {
        Self::Reducer(error)
    }
}
