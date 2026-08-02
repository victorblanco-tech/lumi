use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use crate::{DomainEvent, DomainEventKind, QueueOverloadEvent};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedEventQueue {
    capacity: usize,
    events: VecDeque<DomainEvent>,
    pending_overload: Option<QueueOverloadEvent>,
}

impl BoundedEventQueue {
    pub fn try_new(capacity: usize) -> Result<Self, InvalidQueueCapacity> {
        if capacity == 0 {
            return Err(InvalidQueueCapacity);
        }
        Ok(Self {
            capacity,
            events: VecDeque::with_capacity(capacity),
            pending_overload: None,
        })
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len() + usize::from(self.pending_overload.is_some())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.pending_overload.is_none()
    }

    pub fn push(&mut self, event: DomainEvent) -> Result<IngressOutcome, IngressError> {
        if self.events.len() < self.capacity {
            self.events.push_back(event);
            return Ok(IngressOutcome::Accepted);
        }

        self.record_overload(&event);
        if event.is_critical()
            && let Some(index) = self.events.iter().rposition(|queued| !queued.is_critical())
        {
            let Some(evicted) = self.events.remove(index) else {
                return Err(IngressError::Saturated {
                    rejected_kind: event.kind(),
                    critical: true,
                });
            };
            let evicted_kind = evicted.kind();
            self.events.push_back(event);
            return Ok(IngressOutcome::AcceptedAfterEvictingNonCritical { evicted_kind });
        }

        Err(IngressError::Saturated {
            rejected_kind: event.kind(),
            critical: event.is_critical(),
        })
    }

    pub fn pop(&mut self) -> Option<DomainEvent> {
        self.pending_overload
            .take()
            .map(DomainEvent::QueueOverloaded)
            .or_else(|| self.events.pop_front())
    }

    fn record_overload(&mut self, event: &DomainEvent) {
        match &mut self.pending_overload {
            Some(overload) => {
                overload.rejected_kind = event.kind();
                overload.rejected_critical |= event.is_critical();
                overload.occurrences = overload.occurrences.saturating_add(1);
            }
            None => {
                self.pending_overload = Some(QueueOverloadEvent {
                    occurred_at: event.monotonic_time(),
                    rejected_kind: event.kind(),
                    rejected_critical: event.is_critical(),
                    occurrences: 1,
                });
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressOutcome {
    Accepted,
    AcceptedAfterEvictingNonCritical { evicted_kind: DomainEventKind },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressError {
    Saturated {
        rejected_kind: DomainEventKind,
        critical: bool,
    },
}

impl fmt::Display for IngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Saturated {
                rejected_kind,
                critical,
            } => write!(
                formatter,
                "event queue saturated; rejected {rejected_kind:?} (critical: {critical})"
            ),
        }
    }
}

impl Error for IngressError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidQueueCapacity;

impl fmt::Display for InvalidQueueCapacity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("event queue capacity must be greater than zero")
    }
}

impl Error for InvalidQueueCapacity {}
