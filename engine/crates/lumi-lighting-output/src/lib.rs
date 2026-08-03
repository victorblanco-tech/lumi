//! Provider-neutral port for executing validated semantic lighting actions.

#![forbid(unsafe_code)]

use std::error::Error;

use lumi_domain::{MonotonicTime, OutputEffectResult, OutputExecutionRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightingOutputCapabilities {
    pub supports_apply_look: bool,
    pub supports_hold_current_look: bool,
}

pub trait LightingOutputProvider {
    type Error: Error + Send + Sync + 'static;

    fn provider_kind(&self) -> &'static str;
    fn capabilities(&self) -> LightingOutputCapabilities;
    fn execute(
        &mut self,
        request: &OutputExecutionRequest,
        actual_at: MonotonicTime,
    ) -> Result<OutputEffectResult, Self::Error>;
}
