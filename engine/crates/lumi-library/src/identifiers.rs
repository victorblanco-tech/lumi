use std::error::Error;
use std::fmt;

macro_rules! text_identifier {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, TextIdentifierError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(TextIdentifierError::Empty($label));
                }
                if value.len() > 255 {
                    return Err(TextIdentifierError::TooLong($label));
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

text_identifier!(LibrarySourceId, "library source id");
text_identifier!(SourcePlaylistId, "source playlist id");
text_identifier!(SourceTrackId, "source track id");
text_identifier!(SourceRevision, "source revision");
text_identifier!(PhraseRoleId, "phrase role id");
text_identifier!(VariantId, "variant id");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlaylistId(u64);

impl PlaylistId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextIdentifierError {
    Empty(&'static str),
    TooLong(&'static str),
}

impl fmt::Display for TextIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(label) => write!(formatter, "{label} may not be empty"),
            Self::TooLong(label) => write!(formatter, "{label} exceeds 255 bytes"),
        }
    }
}

impl Error for TextIdentifierError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimelineRevision(u64);

impl TimelineRevision {
    pub fn try_new(value: u64) -> Result<Self, TextIdentifierError> {
        if value == 0 {
            return Err(TextIdentifierError::Empty("timeline revision"));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}
