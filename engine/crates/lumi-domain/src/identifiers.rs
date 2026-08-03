macro_rules! numeric_identifier {
    ($name:ident, $value:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($value);

        impl $name {
            #[must_use]
            pub const fn new(value: $value) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn value(self) -> $value {
                self.0
            }
        }
    };
}

numeric_identifier!(ClientId, u64);
numeric_identifier!(CommandSequence, u64);
numeric_identifier!(CueId, u64);
numeric_identifier!(DeckId, u8);
numeric_identifier!(EffectId, u64);
numeric_identifier!(EffectSequence, u64);
numeric_identifier!(PlanId, u64);
numeric_identifier!(PlanConfigurationRevision, u64);
numeric_identifier!(PlanRevision, u64);
numeric_identifier!(SceneId, u64);
numeric_identifier!(SourceId, u64);
numeric_identifier!(SourceSequence, u64);
numeric_identifier!(StateRevision, u64);
numeric_identifier!(ThemeId, u64);
numeric_identifier!(TrackId, u64);
numeric_identifier!(TrackLoadId, u64);
numeric_identifier!(WorkerId, u64);

impl PlanRevision {
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl StateRevision {
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}
