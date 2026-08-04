//! Pure Lumi domain model, deterministic reducer, and bounded serialized runtime.
//!
//! This crate deliberately has no I/O, serialization, async runtime, provider,
//! UI, or platform dependencies.

#![forbid(unsafe_code)]

mod event;
mod identifiers;
mod output;
mod plan;
mod queue;
mod reducer;
mod runtime;
mod state;
mod time;
mod timeline;
mod track;

pub use event::{
    DeckObservation, DomainEvent, DomainEventKind, EffectResult, EffectResultEnvelope,
    ObservationEnvelope, OperationCommand, QueueOverloadEvent, UserCommandEnvelope,
};
pub use identifiers::{
    ClientId, CommandSequence, CueId, DeckId, EffectId, EffectSequence, OutputCommandId,
    PlanConfigurationRevision, PlanId, PlanRevision, SceneId, SourceId, SourceSequence,
    StateRevision, ThemeId, TrackId, TrackLoadId, WorkerId,
};
pub use output::{
    OutputEffectReason, OutputEffectResult, OutputEffectStatus, OutputExecutionRequest,
};
pub use plan::{
    CueOrigin, CueReason, LightingCue, LightingLook, LightingPlan, LoopSelection, PlanStatus,
    PlanValidationError, SceneCategory, SemanticLightingAction, ThemeDecision,
    ThemeSelectionReason,
};
pub use queue::{BoundedEventQueue, IngressError, IngressOutcome, InvalidQueueCapacity};
pub use reducer::{
    DecisionReason, Diagnostic, DiagnosticSeverity, Effect, ReducerError, Reduction, reduce,
};
pub use runtime::{ProcessResult, SerializedRuntime, SerializedRuntimeError};
pub use state::{DeckState, OperationState, RuntimeHealth, RuntimeState};
pub use time::MonotonicTime;
pub use timeline::{TimelineEntry, TimelineResult, TimelineSource};
pub use track::{
    DeckSourceStatus, KeyMode, MusicalKey, PhraseKind, PitchClass, TrackColor, TrackIdentityFacts,
    TrackMetadata, TrackPhrase, TrackValidationError,
};
