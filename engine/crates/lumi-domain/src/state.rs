use std::collections::{BTreeMap, VecDeque};

use crate::{
    ClientId, CommandSequence, DecisionReason, DeckId, DeckSourceStatus, Diagnostic,
    EffectSequence, LightingPlan, MonotonicTime, OutputEffectResult, SourceId, SourceSequence,
    StateRevision, TimelineEntry, TrackId, TrackLoadId, TrackMetadata, WorkerId,
};

const MAXIMUM_DIAGNOSTICS: usize = 64;
const MAXIMUM_OUTPUT_EFFECTS: usize = 256;
const MAXIMUM_TIMELINE_ENTRIES: usize = 256;

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
    metadata: TrackMetadata,
    track_load_id: TrackLoadId,
    pub(crate) beat: u32,
    pub(crate) effective_bpm_milli: u32,
    pub(crate) playing: bool,
    pub(crate) phrase_index: Option<u16>,
    pub(crate) last_observed_at: MonotonicTime,
}

impl DeckState {
    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.metadata.id()
    }

    #[must_use]
    pub const fn metadata(&self) -> &TrackMetadata {
        &self.metadata
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
    pub const fn effective_bpm_milli(&self) -> u32 {
        self.effective_bpm_milli
    }

    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.playing
    }

    #[must_use]
    pub const fn phrase_index(&self) -> Option<u16> {
        self.phrase_index
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
    pub(crate) source_statuses: BTreeMap<SourceId, DeckSourceStatus>,
    pub(crate) leader_deck: Option<DeckId>,
    pub(crate) active_plan: Option<LightingPlan>,
    pub(crate) output_command_sequence: u64,
    pub(crate) output_effects: VecDeque<OutputEffectResult>,
    pub(crate) source_sequences: BTreeMap<SourceId, SourceSequence>,
    pub(crate) source_times: BTreeMap<SourceId, MonotonicTime>,
    pub(crate) command_sequences: BTreeMap<ClientId, CommandSequence>,
    pub(crate) effect_sequences: BTreeMap<WorkerId, EffectSequence>,
    pub(crate) processed_events: u64,
    pub(crate) last_decision: Option<DecisionReason>,
    pub(crate) diagnostics: VecDeque<Diagnostic>,
    pub(crate) timeline: VecDeque<TimelineEntry>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            revision: StateRevision::initial(),
            operation: OperationState::Off,
            health: RuntimeHealth::Starting,
            decks: BTreeMap::new(),
            plans: BTreeMap::new(),
            source_statuses: BTreeMap::new(),
            leader_deck: None,
            active_plan: None,
            output_command_sequence: 0,
            output_effects: VecDeque::new(),
            source_sequences: BTreeMap::new(),
            source_times: BTreeMap::new(),
            command_sequences: BTreeMap::new(),
            effect_sequences: BTreeMap::new(),
            processed_events: 0,
            last_decision: None,
            diagnostics: VecDeque::new(),
            timeline: VecDeque::new(),
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

    pub fn decks(&self) -> impl Iterator<Item = (DeckId, &DeckState)> {
        self.decks.iter().map(|(id, deck)| (*id, deck))
    }

    #[must_use]
    pub fn source_status(&self, source_id: SourceId) -> Option<DeckSourceStatus> {
        self.source_statuses.get(&source_id).copied()
    }

    pub fn source_statuses(&self) -> impl Iterator<Item = (SourceId, DeckSourceStatus)> + '_ {
        self.source_statuses
            .iter()
            .map(|(source_id, status)| (*source_id, *status))
    }

    #[must_use]
    pub const fn leader_deck(&self) -> Option<DeckId> {
        self.leader_deck
    }

    #[must_use]
    pub const fn active_plan(&self) -> Option<&LightingPlan> {
        self.active_plan.as_ref()
    }

    pub fn output_effects(&self) -> impl Iterator<Item = &OutputEffectResult> {
        self.output_effects.iter()
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

    pub fn timeline(&self) -> impl Iterator<Item = &TimelineEntry> {
        self.timeline.iter()
    }

    pub(crate) fn load_track(
        &mut self,
        deck_id: DeckId,
        metadata: TrackMetadata,
        track_load_id: TrackLoadId,
        observed_at: MonotonicTime,
    ) {
        let effective_bpm_milli = metadata.bpm_milli();
        self.decks.insert(
            deck_id,
            DeckState {
                metadata,
                track_load_id,
                beat: 0,
                effective_bpm_milli,
                playing: false,
                phrase_index: None,
                last_observed_at: observed_at,
            },
        );
        self.plans.remove(&deck_id);
        if self
            .active_plan
            .as_ref()
            .is_some_and(|plan| plan.deck_id() == deck_id)
        {
            self.active_plan = None;
        }
    }

    pub(crate) fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        if self.diagnostics.len() == MAXIMUM_DIAGNOSTICS {
            self.diagnostics.pop_front();
        }
        self.diagnostics.push_back(diagnostic);
    }

    pub(crate) fn push_output_effect(&mut self, result: OutputEffectResult) {
        if self.output_effects.len() == MAXIMUM_OUTPUT_EFFECTS {
            self.output_effects.pop_front();
        }
        self.output_effects.push_back(result);
    }

    pub(crate) fn push_timeline(&mut self, entry: TimelineEntry) {
        if self.timeline.len() == MAXIMUM_TIMELINE_ENTRIES {
            self.timeline.pop_front();
        }
        self.timeline.push_back(entry);
    }
}
