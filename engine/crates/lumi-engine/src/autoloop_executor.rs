use lumi_domain::{DeckId, OutputExecutionRequest, PlanRevision, TrackLoadId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AutoloopTarget {
    pub(crate) bank_number: u8,
    pub(crate) autoloop_number: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AutoloopExecutionIdentity {
    pub(crate) execution_epoch: u64,
    pub(crate) deck_id: DeckId,
    pub(crate) track_load_id: TrackLoadId,
    pub(crate) plan_revision: PlanRevision,
    pub(crate) phrase_index: u16,
}

impl AutoloopExecutionIdentity {
    pub(crate) fn from_request(request: &OutputExecutionRequest, execution_epoch: u64) -> Self {
        Self {
            execution_epoch,
            deck_id: request.deck_id(),
            track_load_id: request.track_load_id(),
            plan_revision: request.plan_revision(),
            phrase_index: request.phrase_index(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AutoloopSchedule {
    pub(crate) identity: AutoloopExecutionIdentity,
    pub(crate) target: AutoloopTarget,
    pub(crate) select_bank: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AutoloopExecutorState {
    Idle,
    Scheduled {
        identity: AutoloopExecutionIdentity,
        target: AutoloopTarget,
    },
    BankPrepared {
        identity: AutoloopExecutionIdentity,
        target: AutoloopTarget,
    },
    Triggered {
        identity: AutoloopExecutionIdentity,
        target: AutoloopTarget,
        expected_lane_emitted_count: u64,
    },
    Completed {
        identity: AutoloopExecutionIdentity,
        target: AutoloopTarget,
    },
}

impl AutoloopExecutorState {
    fn identity(self) -> Option<AutoloopExecutionIdentity> {
        match self {
            Self::Idle => None,
            Self::Scheduled { identity, .. }
            | Self::BankPrepared { identity, .. }
            | Self::Triggered { identity, .. }
            | Self::Completed { identity, .. } => Some(identity),
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Scheduled { .. } => "scheduled",
            Self::BankPrepared { .. } => "bankPrepared",
            Self::Triggered { .. } => "triggered",
            Self::Completed { .. } => "completed",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AutoloopCueExecutor {
    state: AutoloopExecutorState,
    execution_epoch: u64,
    requested_count: u64,
    bank_prepared_count: u64,
    triggered_count: u64,
    completed_count: u64,
    duplicate_count: u64,
    cancelled_count: u64,
    failed_count: u64,
}

impl Default for AutoloopCueExecutor {
    fn default() -> Self {
        Self {
            state: AutoloopExecutorState::Idle,
            execution_epoch: 0,
            requested_count: 0,
            bank_prepared_count: 0,
            triggered_count: 0,
            completed_count: 0,
            duplicate_count: 0,
            cancelled_count: 0,
            failed_count: 0,
        }
    }
}

impl AutoloopCueExecutor {
    pub(crate) fn begin_execution_epoch(&mut self) -> Option<u64> {
        self.execution_epoch = self.execution_epoch.checked_add(1)?;
        self.cancel_pending();
        Some(self.execution_epoch)
    }

    pub(crate) fn schedule(
        &mut self,
        request: &OutputExecutionRequest,
        target: AutoloopTarget,
        active_bank: Option<u8>,
    ) -> Option<AutoloopSchedule> {
        self.requested_count = self.requested_count.saturating_add(1);
        let identity = AutoloopExecutionIdentity::from_request(request, self.execution_epoch);
        if self.state.identity() == Some(identity) {
            self.duplicate_count = self.duplicate_count.saturating_add(1);
            return None;
        }
        self.state = AutoloopExecutorState::Scheduled { identity, target };
        Some(AutoloopSchedule {
            identity,
            target,
            select_bank: active_bank != Some(target.bank_number),
        })
    }

    pub(crate) fn mark_bank_prepared(&mut self, schedule: AutoloopSchedule) {
        if self.state
            == (AutoloopExecutorState::Scheduled {
                identity: schedule.identity,
                target: schedule.target,
            })
        {
            self.state = AutoloopExecutorState::BankPrepared {
                identity: schedule.identity,
                target: schedule.target,
            };
            self.bank_prepared_count = self.bank_prepared_count.saturating_add(1);
        }
    }

    pub(crate) fn mark_triggered(
        &mut self,
        schedule: AutoloopSchedule,
        expected_lane_emitted_count: u64,
    ) {
        if self.state.identity() == Some(schedule.identity) {
            self.state = AutoloopExecutorState::Triggered {
                identity: schedule.identity,
                target: schedule.target,
                expected_lane_emitted_count,
            };
            self.triggered_count = self.triggered_count.saturating_add(1);
        }
    }

    pub(crate) fn complete_if_emitted(&mut self, lane_emitted_count: u64) {
        let AutoloopExecutorState::Triggered {
            identity,
            target,
            expected_lane_emitted_count,
        } = self.state
        else {
            return;
        };
        if lane_emitted_count >= expected_lane_emitted_count {
            self.state = AutoloopExecutorState::Completed { identity, target };
            self.completed_count = self.completed_count.saturating_add(1);
        }
    }

    pub(crate) fn fail(&mut self, identity: AutoloopExecutionIdentity) {
        if self.state.identity() == Some(identity) {
            self.state = AutoloopExecutorState::Idle;
            self.failed_count = self.failed_count.saturating_add(1);
        }
    }

    pub(crate) fn cancel_pending(&mut self) {
        if matches!(
            self.state,
            AutoloopExecutorState::Scheduled { .. }
                | AutoloopExecutorState::BankPrepared { .. }
                | AutoloopExecutorState::Triggered { .. }
        ) {
            self.cancelled_count = self.cancelled_count.saturating_add(1);
        }
        self.state = AutoloopExecutorState::Idle;
    }

    pub(crate) const fn state(&self) -> AutoloopExecutorState {
        self.state
    }

    pub(crate) const fn execution_epoch(&self) -> u64 {
        self.execution_epoch
    }

    pub(crate) const fn requested_count(&self) -> u64 {
        self.requested_count
    }

    pub(crate) const fn bank_prepared_count(&self) -> u64 {
        self.bank_prepared_count
    }

    pub(crate) const fn triggered_count(&self) -> u64 {
        self.triggered_count
    }

    pub(crate) const fn completed_count(&self) -> u64 {
        self.completed_count
    }

    pub(crate) const fn duplicate_count(&self) -> u64 {
        self.duplicate_count
    }

    pub(crate) const fn cancelled_count(&self) -> u64 {
        self.cancelled_count
    }

    pub(crate) const fn failed_count(&self) -> u64 {
        self.failed_count
    }
}

#[cfg(test)]
mod tests {
    use lumi_domain::{
        CueId, CueReason, LightingLook, LoopSelection, MonotonicTime, OutputCommandId,
        OutputExecutionRequest, PhraseKind, PlanId, PlanRevision, SceneCategory, SceneId,
        SemanticLightingAction, ThemeId, TrackLoadId,
    };

    use super::{AutoloopCueExecutor, AutoloopExecutorState, AutoloopTarget};

    fn request(phrase_index: u16, plan_revision: u64) -> OutputExecutionRequest {
        OutputExecutionRequest::new(
            OutputCommandId::new(u64::from(phrase_index) + 1),
            PlanId::new(1),
            PlanRevision::new(plan_revision),
            lumi_domain::DeckId::new(1),
            TrackLoadId::new(7),
            phrase_index,
            CueId::new(u64::from(phrase_index) + 1),
            SemanticLightingAction::ApplyLook(
                LightingLook::try_new(
                    ThemeId::new(1),
                    "Blue Pink".to_owned(),
                    SceneId::new(u64::from(phrase_index) + 1),
                    "AutoLoop".to_owned(),
                    SceneCategory::Ambient,
                    LoopSelection::new(
                        1,
                        u8::try_from(phrase_index.saturating_add(1)).unwrap_or(1),
                    ),
                )
                .unwrap_or_else(|error| panic!("test look must be valid: {error}")),
            ),
            CueReason::PhraseCategoryMatched {
                phrase_kind: PhraseKind::Intro,
                category: SceneCategory::Ambient,
            },
            MonotonicTime::new(0),
        )
    }

    #[test]
    fn one_identity_schedules_once_and_completes_once() {
        let mut executor = AutoloopCueExecutor::default();
        assert_eq!(executor.begin_execution_epoch(), Some(1));
        let request = request(0, 1);
        let target = AutoloopTarget {
            bank_number: 1,
            autoloop_number: 6,
        };
        let schedule = executor
            .schedule(&request, target, None)
            .unwrap_or_else(|| panic!("first cue must schedule"));
        assert!(schedule.select_bank);
        assert!(executor.schedule(&request, target, None).is_none());
        executor.mark_bank_prepared(schedule);
        assert!(matches!(
            executor.state(),
            AutoloopExecutorState::BankPrepared { .. }
        ));
        executor.mark_triggered(schedule, 2);
        executor.complete_if_emitted(1);
        assert!(matches!(
            executor.state(),
            AutoloopExecutorState::Triggered { .. }
        ));
        executor.complete_if_emitted(2);
        assert!(matches!(
            executor.state(),
            AutoloopExecutorState::Completed { .. }
        ));
        assert_eq!(executor.requested_count(), 2);
        assert_eq!(executor.triggered_count(), 1);
        assert_eq!(executor.completed_count(), 1);
        assert_eq!(executor.duplicate_count(), 1);
    }

    #[test]
    fn new_start_epoch_reasserts_the_same_phrase() {
        let mut executor = AutoloopCueExecutor::default();
        let request = request(3, 2);
        let target = AutoloopTarget {
            bank_number: 2,
            autoloop_number: 9,
        };
        assert_eq!(executor.begin_execution_epoch(), Some(1));
        let first = executor
            .schedule(&request, target, Some(2))
            .unwrap_or_else(|| panic!("first Start must schedule"));
        assert!(!first.select_bank);
        executor.mark_bank_prepared(first);
        executor.mark_triggered(first, 1);
        executor.complete_if_emitted(1);

        assert_eq!(executor.begin_execution_epoch(), Some(2));
        assert!(executor.schedule(&request, target, Some(2)).is_some());
        assert_eq!(executor.triggered_count(), 1);
    }

    #[test]
    fn phrase_and_plan_revision_create_new_identities_without_duplicates() {
        let mut executor = AutoloopCueExecutor::default();
        assert_eq!(executor.begin_execution_epoch(), Some(1));
        let target = AutoloopTarget {
            bank_number: 1,
            autoloop_number: 1,
        };
        let first = executor
            .schedule(&request(0, 1), target, Some(1))
            .unwrap_or_else(|| panic!("first phrase must schedule"));
        executor.mark_bank_prepared(first);
        executor.mark_triggered(first, 1);
        executor.complete_if_emitted(1);
        assert!(executor.schedule(&request(1, 1), target, Some(1)).is_some());
        assert!(executor.schedule(&request(1, 2), target, Some(1)).is_some());
        assert_eq!(executor.duplicate_count(), 0);
    }
}
