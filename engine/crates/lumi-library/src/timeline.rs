use std::error::Error;
use std::fmt;

use lumi_domain::{ThemeId, TrackId};

use crate::{PhraseRoleId, SourceRevision, TimelineRevision, VariantId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineRevisionOrigin {
    SourceImport,
    UserEdit,
    SourceReconcile,
    RevisionRestore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineRevisionReason {
    InitialSourceMapping,
    CreatePhrase,
    SplitPhrase,
    MergePrevious,
    MergeNext,
    MoveBoundary,
    AbsorbPrevious,
    AbsorbNext,
    ChangeRole,
    ChangeLoopStrategy,
    Undo,
    Redo,
    RestoreRevision,
    SourceReconcile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeSpecificVariant {
    theme_id: ThemeId,
    variant_id: VariantId,
}

impl ThemeSpecificVariant {
    #[must_use]
    pub const fn new(theme_id: ThemeId, variant_id: VariantId) -> Self {
        Self {
            theme_id,
            variant_id,
        }
    }

    #[must_use]
    pub const fn theme_id(&self) -> ThemeId {
        self.theme_id
    }

    #[must_use]
    pub const fn variant_id(&self) -> &VariantId {
        &self.variant_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhraseLoopStrategy {
    Auto,
    FixedVariant(VariantId),
    ThemeSpecificExact(Vec<ThemeSpecificVariant>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseInstance {
    index: u16,
    start_beat: u32,
    end_beat: u32,
    role_id: PhraseRoleId,
    loop_strategy: PhraseLoopStrategy,
}

impl PhraseInstance {
    #[must_use]
    pub const fn new(index: u16, start_beat: u32, end_beat: u32, role_id: PhraseRoleId) -> Self {
        Self {
            index,
            start_beat,
            end_beat,
            role_id,
            loop_strategy: PhraseLoopStrategy::Auto,
        }
    }

    #[must_use]
    pub fn with_loop_strategy(mut self, loop_strategy: PhraseLoopStrategy) -> Self {
        self.loop_strategy = loop_strategy;
        self
    }

    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    #[must_use]
    pub const fn start_beat(&self) -> u32 {
        self.start_beat
    }

    #[must_use]
    pub const fn end_beat(&self) -> u32 {
        self.end_beat
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn loop_strategy(&self) -> &PhraseLoopStrategy {
        &self.loop_strategy
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LumiPhraseTimeline {
    track_id: TrackId,
    revision: TimelineRevision,
    baseline_revision: SourceRevision,
    total_beats: u32,
    origin: TimelineRevisionOrigin,
    reason: TimelineRevisionReason,
    parent_revision: Option<TimelineRevision>,
    restored_from: Option<TimelineRevision>,
    phrases: Vec<PhraseInstance>,
}

impl LumiPhraseTimeline {
    pub fn try_new(
        track_id: TrackId,
        revision: TimelineRevision,
        baseline_revision: SourceRevision,
        total_beats: u32,
        origin: TimelineRevisionOrigin,
        phrases: Vec<PhraseInstance>,
    ) -> Result<Self, TimelineValidationError> {
        let reason = match origin {
            TimelineRevisionOrigin::SourceImport => TimelineRevisionReason::InitialSourceMapping,
            TimelineRevisionOrigin::UserEdit => TimelineRevisionReason::ChangeRole,
            TimelineRevisionOrigin::SourceReconcile => TimelineRevisionReason::SourceReconcile,
            TimelineRevisionOrigin::RevisionRestore => TimelineRevisionReason::RestoreRevision,
        };
        let parent_revision = if revision == TimelineRevision::initial() {
            None
        } else {
            TimelineRevision::try_new(revision.value() - 1).ok()
        };
        Self::try_new_with_history(
            track_id,
            revision,
            baseline_revision,
            total_beats,
            origin,
            reason,
            parent_revision,
            None,
            phrases,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new_with_history(
        track_id: TrackId,
        revision: TimelineRevision,
        baseline_revision: SourceRevision,
        total_beats: u32,
        origin: TimelineRevisionOrigin,
        reason: TimelineRevisionReason,
        parent_revision: Option<TimelineRevision>,
        restored_from: Option<TimelineRevision>,
        phrases: Vec<PhraseInstance>,
    ) -> Result<Self, TimelineValidationError> {
        if track_id.value() == 0 {
            return Err(TimelineValidationError::InvalidTrackId);
        }
        if total_beats == 0 {
            return Err(TimelineValidationError::EmptyDuration);
        }
        if phrases.is_empty() {
            return Err(TimelineValidationError::EmptyPhrases);
        }
        if parent_revision.is_some_and(|parent| parent >= revision)
            || restored_from.is_some_and(|restored| restored >= revision)
        {
            return Err(TimelineValidationError::InvalidRevisionHistory);
        }
        let mut previous_end = 0;
        for (expected_index, phrase) in phrases.iter().enumerate() {
            if usize::from(phrase.index()) != expected_index {
                return Err(TimelineValidationError::UnorderedPhraseIndex);
            }
            if phrase.start_beat() != previous_end
                || phrase.end_beat() <= phrase.start_beat()
                || phrase.end_beat() > total_beats
            {
                return Err(TimelineValidationError::InvalidPhraseCoverage);
            }
            if let PhraseLoopStrategy::ThemeSpecificExact(overrides) = phrase.loop_strategy()
                && (overrides.is_empty()
                    || overrides
                        .windows(2)
                        .any(|pair| pair[0].theme_id() >= pair[1].theme_id()))
            {
                return Err(TimelineValidationError::InvalidLoopStrategy);
            }
            previous_end = phrase.end_beat();
        }
        if previous_end != total_beats {
            return Err(TimelineValidationError::InvalidPhraseCoverage);
        }
        Ok(Self {
            track_id,
            revision,
            baseline_revision,
            total_beats,
            origin,
            reason,
            parent_revision,
            restored_from,
            phrases,
        })
    }

    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.track_id
    }

    #[must_use]
    pub const fn revision(&self) -> TimelineRevision {
        self.revision
    }

    #[must_use]
    pub const fn baseline_revision(&self) -> &SourceRevision {
        &self.baseline_revision
    }

    #[must_use]
    pub const fn total_beats(&self) -> u32 {
        self.total_beats
    }

    #[must_use]
    pub const fn origin(&self) -> TimelineRevisionOrigin {
        self.origin
    }

    #[must_use]
    pub const fn reason(&self) -> TimelineRevisionReason {
        self.reason
    }

    #[must_use]
    pub const fn parent_revision(&self) -> Option<TimelineRevision> {
        self.parent_revision
    }

    #[must_use]
    pub const fn restored_from(&self) -> Option<TimelineRevision> {
        self.restored_from
    }

    #[must_use]
    pub fn phrases(&self) -> &[PhraseInstance] {
        &self.phrases
    }

    pub fn edit(&self, command: TimelineEditCommand) -> Result<Self, TimelineEditError> {
        let next_revision = self
            .revision
            .checked_next()
            .ok_or(TimelineEditError::RevisionOverflow)?;
        let reason = command.reason();
        let phrases = apply_edit(&self.phrases, self.total_beats, command)?;
        if phrases == self.phrases {
            return Err(TimelineEditError::NoChange);
        }
        Self::try_new_with_history(
            self.track_id,
            next_revision,
            self.baseline_revision.clone(),
            self.total_beats,
            TimelineRevisionOrigin::UserEdit,
            reason,
            Some(self.revision),
            None,
            phrases,
        )
        .map_err(TimelineEditError::InvalidResult)
    }

    pub fn restore(
        head: &Self,
        target: &Self,
        reason: TimelineRevisionReason,
    ) -> Result<Self, TimelineEditError> {
        if head.track_id != target.track_id || head.total_beats != target.total_beats {
            return Err(TimelineEditError::IncompatibleRevision);
        }
        if !matches!(
            reason,
            TimelineRevisionReason::Undo
                | TimelineRevisionReason::Redo
                | TimelineRevisionReason::RestoreRevision
        ) {
            return Err(TimelineEditError::InvalidRestoreReason);
        }
        let next_revision = head
            .revision
            .checked_next()
            .ok_or(TimelineEditError::RevisionOverflow)?;
        let phrases = target
            .phrases
            .iter()
            .enumerate()
            .map(|(index, phrase)| {
                PhraseInstance::new(
                    u16::try_from(index).unwrap_or(u16::MAX),
                    phrase.start_beat,
                    phrase.end_beat,
                    phrase.role_id.clone(),
                )
                .with_loop_strategy(phrase.loop_strategy.clone())
            })
            .collect();
        Self::try_new_with_history(
            head.track_id,
            next_revision,
            head.baseline_revision.clone(),
            head.total_beats,
            TimelineRevisionOrigin::RevisionRestore,
            reason,
            Some(head.revision),
            Some(target.revision),
            phrases,
        )
        .map_err(TimelineEditError::InvalidResult)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhraseAbsorption {
    Previous,
    Next,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimelineEditCommand {
    Create {
        start_beat: u32,
        end_beat: u32,
        role_id: PhraseRoleId,
    },
    Split {
        phrase_index: u16,
        at_beat: u32,
    },
    MergePrevious {
        phrase_index: u16,
    },
    MergeNext {
        phrase_index: u16,
    },
    MoveBoundary {
        boundary_after_phrase_index: u16,
        to_beat: u32,
    },
    Delete {
        phrase_index: u16,
        absorb_into: PhraseAbsorption,
    },
    ChangeRole {
        phrase_index: u16,
        role_id: PhraseRoleId,
    },
    SetLoopStrategy {
        phrase_index: u16,
        strategy: PhraseLoopStrategy,
    },
}

impl TimelineEditCommand {
    #[must_use]
    pub const fn assigned_role_id(&self) -> Option<&PhraseRoleId> {
        match self {
            Self::Create { role_id, .. } | Self::ChangeRole { role_id, .. } => Some(role_id),
            Self::Split { .. }
            | Self::MergePrevious { .. }
            | Self::MergeNext { .. }
            | Self::MoveBoundary { .. }
            | Self::Delete { .. }
            | Self::SetLoopStrategy { .. } => None,
        }
    }
}

impl TimelineEditCommand {
    const fn reason(&self) -> TimelineRevisionReason {
        match self {
            Self::Create { .. } => TimelineRevisionReason::CreatePhrase,
            Self::Split { .. } => TimelineRevisionReason::SplitPhrase,
            Self::MergePrevious { .. } => TimelineRevisionReason::MergePrevious,
            Self::MergeNext { .. } => TimelineRevisionReason::MergeNext,
            Self::MoveBoundary { .. } => TimelineRevisionReason::MoveBoundary,
            Self::Delete {
                absorb_into: PhraseAbsorption::Previous,
                ..
            } => TimelineRevisionReason::AbsorbPrevious,
            Self::Delete {
                absorb_into: PhraseAbsorption::Next,
                ..
            } => TimelineRevisionReason::AbsorbNext,
            Self::ChangeRole { .. } => TimelineRevisionReason::ChangeRole,
            Self::SetLoopStrategy { .. } => TimelineRevisionReason::ChangeLoopStrategy,
        }
    }
}

fn apply_edit(
    current: &[PhraseInstance],
    total_beats: u32,
    command: TimelineEditCommand,
) -> Result<Vec<PhraseInstance>, TimelineEditError> {
    let mut phrases = current.to_vec();
    match command {
        TimelineEditCommand::Create {
            start_beat,
            end_beat,
            role_id,
        } => {
            if start_beat >= end_beat || end_beat > total_beats {
                return Err(TimelineEditError::InvalidBeatSelection);
            }
            let mut replacement = Vec::new();
            for phrase in &phrases {
                if phrase.end_beat <= start_beat || phrase.start_beat >= end_beat {
                    replacement.push(phrase.clone());
                    continue;
                }
                if phrase.start_beat < start_beat {
                    replacement.push(
                        PhraseInstance::new(
                            0,
                            phrase.start_beat,
                            start_beat,
                            phrase.role_id.clone(),
                        )
                        .with_loop_strategy(phrase.loop_strategy.clone()),
                    );
                }
                if replacement
                    .last()
                    .is_none_or(|last| last.end_beat <= start_beat)
                {
                    replacement.push(PhraseInstance::new(
                        0,
                        start_beat,
                        end_beat,
                        role_id.clone(),
                    ));
                }
                if phrase.end_beat > end_beat {
                    replacement.push(
                        PhraseInstance::new(0, end_beat, phrase.end_beat, phrase.role_id.clone())
                            .with_loop_strategy(phrase.loop_strategy.clone()),
                    );
                }
            }
            phrases = replacement;
        }
        TimelineEditCommand::Split {
            phrase_index,
            at_beat,
        } => {
            let index = phrase_index_index(phrase_index, phrases.len())?;
            let phrase = phrases[index].clone();
            if at_beat <= phrase.start_beat || at_beat >= phrase.end_beat {
                return Err(TimelineEditError::InvalidSplitBoundary);
            }
            phrases[index] =
                PhraseInstance::new(0, phrase.start_beat, at_beat, phrase.role_id.clone())
                    .with_loop_strategy(phrase.loop_strategy);
            phrases.insert(
                index + 1,
                PhraseInstance::new(0, at_beat, phrase.end_beat, phrase.role_id),
            );
        }
        TimelineEditCommand::MergePrevious { phrase_index } => {
            let index = phrase_index_index(phrase_index, phrases.len())?;
            if index == 0 {
                return Err(TimelineEditError::MissingPreviousPhrase);
            }
            let selected = phrases[index].clone();
            phrases[index] = PhraseInstance::new(
                0,
                phrases[index - 1].start_beat,
                selected.end_beat,
                selected.role_id,
            )
            .with_loop_strategy(selected.loop_strategy);
            phrases.remove(index - 1);
        }
        TimelineEditCommand::MergeNext { phrase_index } => {
            let index = phrase_index_index(phrase_index, phrases.len())?;
            if index + 1 >= phrases.len() {
                return Err(TimelineEditError::MissingNextPhrase);
            }
            let selected = phrases[index].clone();
            phrases[index] = PhraseInstance::new(
                0,
                selected.start_beat,
                phrases[index + 1].end_beat,
                selected.role_id,
            )
            .with_loop_strategy(selected.loop_strategy);
            phrases.remove(index + 1);
        }
        TimelineEditCommand::MoveBoundary {
            boundary_after_phrase_index,
            to_beat,
        } => {
            let left = phrase_index_index(boundary_after_phrase_index, phrases.len())?;
            if left + 1 >= phrases.len() {
                return Err(TimelineEditError::MissingNextPhrase);
            }
            if to_beat <= phrases[left].start_beat || to_beat >= phrases[left + 1].end_beat {
                return Err(TimelineEditError::InvalidBoundaryMove);
            }
            let left_phrase = phrases[left].clone();
            let right_phrase = phrases[left + 1].clone();
            phrases[left] =
                PhraseInstance::new(0, left_phrase.start_beat, to_beat, left_phrase.role_id)
                    .with_loop_strategy(left_phrase.loop_strategy);
            phrases[left + 1] =
                PhraseInstance::new(0, to_beat, right_phrase.end_beat, right_phrase.role_id)
                    .with_loop_strategy(right_phrase.loop_strategy);
        }
        TimelineEditCommand::Delete {
            phrase_index,
            absorb_into,
        } => {
            let index = phrase_index_index(phrase_index, phrases.len())?;
            if phrases.len() == 1 {
                return Err(TimelineEditError::CannotDeleteOnlyPhrase);
            }
            let removed = phrases[index].clone();
            match absorb_into {
                PhraseAbsorption::Previous if index > 0 => {
                    let previous = phrases[index - 1].clone();
                    phrases[index - 1] = PhraseInstance::new(
                        0,
                        previous.start_beat,
                        removed.end_beat,
                        previous.role_id,
                    )
                    .with_loop_strategy(previous.loop_strategy);
                    phrases.remove(index);
                }
                PhraseAbsorption::Next if index + 1 < phrases.len() => {
                    let next = phrases[index + 1].clone();
                    phrases[index + 1] =
                        PhraseInstance::new(0, removed.start_beat, next.end_beat, next.role_id)
                            .with_loop_strategy(next.loop_strategy);
                    phrases.remove(index);
                }
                PhraseAbsorption::Previous => {
                    return Err(TimelineEditError::MissingPreviousPhrase);
                }
                PhraseAbsorption::Next => return Err(TimelineEditError::MissingNextPhrase),
            }
        }
        TimelineEditCommand::ChangeRole {
            phrase_index,
            role_id,
        } => {
            let index = phrase_index_index(phrase_index, phrases.len())?;
            let phrase = phrases[index].clone();
            if phrase.role_id == role_id {
                return Err(TimelineEditError::NoChange);
            }
            phrases[index] = PhraseInstance::new(0, phrase.start_beat, phrase.end_beat, role_id);
        }
        TimelineEditCommand::SetLoopStrategy {
            phrase_index,
            strategy,
        } => {
            let index = phrase_index_index(phrase_index, phrases.len())?;
            let phrase = phrases[index].clone();
            if phrase.loop_strategy == strategy {
                return Err(TimelineEditError::NoChange);
            }
            phrases[index] =
                PhraseInstance::new(0, phrase.start_beat, phrase.end_beat, phrase.role_id)
                    .with_loop_strategy(strategy);
        }
    }
    reindex(phrases)
}

fn phrase_index_index(index: u16, len: usize) -> Result<usize, TimelineEditError> {
    let index = usize::from(index);
    if index >= len {
        return Err(TimelineEditError::UnknownPhrase);
    }
    Ok(index)
}

fn reindex(phrases: Vec<PhraseInstance>) -> Result<Vec<PhraseInstance>, TimelineEditError> {
    phrases
        .into_iter()
        .enumerate()
        .map(|(index, phrase)| {
            let index = u16::try_from(index).map_err(|_| TimelineEditError::TooManyPhrases)?;
            Ok(
                PhraseInstance::new(index, phrase.start_beat, phrase.end_beat, phrase.role_id)
                    .with_loop_strategy(phrase.loop_strategy),
            )
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimelineEditError {
    UnknownPhrase,
    InvalidBeatSelection,
    InvalidSplitBoundary,
    InvalidBoundaryMove,
    MissingPreviousPhrase,
    MissingNextPhrase,
    CannotDeleteOnlyPhrase,
    NoChange,
    TooManyPhrases,
    RevisionOverflow,
    IncompatibleRevision,
    InvalidRestoreReason,
    InvalidResult(TimelineValidationError),
}

impl fmt::Display for TimelineEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPhrase => formatter.write_str("the selected phrase no longer exists"),
            Self::InvalidBeatSelection => {
                formatter.write_str("phrase selection must contain at least one complete beat")
            }
            Self::InvalidSplitBoundary => {
                formatter.write_str("split must be on an interior beat boundary")
            }
            Self::InvalidBoundaryMove => {
                formatter.write_str("boundary move would create an empty phrase")
            }
            Self::MissingPreviousPhrase => formatter.write_str("there is no previous phrase"),
            Self::MissingNextPhrase => formatter.write_str("there is no next phrase"),
            Self::CannotDeleteOnlyPhrase => {
                formatter.write_str("the only phrase cannot be deleted")
            }
            Self::NoChange => formatter.write_str("the edit would not change the timeline"),
            Self::TooManyPhrases => formatter.write_str("the timeline contains too many phrases"),
            Self::RevisionOverflow => formatter.write_str("the timeline revision overflowed"),
            Self::IncompatibleRevision => {
                formatter.write_str("the revision belongs to another timeline")
            }
            Self::InvalidRestoreReason => formatter.write_str("the restore reason is invalid"),
            Self::InvalidResult(error) => {
                write!(formatter, "the edit produced an invalid timeline: {error}")
            }
        }
    }
}

impl Error for TimelineEditError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineValidationError {
    InvalidTrackId,
    EmptyDuration,
    EmptyPhrases,
    UnorderedPhraseIndex,
    InvalidPhraseCoverage,
    InvalidRevisionHistory,
    InvalidLoopStrategy,
}

impl fmt::Display for TimelineValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTrackId => formatter.write_str("timeline track id must be positive"),
            Self::EmptyDuration => formatter.write_str("timeline must contain at least one beat"),
            Self::EmptyPhrases => formatter.write_str("timeline must contain phrases"),
            Self::UnorderedPhraseIndex => {
                formatter.write_str("phrase indexes must be contiguous and ordered")
            }
            Self::InvalidPhraseCoverage => {
                formatter.write_str("phrases must cover every complete beat without gaps")
            }
            Self::InvalidRevisionHistory => {
                formatter.write_str("timeline history must only reference earlier revisions")
            }
            Self::InvalidLoopStrategy => {
                formatter.write_str("theme-specific loop overrides must be nonempty and ordered")
            }
        }
    }
}

impl Error for TimelineValidationError {}
