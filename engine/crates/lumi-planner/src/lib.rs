//! Deterministic, provider-neutral next-track lighting planner.

#![forbid(unsafe_code)]

mod planner;
mod transcript;

pub use planner::{
    ChoiceSource, DeterministicPlanner, PlanMutationError, PlannerError, PlannerTrack,
    PlanningConfiguration, PlanningInput, PlanningOptions, SceneOption, StableChoiceSource,
    ThemeOption,
};
pub use transcript::{CanonicalPlanError, canonical_plan};
