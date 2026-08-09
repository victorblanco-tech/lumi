/// Monotonic runtime time supplied by an adapter-owned clock.
///
/// The numeric unit is adapter-defined but must remain stable for one runtime.
/// Wall-clock time is intentionally absent from domain decisions.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicTime(u64);

impl MonotonicTime {
    #[must_use]
    pub const fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.0
    }
}
