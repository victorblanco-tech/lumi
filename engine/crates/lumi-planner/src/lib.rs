//! Deterministic, provider-neutral next-track lighting planner.

#![forbid(unsafe_code)]

mod planner;
mod transcript;

pub use planner::{
    ChoiceSource, DeterministicPlanner, PlannerError, PlannerTrack, PlanningConfiguration,
    PlanningInput, StableChoiceSource,
};
pub use transcript::{CanonicalPlanError, canonical_plan};
