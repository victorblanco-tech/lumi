//! Pure Lumi domain types and invariants.
//!
//! This crate deliberately has no I/O, serialization, runtime, provider, or
//! platform dependencies.

#![forbid(unsafe_code)]

/// Controls whether Lumi observes, plans, or may execute lighting output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OperationState {
    /// Sources, planning, and output are stopped.
    #[default]
    Off,
    /// Sources and planning are active; output is blocked.
    Armed,
    /// Sources, planning, and validated output are active.
    Live,
    /// Sources and planning continue; output is blocked.
    Paused,
}

impl OperationState {
    /// Returns whether the global output gate is open.
    #[must_use]
    pub const fn allows_output(self) -> bool {
        matches!(self, Self::Live)
    }
}

#[cfg(test)]
mod tests {
    use super::OperationState;

    #[test]
    fn only_live_allows_output() {
        assert!(!OperationState::Off.allows_output());
        assert!(!OperationState::Armed.allows_output());
        assert!(OperationState::Live.allows_output());
        assert!(!OperationState::Paused.allows_output());
    }
}
