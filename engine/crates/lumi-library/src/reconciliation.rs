use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    ImportedTrackAnalysis, LumiPhraseTimeline, PhraseInstance, SourceRevision, StoredTrack,
    TimelineRevisionOrigin, TimelineRevisionReason, TimelineValidationError,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceChangeClass {
    Metadata,
    Waveform,
    BeatGrid,
    RawPhrases,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTrackDiff {
    from_revision: SourceRevision,
    to_revision: SourceRevision,
    changes: BTreeSet<SourceChangeClass>,
}

impl SourceTrackDiff {
    #[must_use]
    pub fn between(current: &StoredTrack, incoming: &ImportedTrackAnalysis) -> Self {
        let mut changes = BTreeSet::new();
        let summary = current.summary();
        if summary.title() != incoming.title()
            || summary.artist() != incoming.artist()
            || summary.bpm_milli() != incoming.bpm_milli()
            || summary.musical_key() != incoming.musical_key()
            || summary.duration_millis() != incoming.duration_millis()
            || summary.color() != incoming.color()
            || current.audio_uri() != incoming.audio_uri()
        {
            changes.insert(SourceChangeClass::Metadata);
        }
        if current.waveform() != incoming.waveform() {
            changes.insert(SourceChangeClass::Waveform);
        }
        if current.beat_grid() != incoming.beat_grid() {
            changes.insert(SourceChangeClass::BeatGrid);
        }
        if current.raw_phrases() != incoming.raw_phrases() {
            changes.insert(SourceChangeClass::RawPhrases);
        }
        Self {
            from_revision: summary.source_revision().clone(),
            to_revision: incoming.analysis_revision().clone(),
            changes,
        }
    }

    #[must_use]
    pub const fn from_revision(&self) -> &SourceRevision {
        &self.from_revision
    }

    #[must_use]
    pub const fn to_revision(&self) -> &SourceRevision {
        &self.to_revision
    }

    #[must_use]
    pub const fn changes(&self) -> &BTreeSet<SourceChangeClass> {
        &self.changes
    }

    #[must_use]
    pub fn is_metadata_only(&self) -> bool {
        self.changes == BTreeSet::from([SourceChangeClass::Metadata])
    }

    #[must_use]
    pub fn requires_timeline_decision(&self) -> bool {
        self.changes.contains(&SourceChangeClass::BeatGrid)
            || self.changes.contains(&SourceChangeClass::RawPhrases)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconcileSide {
    Lumi,
    Source,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhraseConflictChoice {
    pub phrase_index: u16,
    pub side: ReconcileSide,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconcileStrategy {
    KeepLumi,
    Rebase,
    Merge(Vec<PhraseConflictChoice>),
    ReplaceWithSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseConflict {
    phrase_index: u16,
    lumi: Option<PhraseInstance>,
    source: Option<PhraseInstance>,
}

impl PhraseConflict {
    #[must_use]
    pub const fn phrase_index(&self) -> u16 {
        self.phrase_index
    }

    #[must_use]
    pub const fn lumi(&self) -> Option<&PhraseInstance> {
        self.lumi.as_ref()
    }

    #[must_use]
    pub const fn source(&self) -> Option<&PhraseInstance> {
        self.source.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcilePreview {
    conflicts: Vec<PhraseConflict>,
    rebase_ambiguities: Vec<u16>,
}

impl ReconcilePreview {
    #[must_use]
    pub fn between(
        current: &LumiPhraseTimeline,
        source_phrases: &[PhraseInstance],
        source_total_bars: u32,
    ) -> Self {
        let count = current.phrases().len().max(source_phrases.len());
        let mut conflicts = Vec::new();
        for index in 0..count {
            let lumi = current.phrases().get(index).cloned();
            let source = source_phrases.get(index).cloned();
            if lumi != source {
                conflicts.push(PhraseConflict {
                    phrase_index: u16::try_from(index).unwrap_or(u16::MAX),
                    lumi,
                    source,
                });
            }
        }
        let rebase_ambiguities = current
            .phrases()
            .iter()
            .take(current.phrases().len().saturating_sub(1))
            .filter(|phrase| {
                u64::from(phrase.end_bar()) * u64::from(source_total_bars)
                    % u64::from(current.total_bars())
                    != 0
            })
            .map(PhraseInstance::index)
            .collect();
        Self {
            conflicts,
            rebase_ambiguities,
        }
    }

    #[must_use]
    pub fn conflicts(&self) -> &[PhraseConflict] {
        &self.conflicts
    }

    #[must_use]
    pub fn rebase_ambiguities(&self) -> &[u16] {
        &self.rebase_ambiguities
    }
}

pub fn reconcile_timeline(
    current: &LumiPhraseTimeline,
    new_baseline_revision: SourceRevision,
    source_total_bars: u32,
    source_phrases: &[PhraseInstance],
    strategy: &ReconcileStrategy,
) -> Result<LumiPhraseTimeline, ReconcileError> {
    let next_revision = current
        .revision()
        .checked_next()
        .ok_or(ReconcileError::RevisionOverflow)?;
    let phrases = match strategy {
        ReconcileStrategy::KeepLumi => {
            if current.total_bars() != source_total_bars {
                return Err(ReconcileError::KeepLumiDurationChanged);
            }
            current.phrases().to_vec()
        }
        ReconcileStrategy::Rebase => rebase_phrases(current, source_total_bars)?,
        ReconcileStrategy::Merge(choices) => {
            merge_phrases(current, source_phrases, choices, source_total_bars)?
        }
        ReconcileStrategy::ReplaceWithSource => source_phrases.to_vec(),
    };
    LumiPhraseTimeline::try_new_with_history(
        current.track_id(),
        next_revision,
        new_baseline_revision,
        source_total_bars,
        TimelineRevisionOrigin::SourceReconcile,
        TimelineRevisionReason::SourceReconcile,
        Some(current.revision()),
        None,
        reindex(phrases)?,
    )
    .map_err(ReconcileError::InvalidTimeline)
}

fn rebase_phrases(
    current: &LumiPhraseTimeline,
    source_total_bars: u32,
) -> Result<Vec<PhraseInstance>, ReconcileError> {
    if source_total_bars == 0 || source_total_bars < current.phrases().len() as u32 {
        return Err(ReconcileError::InsufficientBars);
    }
    let mut previous = 0_u32;
    let phrase_count = current.phrases().len();
    let mut result = Vec::with_capacity(phrase_count);
    for (index, phrase) in current.phrases().iter().enumerate() {
        let remaining =
            u32::try_from(phrase_count - index - 1).map_err(|_| ReconcileError::TooManyPhrases)?;
        let end = if index + 1 == phrase_count {
            source_total_bars
        } else {
            let numerator = u64::from(phrase.end_bar()) * u64::from(source_total_bars);
            let denominator = u64::from(current.total_bars());
            let rounded = (numerator + denominator / 2) / denominator;
            u32::try_from(rounded)
                .map_err(|_| ReconcileError::ArithmeticOverflow)?
                .clamp(previous + 1, source_total_bars - remaining)
        };
        result.push(
            PhraseInstance::new(phrase.index(), previous, end, phrase.role_id().clone())
                .with_loop_strategy(phrase.loop_strategy().clone()),
        );
        previous = end;
    }
    Ok(result)
}

fn merge_phrases(
    current: &LumiPhraseTimeline,
    source: &[PhraseInstance],
    choices: &[PhraseConflictChoice],
    source_total_bars: u32,
) -> Result<Vec<PhraseInstance>, ReconcileError> {
    let preview = ReconcilePreview::between(current, source, source_total_bars);
    let expected = preview
        .conflicts()
        .iter()
        .map(PhraseConflict::phrase_index)
        .collect::<BTreeSet<_>>();
    let selected = choices
        .iter()
        .map(|choice| choice.phrase_index)
        .collect::<BTreeSet<_>>();
    if selected.len() != choices.len() || selected != expected {
        return Err(ReconcileError::IncompleteConflictChoices);
    }
    let count = current.phrases().len().max(source.len());
    let mut merged = Vec::with_capacity(count);
    for index in 0..count {
        let phrase_index = u16::try_from(index).map_err(|_| ReconcileError::TooManyPhrases)?;
        let choice = choices
            .iter()
            .find(|choice| choice.phrase_index == phrase_index)
            .map_or(ReconcileSide::Lumi, |choice| choice.side);
        let phrase = match choice {
            ReconcileSide::Lumi => current.phrases().get(index),
            ReconcileSide::Source => source.get(index),
        }
        .ok_or(ReconcileError::MissingMergePhrase)?;
        merged.push(phrase.clone());
    }
    validate_coverage(&merged, source_total_bars)?;
    Ok(merged)
}

fn validate_coverage(phrases: &[PhraseInstance], total_bars: u32) -> Result<(), ReconcileError> {
    let mut previous = 0;
    for phrase in phrases {
        if phrase.start_bar() != previous || phrase.end_bar() <= phrase.start_bar() {
            return Err(ReconcileError::NonContiguousMerge);
        }
        previous = phrase.end_bar();
    }
    if previous != total_bars {
        return Err(ReconcileError::NonContiguousMerge);
    }
    Ok(())
}

fn reindex(phrases: Vec<PhraseInstance>) -> Result<Vec<PhraseInstance>, ReconcileError> {
    phrases
        .into_iter()
        .enumerate()
        .map(|(index, phrase)| {
            Ok(PhraseInstance::new(
                u16::try_from(index).map_err(|_| ReconcileError::TooManyPhrases)?,
                phrase.start_bar(),
                phrase.end_bar(),
                phrase.role_id().clone(),
            )
            .with_loop_strategy(phrase.loop_strategy().clone()))
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconcileError {
    RevisionOverflow,
    KeepLumiDurationChanged,
    InsufficientBars,
    ArithmeticOverflow,
    TooManyPhrases,
    IncompleteConflictChoices,
    MissingMergePhrase,
    NonContiguousMerge,
    InvalidTimeline(TimelineValidationError),
}

impl fmt::Display for ReconcileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RevisionOverflow => formatter.write_str("timeline revision overflow"),
            Self::KeepLumiDurationChanged => {
                formatter.write_str("Keep Lumi requires the same complete-bar duration")
            }
            Self::InsufficientBars => {
                formatter.write_str("source has too few bars for the timeline")
            }
            Self::ArithmeticOverflow => formatter.write_str("rebase arithmetic overflow"),
            Self::TooManyPhrases => formatter.write_str("timeline contains too many phrases"),
            Self::IncompleteConflictChoices => {
                formatter.write_str("every merge conflict needs exactly one explicit choice")
            }
            Self::MissingMergePhrase => formatter.write_str("chosen merge phrase does not exist"),
            Self::NonContiguousMerge => {
                formatter.write_str("merge choices create a gap, overlap, or incomplete coverage")
            }
            Self::InvalidTimeline(error) => {
                write!(formatter, "invalid reconciled timeline: {error}")
            }
        }
    }
}

impl Error for ReconcileError {}
