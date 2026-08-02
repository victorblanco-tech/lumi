//! Deterministic, license-safe two-deck source adapter.

#![forbid(unsafe_code)]

mod clock;
mod fixture;
mod provider;
mod transcript;

pub use clock::{ManualClock, MonotonicClock};
pub use provider::{
    SimulationControl, SimulationSpeed, SimulatorCanonicalSnapshot, SimulatorDeckSourceProvider,
    SimulatorError,
};
pub use transcript::canonical_transcript;
