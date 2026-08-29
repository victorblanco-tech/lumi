use std::collections::{BTreeMap, BTreeSet};

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
    VersionCandidates,
    NotStarted,
    InProgress,
    ReadyForShow,
}

impl TrackWorkflowFilter {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChangedAfterUsbSync => "changedAfterUsbSync",
            Self::VersionCandidates => "versionCandidates",
            Self::NotStarted => "notStarted",
            Self::InProgress => "inProgress",
            Self::ReadyForShow => "readyForShow",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, TrackWorkflowValueError> {
        match value {
            "changedAfterUsbSync" => Ok(Self::ChangedAfterUsbSync),
            "versionCandidates" => Ok(Self::VersionCandidates),
            "notStarted" => Ok(Self::NotStarted),
            "inProgress" => Ok(Self::InProgress),
            "readyForShow" => Ok(Self::ReadyForShow),
            _ => Err(TrackWorkflowValueError::InvalidWorkflowFilter),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowRuleField {
    PreparationStatus,
    TechnicalReady,
    UnresolvedUsbChange,
    AuthoredTimeline,
    AudioAvailable,
    VersionCandidate,
}

impl WorkflowRuleField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreparationStatus => "preparationStatus",
            Self::TechnicalReady => "technicalReady",
            Self::UnresolvedUsbChange => "unresolvedUsbChange",
            Self::AuthoredTimeline => "authoredTimeline",
            Self::AudioAvailable => "audioAvailable",
            Self::VersionCandidate => "versionCandidate",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, TrackWorkflowValueError> {
        match value {
            "preparationStatus" => Ok(Self::PreparationStatus),
            "technicalReady" => Ok(Self::TechnicalReady),
            "unresolvedUsbChange" => Ok(Self::UnresolvedUsbChange),
            "authoredTimeline" => Ok(Self::AuthoredTimeline),
            "audioAvailable" => Ok(Self::AudioAvailable),
            "versionCandidate" => Ok(Self::VersionCandidate),
            _ => Err(TrackWorkflowValueError::InvalidWorkflowRule),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowRuleOperator {
    Is,
    IsNot,
}

impl WorkflowRuleOperator {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Is => "is",
            Self::IsNot => "isNot",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, TrackWorkflowValueError> {
        match value {
            "is" => Ok(Self::Is),
            "isNot" => Ok(Self::IsNot),
            _ => Err(TrackWorkflowValueError::InvalidWorkflowRule),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRule {
    field: WorkflowRuleField,
    operator: WorkflowRuleOperator,
    value: String,
}

impl WorkflowRule {
    pub fn try_new(
        field: WorkflowRuleField,
        operator: WorkflowRuleOperator,
        value: impl Into<String>,
    ) -> Result<Self, TrackWorkflowValueError> {
        let value = value.into();
        let valid = match field {
            WorkflowRuleField::PreparationStatus => valid_step_id(&value),
            _ => matches!(value.as_str(), "true" | "false"),
        };
        if !valid {
            return Err(TrackWorkflowValueError::InvalidWorkflowRule);
        }
        Ok(Self {
            field,
            operator,
            value,
        })
    }

    #[must_use]
    pub const fn field(&self) -> WorkflowRuleField {
        self.field
    }
    #[must_use]
    pub const fn operator(&self) -> WorkflowRuleOperator {
        self.operator
    }
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowStepDefinition {
    id: String,
    display_name: String,
    icon: String,
    color_rgb: u32,
    sort_order: u16,
    archived: bool,
    rules: Vec<WorkflowRule>,
}

impl WorkflowStepDefinition {
    pub fn try_new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        icon: impl Into<String>,
        color_rgb: u32,
        sort_order: u16,
        archived: bool,
        rules: Vec<WorkflowRule>,
    ) -> Result<Self, TrackWorkflowValueError> {
        let id = id.into();
        let display_name = display_name.into().trim().to_owned();
        let icon = icon.into().trim().to_owned();
        if !valid_step_id(&id)
            || display_name.is_empty()
            || display_name.chars().count() > 48
            || icon.is_empty()
            || icon.chars().count() > 64
            || color_rgb > 0xFF_FF_FF
            || sort_order == 0
            || rules.is_empty()
            || rules.len() > 8
        {
            return Err(TrackWorkflowValueError::InvalidWorkflowStep);
        }
        let mut seen = BTreeMap::new();
        for rule in &rules {
            let key = rule.field();
            if let Some((operator, value)) =
                seen.insert(key.as_str(), (rule.operator(), rule.value()))
                && (operator != rule.operator() || value != rule.value())
            {
                return Err(TrackWorkflowValueError::UnreachableWorkflowStep);
            }
        }
        Ok(Self {
            id,
            display_name,
            icon,
            color_rgb,
            sort_order,
            archived,
            rules,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    #[must_use]
    pub fn icon(&self) -> &str {
        &self.icon
    }
    #[must_use]
    pub const fn color_rgb(&self) -> u32 {
        self.color_rgb
    }
    #[must_use]
    pub const fn sort_order(&self) -> u16 {
        self.sort_order
    }
    #[must_use]
    pub const fn archived(&self) -> bool {
        self.archived
    }
    #[must_use]
    pub fn rules(&self) -> &[WorkflowRule] {
        &self.rules
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackWorkflowCatalog {
    revision: u64,
    steps: Vec<WorkflowStepDefinition>,
}

impl TrackWorkflowCatalog {
    pub fn try_new(
        revision: u64,
        mut steps: Vec<WorkflowStepDefinition>,
    ) -> Result<Self, TrackWorkflowValueError> {
        if steps.len() < 3 || steps.len() > 12 {
            return Err(TrackWorkflowValueError::InvalidWorkflowCatalog);
        }
        steps.sort_by_key(WorkflowStepDefinition::sort_order);
        let ids = steps
            .iter()
            .map(WorkflowStepDefinition::id)
            .collect::<BTreeSet<_>>();
        if ids.len() != steps.len()
            || !["not-started", "in-progress", "ready-for-show"]
                .iter()
                .all(|id| ids.contains(id))
        {
            return Err(TrackWorkflowValueError::InvalidWorkflowCatalog);
        }
        Ok(Self { revision, steps })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub fn steps(&self) -> &[WorkflowStepDefinition] {
        &self.steps
    }
}

fn valid_step_id(value: &str) -> bool {
    let len = value.len();
    (1..=48).contains(&len)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
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
    step_id: String,
    status_revision: u64,
    attention: Option<TrackWorkflowAttention>,
}

impl TrackWorkflowState {
    #[must_use]
    pub fn new(
        track_id: TrackId,
        preparation_status: TrackPreparationStatus,
        step_id: String,
        status_revision: u64,
        attention: Option<TrackWorkflowAttention>,
    ) -> Self {
        Self {
            track_id,
            preparation_status,
            step_id,
            status_revision,
            attention,
        }
    }

    #[must_use]
    pub fn default_for(track_id: TrackId) -> Self {
        Self::new(
            track_id,
            TrackPreparationStatus::NotStarted,
            "not-started".to_owned(),
            0,
            None,
        )
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
    pub fn step_id(&self) -> &str {
        &self.step_id
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrackWorkflowSummary {
    pub changed_after_usb_sync: u64,
    pub version_candidates: u64,
    pub not_started: u64,
    pub in_progress: u64,
    pub ready_for_show: u64,
    pub catalog_revision: u64,
    pub step_counts: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackWorkflowValueError {
    InvalidPreparationStatus,
    InvalidWorkflowFilter,
    InvalidWorkflowRule,
    InvalidWorkflowStep,
    InvalidWorkflowCatalog,
    UnreachableWorkflowStep,
}

impl std::fmt::Display for TrackWorkflowValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPreparationStatus => {
                formatter.write_str("invalid track preparation status")
            }
            Self::InvalidWorkflowFilter => formatter.write_str("invalid track workflow filter"),
            Self::InvalidWorkflowRule => formatter.write_str("invalid track workflow rule"),
            Self::InvalidWorkflowStep => formatter.write_str("invalid track workflow step"),
            Self::InvalidWorkflowCatalog => formatter.write_str("invalid track workflow catalog"),
            Self::UnreachableWorkflowStep => {
                formatter.write_str("workflow step has contradictory rules")
            }
        }
    }
}

impl std::error::Error for TrackWorkflowValueError {}
