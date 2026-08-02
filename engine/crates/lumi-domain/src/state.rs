use std::collections::{BTreeMap, VecDeque};

use crate::{
    ClientId, CommandSequence, DecisionReason, DeckId, Diagnostic, EffectSequence, LightingPlan,
    MonotonicTime, SourceId, SourceSequence, StateRevision, TrackId, TrackLoadId, WorkerId,
};

const MAXIMUM_DIAGNOSTICS: usize = 64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OperationState {
    #[default]
    Off,
    Armed,
    Live,
    Paused,
}

impl OperationState {
    #[must_use]
    pub const fn allows_output(self) -> bool {
        matches!(self, Self::Live)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeHealth {
    #[default]
    Starting,
    Ready,
    Degraded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckState {
    track_id: TrackId,
    track_load_id: TrackLoadId,
    pub(crate) beat: u32,
    pub(crate) last_observed_at: MonotonicTime,
}

impl DeckState {
    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.track_id
    }

    #[must_use]
    pub const fn track_load_id(&self) -> TrackLoadId {
        self.track_load_id
    }

    #[must_use]
    pub const fn beat(&self) -> u32 {
        self.beat
    }

    #[must_use]
    pub const fn last_observed_at(&self) -> MonotonicTime {
        self.last_observed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeState {
    pub(crate) revision: StateRevision,
    pub(crate) operation: OperationState,
    pub(crate) health: RuntimeHealth,
    pub(crate) decks: BTreeMap<DeckId, DeckState>,
    pub(crate) plans: BTreeMap<DeckId, LightingPlan>,
    pub(crate) source_sequences: BTreeMap<SourceId, SourceSequence>,
    pub(crate) source_times: BTreeMap<SourceId, MonotonicTime>,
    pub(crate) command_sequences: BTreeMap<ClientId, CommandSequence>,
    pub(crate) effect_sequences: BTreeMap<WorkerId, EffectSequence>,
    pub(crate) processed_events: u64,
    pub(crate) last_decision: Option<DecisionReason>,
    pub(crate) diagnostics: VecDeque<Diagnostic>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            revision: StateRevision::initial(),
            operation: OperationState::Off,
            health: RuntimeHealth::Starting,
            decks: BTreeMap::new(),
            plans: BTreeMap::new(),
            source_sequences: BTreeMap::new(),
            source_times: BTreeMap::new(),
            command_sequences: BTreeMap::new(),
            effect_sequences: BTreeMap::new(),
            processed_events: 0,
            last_decision: None,
            diagnostics: VecDeque::new(),
        }
    }
}

impl RuntimeState {
    #[must_use]
    pub const fn revision(&self) -> StateRevision {
        self.revision
    }

    #[must_use]
    pub const fn operation(&self) -> OperationState {
        self.operation
    }

    #[must_use]
    pub const fn health(&self) -> RuntimeHealth {
        self.health
    }

    #[must_use]
    pub fn deck(&self, deck_id: DeckId) -> Option<&DeckState> {
        self.decks.get(&deck_id)
    }

    #[must_use]
    pub fn plan(&self, deck_id: DeckId) -> Option<&LightingPlan> {
        self.plans.get(&deck_id)
    }

    #[must_use]
    pub const fn processed_events(&self) -> u64 {
        self.processed_events
    }

    #[must_use]
    pub const fn last_decision(&self) -> Option<DecisionReason> {
        self.last_decision
    }

    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    pub(crate) fn load_track(
        &mut self,
        deck_id: DeckId,
        track_id: TrackId,
        track_load_id: TrackLoadId,
        observed_at: MonotonicTime,
    ) {
        self.decks.insert(
            deck_id,
            DeckState {
                track_id,
                track_load_id,
                beat: 0,
                last_observed_at: observed_at,
            },
        );
        self.plans.remove(&deck_id);
    }

    pub(crate) fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        if self.diagnostics.len() == MAXIMUM_DIAGNOSTICS {
            self.diagnostics.pop_front();
        }
        self.diagnostics.push_back(diagnostic);
    }
}
