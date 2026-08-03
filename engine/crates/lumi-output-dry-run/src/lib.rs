//! Deterministic no-I/O lighting output adapter used by the Epic 1 demo.

#![forbid(unsafe_code)]

mod transcript;

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use lumi_domain::{
    MonotonicTime, OutputEffectReason, OutputEffectResult, OutputEffectStatus,
    OutputExecutionRequest,
};
use lumi_lighting_output::{LightingOutputCapabilities, LightingOutputProvider};

pub use transcript::canonical_output_transcript;

const DEFAULT_RECORD_CAPACITY: usize = 256;

pub struct DryRunLightingOutputProvider {
    capacity: usize,
    records: VecDeque<OutputEffectResult>,
}

impl Default for DryRunLightingOutputProvider {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_RECORD_CAPACITY,
            records: VecDeque::with_capacity(DEFAULT_RECORD_CAPACITY),
        }
    }
}

impl DryRunLightingOutputProvider {
    pub fn try_new(capacity: usize) -> Result<Self, DryRunOutputError> {
        if capacity == 0 {
            return Err(DryRunOutputError::InvalidCapacity);
        }
        Ok(Self {
            capacity,
            records: VecDeque::with_capacity(capacity),
        })
    }

    pub fn records(&self) -> impl Iterator<Item = &OutputEffectResult> {
        self.records.iter()
    }
}

impl LightingOutputProvider for DryRunLightingOutputProvider {
    type Error = DryRunOutputError;

    fn provider_kind(&self) -> &'static str {
        "dryRun"
    }

    fn capabilities(&self) -> LightingOutputCapabilities {
        LightingOutputCapabilities {
            supports_apply_look: true,
            supports_hold_current_look: true,
        }
    }

    fn execute(
        &mut self,
        request: &OutputExecutionRequest,
        actual_at: MonotonicTime,
    ) -> Result<OutputEffectResult, Self::Error> {
        let result = OutputEffectResult::new(
            request.clone(),
            actual_at,
            OutputEffectStatus::Simulated,
            OutputEffectReason::PhraseBoundary,
        );
        if self.records.len() == self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(result.clone());
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DryRunOutputError {
    InvalidCapacity,
}

impl fmt::Display for DryRunOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity => {
                formatter.write_str("dry-run output record capacity must be greater than zero")
            }
        }
    }
}

impl Error for DryRunOutputError {}
