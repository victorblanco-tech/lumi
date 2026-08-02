//! Application-owned port for normalized deck-source adapters.

#![forbid(unsafe_code)]

use std::error::Error;

use lumi_domain::DomainEvent;

/// Provider-neutral event source implemented by simulator and future live adapters.
pub trait DeckSourceProvider {
    type Error: Error + Send + Sync + 'static;

    /// Stable provider kind for diagnostics, never a hardware-device identity.
    fn provider_kind(&self) -> &'static str;

    /// Drains normalized events accumulated since the previous call.
    fn drain_events(&mut self) -> Result<Vec<DomainEvent>, Self::Error>;
}
