use crate::{
    CueId, CueReason, DeckId, MonotonicTime, OutputCommandId, PlanId, PlanRevision,
    SemanticLightingAction, TrackLoadId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputExecutionRequest {
    command_id: OutputCommandId,
    plan_id: PlanId,
    plan_revision: PlanRevision,
    deck_id: DeckId,
    track_load_id: TrackLoadId,
    phrase_index: u16,
    cue_id: CueId,
    action: SemanticLightingAction,
    cue_reason: CueReason,
    scheduled_at: MonotonicTime,
}

impl OutputExecutionRequest {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        command_id: OutputCommandId,
        plan_id: PlanId,
        plan_revision: PlanRevision,
        deck_id: DeckId,
        track_load_id: TrackLoadId,
        phrase_index: u16,
        cue_id: CueId,
        action: SemanticLightingAction,
        cue_reason: CueReason,
        scheduled_at: MonotonicTime,
    ) -> Self {
        Self {
            command_id,
            plan_id,
            plan_revision,
            deck_id,
            track_load_id,
            phrase_index,
            cue_id,
            action,
            cue_reason,
            scheduled_at,
        }
    }

    #[must_use]
    pub const fn command_id(&self) -> OutputCommandId {
        self.command_id
    }
    #[must_use]
    pub const fn plan_id(&self) -> PlanId {
        self.plan_id
    }
    #[must_use]
    pub const fn plan_revision(&self) -> PlanRevision {
        self.plan_revision
    }
    #[must_use]
    pub const fn deck_id(&self) -> DeckId {
        self.deck_id
    }
    #[must_use]
    pub const fn track_load_id(&self) -> TrackLoadId {
        self.track_load_id
    }
    #[must_use]
    pub const fn phrase_index(&self) -> u16 {
        self.phrase_index
    }
    #[must_use]
    pub const fn cue_id(&self) -> CueId {
        self.cue_id
    }
    #[must_use]
    pub const fn action(&self) -> &SemanticLightingAction {
        &self.action
    }
    #[must_use]
    pub const fn cue_reason(&self) -> CueReason {
        self.cue_reason
    }
    #[must_use]
    pub const fn scheduled_at(&self) -> MonotonicTime {
        self.scheduled_at
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputEffectStatus {
    Simulated,
    Rejected,
    Skipped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputEffectReason {
    PhraseBoundary,
    ProviderRejected,
    StaleExecutionContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputEffectResult {
    request: OutputExecutionRequest,
    actual_at: MonotonicTime,
    status: OutputEffectStatus,
    reason: OutputEffectReason,
}

impl OutputEffectResult {
    #[must_use]
    pub const fn new(
        request: OutputExecutionRequest,
        actual_at: MonotonicTime,
        status: OutputEffectStatus,
        reason: OutputEffectReason,
    ) -> Self {
        Self {
            request,
            actual_at,
            status,
            reason,
        }
    }

    #[must_use]
    pub const fn request(&self) -> &OutputExecutionRequest {
        &self.request
    }
    #[must_use]
    pub const fn actual_at(&self) -> MonotonicTime {
        self.actual_at
    }
    #[must_use]
    pub const fn status(&self) -> OutputEffectStatus {
        self.status
    }
    #[must_use]
    pub const fn reason(&self) -> OutputEffectReason {
        self.reason
    }
}
