use lumi_domain::{KeyMode, MusicalKey, PitchClass, TrackId};
use lumi_library::{
    BeatGrid, BeatMarker, ImportedTrackAnalysis, LumiPhraseTimeline, PhraseConflictChoice,
    PhraseInstance, PhraseRoleId, ReconcileSide, ReconcileStrategy, SourceChangeClass,
    SourceRevision, SourceTrackDiff, SourceTrackId, StoredTrack, TimelineRevision,
    TimelineRevisionOrigin, TimelineRevisionReason, TrackColor, TrackSummary, WaveformPoint,
    reconcile_timeline,
};

#[test]
fn source_diff_classifies_metadata_analysis_and_phrase_changes_independently()
-> Result<(), Box<dyn std::error::Error>> {
    let original = analysis("analysis-v1", "Original", 8, 1, 4)?;
    let stored = stored(&original)?;
    let metadata = analysis("analysis-v2", "Renamed", 8, 1, 4)?;
    let metadata_diff = SourceTrackDiff::between(&stored, &metadata);
    assert!(metadata_diff.is_metadata_only());
    assert!(!metadata_diff.requires_timeline_decision());

    let changed = analysis("analysis-v3", "Renamed", 10, 2, 6)?;
    let diff = SourceTrackDiff::between(&stored, &changed);
    assert_eq!(
        diff.changes(),
        &[
            SourceChangeClass::Metadata,
            SourceChangeClass::Waveform,
            SourceChangeClass::BeatGrid,
            SourceChangeClass::RawPhrases,
        ]
        .into_iter()
        .collect()
    );
    assert!(diff.requires_timeline_decision());
    Ok(())
}

#[test]
fn keep_lumi_records_the_new_baseline_without_changing_phrase_content()
-> Result<(), Box<dyn std::error::Error>> {
    let current = timeline(8, &[(0, 4, "intro"), (4, 8, "drop")])?;
    let reconciled = reconcile_timeline(
        &current,
        SourceRevision::try_new("analysis-v2")?,
        8,
        &source_phrases(&[(0, 3, "intro"), (3, 8, "drop")])?,
        &ReconcileStrategy::KeepLumi,
    )?;
    assert_eq!(reconciled.revision(), TimelineRevision::try_new(2)?);
    assert_eq!(reconciled.phrases(), current.phrases());
    assert_eq!(reconciled.baseline_revision().as_str(), "analysis-v2");
    assert_eq!(reconciled.origin(), TimelineRevisionOrigin::SourceReconcile);
    Ok(())
}

#[test]
fn rebase_is_deterministic_beat_aligned_and_exposes_fractional_ambiguity()
-> Result<(), Box<dyn std::error::Error>> {
    let current = timeline(10, &[(0, 3, "intro"), (3, 7, "build"), (7, 10, "drop")])?;
    let source = source_phrases(&[(0, 4, "intro"), (4, 8, "build"), (8, 12, "drop")])?;
    let preview = lumi_library::ReconcilePreview::between(&current, &source, 12);
    assert_eq!(preview.rebase_ambiguities(), &[0, 1]);

    let first = reconcile_timeline(
        &current,
        SourceRevision::try_new("analysis-v2")?,
        12,
        &source,
        &ReconcileStrategy::Rebase,
    )?;
    let second = reconcile_timeline(
        &current,
        SourceRevision::try_new("analysis-v2")?,
        12,
        &source,
        &ReconcileStrategy::Rebase,
    )?;
    assert_eq!(first, second);
    assert_eq!(
        first
            .phrases()
            .iter()
            .map(|phrase| (phrase.start_beat(), phrase.end_beat()))
            .collect::<Vec<_>>(),
        vec![(0, 4), (4, 8), (8, 12)]
    );
    Ok(())
}

#[test]
fn merge_requires_every_explicit_choice_and_fails_closed_on_invalid_coverage()
-> Result<(), Box<dyn std::error::Error>> {
    let current = timeline(8, &[(0, 4, "intro"), (4, 8, "drop")])?;
    let source = source_phrases(&[(0, 3, "intro"), (3, 8, "drop")])?;
    assert!(
        reconcile_timeline(
            &current,
            SourceRevision::try_new("analysis-v2")?,
            8,
            &source,
            &ReconcileStrategy::Merge(vec![]),
        )
        .is_err()
    );
    let merged = reconcile_timeline(
        &current,
        SourceRevision::try_new("analysis-v2")?,
        8,
        &source,
        &ReconcileStrategy::Merge(vec![
            PhraseConflictChoice {
                phrase_index: 0,
                side: ReconcileSide::Source,
            },
            PhraseConflictChoice {
                phrase_index: 1,
                side: ReconcileSide::Source,
            },
        ]),
    )?;
    assert_eq!(merged.phrases(), source);
    Ok(())
}

#[test]
fn replace_creates_a_recoverable_revision_before_source_initialization()
-> Result<(), Box<dyn std::error::Error>> {
    let current = timeline(8, &[(0, 4, "intro"), (4, 8, "drop")])?;
    let source = source_phrases(&[(0, 2, "intro"), (2, 6, "build"), (6, 8, "drop")])?;
    let replaced = reconcile_timeline(
        &current,
        SourceRevision::try_new("analysis-v2")?,
        8,
        &source,
        &ReconcileStrategy::ReplaceWithSource,
    )?;
    let recovered = LumiPhraseTimeline::restore(&replaced, &current, TimelineRevisionReason::Undo)?;
    assert_eq!(recovered.phrases(), current.phrases());
    assert_eq!(recovered.restored_from(), Some(current.revision()));
    Ok(())
}

fn timeline(
    total_beats: u32,
    phrases: &[(u32, u32, &str)],
) -> Result<LumiPhraseTimeline, Box<dyn std::error::Error>> {
    LumiPhraseTimeline::try_new(
        TrackId::new(1),
        TimelineRevision::initial(),
        SourceRevision::try_new("analysis-v1")?,
        total_beats,
        TimelineRevisionOrigin::UserEdit,
        source_phrases(phrases)?,
    )
    .map_err(Into::into)
}

fn source_phrases(
    phrases: &[(u32, u32, &str)],
) -> Result<Vec<PhraseInstance>, Box<dyn std::error::Error>> {
    phrases
        .iter()
        .enumerate()
        .map(|(index, (start, end, role))| {
            Ok(PhraseInstance::new(
                u16::try_from(index)?,
                *start,
                *end,
                PhraseRoleId::try_new(*role)?,
            ))
        })
        .collect()
}

fn analysis(
    revision: &str,
    title: &str,
    bars: u32,
    waveform_seed: u8,
    phrase_boundary: u32,
) -> Result<ImportedTrackAnalysis, Box<dyn std::error::Error>> {
    let markers = (0..bars * 4)
        .map(|beat| {
            BeatMarker::new(
                beat,
                u64::from(beat) * 500,
                beat / 4 + 1,
                (beat % 4 + 1) as u8,
            )
        })
        .collect();
    ImportedTrackAnalysis::try_new(
        SourceTrackId::try_new("track-1")?,
        SourceRevision::try_new(revision)?,
        title,
        "Artist",
        120_000,
        MusicalKey::new(PitchClass::A, KeyMode::Minor),
        u64::from(bars) * 2_000,
        Some(TrackColor::new(1, 2, 3)),
        "lumi-demo://track-1",
        BeatGrid::try_new(4, markers)?,
        vec![WaveformPoint::new(waveform_seed, 2, 3)],
        vec![
            lumi_library::RawPhraseObservation::try_new(0, phrase_boundary * 4, "Intro")?,
            lumi_library::RawPhraseObservation::try_new(phrase_boundary * 4, bars * 4, "Drop")?,
        ],
    )
    .map_err(Into::into)
}

fn stored(analysis: &ImportedTrackAnalysis) -> Result<StoredTrack, Box<dyn std::error::Error>> {
    Ok(StoredTrack::new(
        TrackSummary::new(
            TrackId::new(1),
            analysis.source_track_id().clone(),
            analysis.title().to_owned(),
            analysis.artist().to_owned(),
            analysis.bpm_milli(),
            analysis.musical_key(),
            analysis.duration_millis(),
            analysis.color(),
            analysis.analysis_revision().clone(),
            Some(TimelineRevision::initial()),
        ),
        analysis.audio_uri().to_owned(),
        analysis.beat_grid().clone(),
        analysis.waveform().to_vec(),
        analysis.raw_phrases().to_vec(),
    ))
}
