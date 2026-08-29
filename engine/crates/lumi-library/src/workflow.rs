use std::collections::BTreeSet;

use lumi_domain::TrackId;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TrackPreparationStatus {
    #[default]
    NotStarted,
    InProgress,
    ReadyForShow,
}

impl TrackPreparationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not-started",
            Self::InProgress => "in-progress",
            Self::ReadyForShow => "ready-for-show",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, TrackWorkflowValueError> {
        match value {
            "not-started" => Ok(Self::NotStarted),
            "in-progress" => Ok(Self::InProgress),
            "ready-for-show" => Ok(Self::ReadyForShow),
            _ => Err(TrackWorkflowValueError::InvalidPreparationStatus),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TrackAttentionReason {
    MetadataChanged,
    WaveformChanged,
    BeatGridChanged,
    HotCuesChanged,
    SourcePhrasesChanged,
}

impl TrackAttentionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataChanged => "metadataChanged",
            Self::WaveformChanged => "waveformChanged",
            Self::BeatGridChanged => "beatGridChanged",
            Self::HotCuesChanged => "hotCuesChanged",
            Self::SourcePhrasesChanged => "sourcePhrasesChanged",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackWorkflowFilter {
    ChangedAfterUsbSync,
    NotStarted,
    InProgress,
    ReadyForShow,
}

impl TrackWorkflowFilter {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChangedAfterUsbSync => "changedAfterUsbSync",
            Self::NotStarted => "notStarted",
            Self::InProgress => "inProgress",
            Self::ReadyForShow => "readyForShow",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, TrackWorkflowValueError> {
        match value {
            "changedAfterUsbSync" => Ok(Self::ChangedAfterUsbSync),
            "notStarted" => Ok(Self::NotStarted),
            "inProgress" => Ok(Self::InProgress),
            "readyForShow" => Ok(Self::ReadyForShow),
            _ => Err(TrackWorkflowValueError::InvalidWorkflowFilter),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackWorkflowAttention {
    revision: u64,
    source_id: String,
    source_revision: String,
    detected_at: String,
    reasons: BTreeSet<TrackAttentionReason>,
}

impl TrackWorkflowAttention {
    #[must_use]
    pub fn new(
        revision: u64,
        source_id: String,
        source_revision: String,
        detected_at: String,
        reasons: BTreeSet<TrackAttentionReason>,
    ) -> Self {
        Self {
            revision,
            source_id,
            source_revision,
            detected_at,
            reasons,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    #[must_use]
    pub fn detected_at(&self) -> &str {
        &self.detected_at
    }
    #[must_use]
    pub const fn reasons(&self) -> &BTreeSet<TrackAttentionReason> {
        &self.reasons
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackWorkflowState {
    track_id: TrackId,
    preparation_status: TrackPreparationStatus,
    status_revision: u64,
    attention: Option<TrackWorkflowAttention>,
}

impl TrackWorkflowState {
    #[must_use]
    pub const fn new(
        track_id: TrackId,
        preparation_status: TrackPreparationStatus,
        status_revision: u64,
        attention: Option<TrackWorkflowAttention>,
    ) -> Self {
        Self {
            track_id,
            preparation_status,
            status_revision,
            attention,
        }
    }

    #[must_use]
    pub const fn default_for(track_id: TrackId) -> Self {
        Self::new(track_id, TrackPreparationStatus::NotStarted, 0, None)
    }

    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.track_id
    }
    #[must_use]
    pub const fn preparation_status(&self) -> TrackPreparationStatus {
        self.preparation_status
    }
    #[must_use]
    pub const fn status_revision(&self) -> u64 {
        self.status_revision
    }
    #[must_use]
    pub const fn attention(&self) -> Option<&TrackWorkflowAttention> {
        self.attention.as_ref()
    }
    #[must_use]
    pub const fn is_effectively_ready(&self) -> bool {
        matches!(
            self.preparation_status,
            TrackPreparationStatus::ReadyForShow
        ) && self.attention.is_none()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrackWorkflowSummary {
    pub changed_after_usb_sync: u64,
    pub not_started: u64,
    pub in_progress: u64,
    pub ready_for_show: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackWorkflowValueError {
    InvalidPreparationStatus,
    InvalidWorkflowFilter,
}

impl std::fmt::Display for TrackWorkflowValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPreparationStatus => {
                formatter.write_str("invalid track preparation status")
            }
            Self::InvalidWorkflowFilter => formatter.write_str("invalid track workflow filter"),
        }
    }
}

impl std::error::Error for TrackWorkflowValueError {}
