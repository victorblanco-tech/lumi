use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lumi_domain::MonotonicTime;

pub trait MonotonicClock: Clone {
    fn now(&self) -> MonotonicTime;
}

#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    ticks: Arc<AtomicU64>,
}

impl ManualClock {
    #[must_use]
    pub fn new(initial_ticks: u64) -> Self {
        Self {
            ticks: Arc::new(AtomicU64::new(initial_ticks)),
        }
    }

    pub fn set(&self, ticks: u64) {
        self.ticks.store(ticks, Ordering::SeqCst);
    }

    pub fn advance(&self, ticks: u64) -> Option<MonotonicTime> {
        self.ticks
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                current.checked_add(ticks)
            })
            .ok()
            .and_then(|previous| previous.checked_add(ticks))
            .map(MonotonicTime::new)
    }
}

impl MonotonicClock for ManualClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::new(self.ticks.load(Ordering::SeqCst))
    }
}
