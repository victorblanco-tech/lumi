//! Pure initial-show launch decision. No clock, MIDI, network or UI ownership.

use lumi_domain::{DeckId, PlanRevision, TrackLoadId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchPolicy {
    #[default]
    Immediate,
    OnPhraseStart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchContext {
    pub deck: DeckId,
    pub load: TrackLoadId,
    pub revision: PlanRevision,
    pub epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchBoundary {
    pub phrase: u16,
    pub beat: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LaunchState {
    #[default]
    WaitingForPlayback,
    Waiting {
        context: LaunchContext,
        target: LaunchBoundary,
    },
    NoUpcomingBoundary {
        context: LaunchContext,
    },
    Launched,
}

#[derive(Clone, Debug, Default)]
pub struct LaunchGate {
    state: LaunchState,
}

impl LaunchGate {
    pub fn state(&self) -> LaunchState {
        self.state
    }

    /// Called for a new Arm cycle, not for every phrase or hotcue.
    pub fn arm(&mut self) {
        self.state = LaunchState::WaitingForPlayback;
    }

    /// Cancels only an unsent initial launch. A running show must not start
    /// waiting for a phrase again merely because playback pauses or seeks.
    pub fn cancel_pending(&mut self) {
        if self.state != LaunchState::Launched {
            self.arm();
        }
    }

    pub fn observe(
        &mut self,
        policy: LaunchPolicy,
        context: LaunchContext,
        playing: bool,
        beat: u32,
        boundaries: impl IntoIterator<Item = LaunchBoundary>,
    ) {
        if self.state == LaunchState::Launched {
            return;
        }
        if !playing {
            self.cancel_pending();
            return;
        }
        if policy == LaunchPolicy::Immediate {
            return;
        }
        let mut next: Option<LaunchBoundary> = None;
        for boundary in boundaries {
            if let LaunchState::Waiting {
                context: previous,
                target,
            } = self.state
                && previous == context
                && boundary == target
            {
                // Keep a late target visible; never silently move to another phrase.
                return;
            }
            if boundary.beat >= beat && next.is_none_or(|existing| boundary.beat < existing.beat) {
                next = Some(boundary);
            }
        }
        self.state = match next {
            Some(target) => LaunchState::Waiting { context, target },
            None => LaunchState::NoUpcomingBoundary { context },
        };
    }

    /// A negative offset may schedule the chosen target before its beat. The
    /// existing executor still owns deadline calculation and deduplication.
    pub fn allows(&self, policy: LaunchPolicy, context: LaunchContext, phrase: u16) -> bool {
        policy == LaunchPolicy::Immediate
            || self.state == LaunchState::Launched
            || matches!(self.state, LaunchState::Waiting { context: selected, target }
                if selected == context && target.phrase == phrase)
    }

    /// Only successful dispatch of this exact target opens normal operation.
    pub fn did_launch(
        &mut self,
        policy: LaunchPolicy,
        context: LaunchContext,
        phrase: u16,
    ) -> bool {
        if !self.allows(policy, context, phrase) {
            return false;
        }
        self.state = LaunchState::Launched;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(epoch: u64) -> LaunchContext {
        LaunchContext {
            deck: DeckId::new(1),
            load: TrackLoadId::new(1),
            revision: PlanRevision::new(1),
            epoch,
        }
    }
    fn boundaries() -> [LaunchBoundary; 3] {
        [
            LaunchBoundary { phrase: 0, beat: 0 },
            LaunchBoundary {
                phrase: 1,
                beat: 32,
            },
            LaunchBoundary {
                phrase: 2,
                beat: 64,
            },
        ]
    }

    #[test]
    fn paused_start_does_not_launch() {
        let mut gate = LaunchGate::default();
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(1),
            false,
            12,
            boundaries(),
        );
        assert_eq!(gate.state(), LaunchState::WaitingForPlayback);
        assert!(!gate.allows(LaunchPolicy::OnPhraseStart, context(1), 0));
    }
    #[test]
    fn exact_boundary_is_admitted_but_mid_phrase_waits() {
        for (beat, phrase) in [(0, 0), (1, 1), (31, 1), (32, 1), (33, 2)] {
            let mut gate = LaunchGate::default();
            gate.observe(
                LaunchPolicy::OnPhraseStart,
                context(1),
                true,
                beat,
                boundaries(),
            );
            assert!(gate.allows(LaunchPolicy::OnPhraseStart, context(1), phrase));
            assert!(!gate.allows(LaunchPolicy::OnPhraseStart, context(1), 9));
        }
    }
    #[test]
    fn no_future_boundary_is_explicit_and_not_an_automatic_fallback() {
        let mut gate = LaunchGate::default();
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(1),
            true,
            65,
            boundaries(),
        );
        assert_eq!(
            gate.state(),
            LaunchState::NoUpcomingBoundary {
                context: context(1)
            }
        );
        assert!(!gate.allows(LaunchPolicy::OnPhraseStart, context(1), 2));
    }
    #[test]
    fn stale_epoch_is_rejected_and_late_target_does_not_move() {
        let mut gate = LaunchGate::default();
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(1),
            true,
            20,
            boundaries(),
        );
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(1),
            true,
            40,
            boundaries(),
        );
        assert!(gate.allows(LaunchPolicy::OnPhraseStart, context(1), 1));
        assert!(!gate.did_launch(LaunchPolicy::OnPhraseStart, context(2), 1));
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(2),
            true,
            40,
            boundaries(),
        );
        assert!(gate.allows(LaunchPolicy::OnPhraseStart, context(2), 2));
    }
    #[test]
    fn launched_show_never_reenters_run_in_on_pause_seek_or_master_change() {
        let mut gate = LaunchGate::default();
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(1),
            true,
            20,
            boundaries(),
        );
        assert!(gate.did_launch(LaunchPolicy::OnPhraseStart, context(1), 1));
        gate.cancel_pending();
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(2),
            false,
            65,
            boundaries(),
        );
        assert_eq!(gate.state(), LaunchState::Launched);
        assert!(gate.allows(LaunchPolicy::OnPhraseStart, context(2), 0));
        gate.arm();
        assert_eq!(gate.state(), LaunchState::WaitingForPlayback);
    }
    #[test]
    fn changed_plan_or_load_cannot_admit_old_target() {
        let mut gate = LaunchGate::default();
        gate.observe(
            LaunchPolicy::OnPhraseStart,
            context(1),
            true,
            20,
            boundaries(),
        );
        let changed = LaunchContext {
            load: TrackLoadId::new(2),
            ..context(1)
        };
        assert!(!gate.allows(LaunchPolicy::OnPhraseStart, changed, 1));
    }
    #[test]
    fn immediate_remains_default() {
        assert_eq!(LaunchPolicy::default(), LaunchPolicy::Immediate);
        assert!(LaunchGate::default().allows(LaunchPolicy::Immediate, context(1), 0));
    }
}
