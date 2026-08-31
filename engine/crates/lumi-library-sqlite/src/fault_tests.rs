use std::collections::BTreeSet;
use std::fs;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lumi_domain::TrackId;
use lumi_library::{
    HotCue, ImportedLibraryBaseline, ImportedTrackAnalysis, LibraryRepository, LibraryTrackQuery,
    RawPhraseObservation, SourceRevision, TrackAttentionReason, TrackColor, TrackPageRequest,
    TrackPreparationStatus, TrackWorkflowFilter, WorkflowRule, WorkflowRuleField,
    WorkflowRuleOperator, WorkflowStepDefinition,
};
use lumi_library_demo::DemoLibrarySourceProvider;
use lumi_library_source::MusicLibrarySourceProvider;
use lumi_light_plans::{BankOrganization, ColorBehavior, ThemeRule};

use super::{
    Connection, DeviceAliasUpsert, DeviceAnalysisDecision, DeviceAnalysisUpsert,
    DeviceHotCueUpsert, DevicePlaylistUpsert, DeviceTrackImport, SQLITE_BUSY_TIMEOUT,
    SqliteLibraryError, SqliteLibraryRepository, legacy_partial_bar_phrase_projection,
    record_workflow_attention, repair_legacy_partial_bar_timeline_points, to_i64,
};

#[test]
fn file_repository_has_explicit_wal_durability_and_bounded_lock_waiting()
-> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let directory =
        std::env::temp_dir().join(format!("lumi-sqlite-policy-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory)?;
    let database = directory.join("library.sqlite");
    let repository = SqliteLibraryRepository::open(&database)?;

    let journal_mode: String =
        repository
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    let synchronous: u32 = repository
        .connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    let busy_timeout: i64 = repository
        .connection
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))?;
    assert_eq!(journal_mode, "wal");
    assert_eq!(synchronous, 1, "SQLite NORMAL must stay explicit");
    assert_eq!(busy_timeout, SQLITE_BUSY_TIMEOUT.as_millis() as i64);

    let lock_connection = Connection::open(&database)?;
    lock_connection.execute_batch("BEGIN IMMEDIATE;")?;
    let (released_tx, released_rx) = mpsc::channel();
    let release = thread::spawn(move || -> Result<(), rusqlite::Error> {
        thread::sleep(Duration::from_millis(75));
        lock_connection.execute_batch("ROLLBACK;")?;
        let _ = released_tx.send(());
        Ok(())
    });
    let started = Instant::now();
    repository.connection.execute(
        "INSERT INTO library_settings(key, value) VALUES ('busy-policy-test', 'ok')",
        [],
    )?;
    assert!(started.elapsed() >= Duration::from_millis(50));
    released_rx.recv_timeout(Duration::from_secs(1))?;
    release
        .join()
        .map_err(|_| "SQLite lock-release thread panicked")??;
    drop(repository);
    fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn workflow_status_and_usb_attention_are_revisioned_filterable_and_persistent()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 1)?)?
        .tracks()[0]
        .id();

    let updated = repository.set_track_preparation_status(
        track_id,
        0,
        TrackPreparationStatus::ReadyForShow,
    )?;
    assert_eq!(updated.status_revision(), 1);
    assert!(updated.is_effectively_ready());
    assert!(matches!(
        repository.set_track_preparation_status(track_id, 0, TrackPreparationStatus::InProgress,),
        Err(SqliteLibraryError::TrackWorkflowRevisionConflict {
            expected: 0,
            actual: 1,
        })
    ));

    let transaction = repository.connection.transaction()?;
    record_workflow_attention(
        &transaction,
        to_i64(track_id.value())?,
        "usb:gray",
        "analysis-v2",
        &BTreeSet::from([
            TrackAttentionReason::BeatGridChanged,
            TrackAttentionReason::HotCuesChanged,
        ]),
    )?;
    transaction.commit()?;

    let workflow = repository
        .track_workflow_states(&[track_id])?
        .remove(&track_id)
        .ok_or("workflow state missing")?;
    assert_eq!(
        workflow.preparation_status(),
        TrackPreparationStatus::ReadyForShow
    );
    assert!(!workflow.is_effectively_ready());
    let attention = workflow.attention().ok_or("USB attention missing")?;
    assert!(
        attention
            .reasons()
            .contains(&TrackAttentionReason::BeatGridChanged)
    );
    assert!(
        attention
            .reasons()
            .contains(&TrackAttentionReason::HotCuesChanged)
    );

    let changed = repository.query_tracks(
        &LibraryTrackQuery::try_new("", None, TrackPageRequest::try_new(0, 25)?)?
            .with_workflow_filter(Some(TrackWorkflowFilter::ChangedAfterUsbSync)),
    )?;
    assert_eq!(changed.total(), 1);
    assert_eq!(changed.tracks()[0].id(), track_id);
    let ready = repository.query_tracks(
        &LibraryTrackQuery::try_new("", None, TrackPageRequest::try_new(0, 25)?)?
            .with_workflow_filter(Some(TrackWorkflowFilter::ReadyForShow)),
    )?;
    assert_eq!(ready.total(), 0);

    let resolved = repository.resolve_track_workflow_attention(track_id, attention.revision())?;
    assert!(resolved.is_effectively_ready());
    assert_eq!(
        repository.track_workflow_summary()?.changed_after_usb_sync,
        0
    );
    assert_eq!(repository.track_workflow_summary()?.ready_for_show, 1);
    Ok(())
}

#[test]
fn configurable_workflow_steps_are_revisioned_assignable_and_queryable()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let mut steps = repository.track_workflow_catalog()?.steps().to_vec();
    steps.push(WorkflowStepDefinition::try_new(
        "quality-check",
        "Quality Check",
        "checklist",
        0x32_B8_F5,
        4,
        false,
        vec![WorkflowRule::try_new(
            WorkflowRuleField::PreparationStatus,
            WorkflowRuleOperator::Is,
            "quality-check",
        )?],
    )?);
    let catalog = repository.replace_track_workflow_catalog(1, steps)?;
    assert_eq!(catalog.revision(), 2);

    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 1)?)?
        .tracks()[0]
        .id();
    let state = repository.assign_track_workflow_step(track_id, 0, "quality-check")?;
    assert_eq!(state.step_id(), "quality-check");
    let page = repository.query_tracks(
        &LibraryTrackQuery::try_new("", None, TrackPageRequest::try_new(0, 25)?)?
            .with_workflow_step_id(Some("quality-check".to_owned())),
    )?;
    assert_eq!(page.total(), 1);
    assert_eq!(page.tracks()[0].id(), track_id);
    assert_eq!(
        repository
            .track_workflow_summary()?
            .step_counts
            .get("quality-check"),
        Some(&1),
    );
    Ok(())
}

#[test]
fn detects_only_the_legacy_partial_bar_phrase_projection() -> Result<(), Box<dyn std::error::Error>>
{
    let legacy = vec![
        (0, 1, "Intro".to_owned()),
        (1, 33, "Intro".to_owned()),
        (33, 65, "Drop".to_owned()),
    ];
    let corrected = vec![
        RawPhraseObservation::try_new(0, 32, "Intro")?,
        RawPhraseObservation::try_new(32, 64, "Drop")?,
    ];
    assert!(legacy_partial_bar_phrase_projection(&legacy, &corrected));

    let unrelated = vec![(0, 32, "Intro".to_owned()), (32, 64, "Drop".to_owned())];
    assert!(!legacy_partial_bar_phrase_projection(
        &unrelated, &corrected
    ));

    let repaired = repair_legacy_partial_bar_timeline_points(
        &legacy,
        &corrected,
        vec![
            (0, "old-leading".to_owned(), "auto".to_owned()),
            (1, "intro".to_owned(), "fixed".to_owned()),
            // A user deliberately moved the old source boundary from 33.
            (29, "custom".to_owned(), "auto".to_owned()),
        ],
    );
    assert_eq!(
        repaired,
        vec![
            (0, "intro".to_owned(), "fixed".to_owned()),
            (29, "custom".to_owned(), "auto".to_owned()),
        ]
    );
    Ok(())
}

#[test]
fn failed_analysis_refresh_rolls_back_the_complete_previous_track()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let source_track = &baseline.tracks()[0];
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == source_track.source_track_id())
        .ok_or("stored source track not found")?
        .id();
    let before = repository
        .track(track_id)?
        .ok_or("stored track not found before fault")?;

    let changed_track = ImportedTrackAnalysis::try_new(
        source_track.source_track_id().clone(),
        SourceRevision::try_new("forced-failure-v2")?,
        "This update must roll back",
        source_track.artist(),
        source_track.bpm_milli(),
        source_track.musical_key(),
        source_track.duration_millis(),
        source_track.color(),
        source_track.audio_uri(),
        source_track.beat_grid().clone(),
        source_track.waveform().to_vec(),
        source_track.raw_phrases().to_vec(),
    )?;
    let mut changed_tracks = baseline.tracks().to_vec();
    changed_tracks[0] = changed_track;
    let changed_baseline = ImportedLibraryBaseline::try_new(
        baseline.source_id().clone(),
        baseline.source_kind(),
        baseline.display_name(),
        SourceRevision::try_new("forced-failure-v2")?,
        changed_tracks,
        baseline.playlists().to_vec(),
    )?;
    repository.connection.execute_batch(
        "CREATE TRIGGER force_waveform_failure
         BEFORE INSERT ON waveform_points
         BEGIN SELECT RAISE(ABORT, 'injected waveform failure'); END;",
    )?;

    assert!(repository.import_baseline(&changed_baseline).is_err());
    let after = repository
        .track(track_id)?
        .ok_or("stored track not found after rollback")?;
    assert_eq!(after, before);
    Ok(())
}

#[test]
fn device_hot_cues_enrich_a_track_without_replacing_its_analysis()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let source_track = &baseline.tracks()[0];
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == source_track.source_track_id())
        .ok_or("stored source track not found")?
        .id();
    let before = repository.track(track_id)?.ok_or("stored track missing")?;
    let cue = HotCue::try_new(1, 12_500, None, "Drop", 0x00ff_3366)?;
    let mut aliases = vec![DeviceAliasUpsert {
        device_track_id: 1_256,
        simulator_signature: 0,
        canonical_track_id: Some(track_id),
        match_kind: "metadata+file-size".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-v1".to_owned(),
        color_rgb: Some(0x32_80_ff),
        master_database_id: 1,
        master_content_id: 1_256,
        information_update_count: 1,
        analysis_revision: "analysis-v1".to_owned(),
        audio_signature: "audio:test:1256".to_owned(),
        analyzed_at: "2026-08-13".to_owned(),
        sync_disposition: "held-conflict".to_owned(),
    }];

    assert_eq!(
        repository.device_hot_cue_decision(track_id, "usb-fs:gray", "analysis-v1", "2026-08-13",)?,
        DeviceAnalysisDecision::PromoteInitial
    );
    repository.sync_device_aliases(
        "usb-fs:gray",
        "DJ VIC GRAY",
        "database-v1",
        &mut aliases,
        &[],
        &[],
        &[DeviceHotCueUpsert {
            track_id,
            source_id: "usb-fs:gray".to_owned(),
            device_track_id: 1_256,
            source_analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-13".to_owned(),
            hot_cues: vec![cue.clone()],
        }],
        &[],
    )?;

    let review_tracks = repository.device_review_tracks()?;
    let gray_review = review_tracks
        .get("usb-fs:gray")
        .ok_or("held conflict was not exposed for review")?;
    assert_eq!(gray_review.len(), 1);
    assert_eq!(gray_review[0].device_track_id, 1_256);
    assert_eq!(gray_review[0].canonical_track_id, Some(track_id));
    assert_eq!(gray_review[0].title, source_track.title());
    assert_eq!(
        gray_review[0].active_source_name.as_deref(),
        Some("Lumi library")
    );
    assert_eq!(
        gray_review[0].active_analysis_revision.as_deref(),
        Some(before.summary().source_revision().as_str()),
        "review actions remain available for tracks imported before provenance tracking"
    );

    let after = repository
        .track(track_id)?
        .ok_or("enriched track missing")?;
    assert_eq!(
        after.summary().source_revision(),
        before.summary().source_revision()
    );
    assert_eq!(after.beat_grid(), before.beat_grid());
    assert_eq!(after.waveform(), before.waveform());
    assert_eq!(after.raw_phrases(), before.raw_phrases());
    assert_eq!(after.hot_cues(), &[cue]);
    assert_eq!(
        after.summary().color().map(TrackColor::rgb_u32),
        Some(0x32_80_ff)
    );
    assert_eq!(repository.track_color_summaries()?[0].track_count, 1);

    aliases[0].metadata_revision = "metadata-older".to_owned();
    aliases[0].color_rgb = Some(0xff_33_33);
    aliases[0].information_update_count = 0;
    aliases[0].analyzed_at = "2026-08-12".to_owned();
    repository.sync_device_aliases(
        "usb-fs:gray",
        "DJ VIC GRAY",
        "database-older",
        &mut aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert_eq!(
        repository
            .track(track_id)?
            .and_then(|track| track.summary().color())
            .map(TrackColor::rgb_u32),
        Some(0x32_80_ff),
        "an older USB metadata revision must not overwrite the active color"
    );

    aliases[0].metadata_revision = "metadata-newer".to_owned();
    aliases[0].color_rgb = Some(0x32_d7_4b);
    aliases[0].information_update_count = 2;
    aliases[0].analyzed_at = "2026-08-14".to_owned();
    repository.sync_device_aliases(
        "usb-fs:gray",
        "DJ VIC GRAY",
        "database-newer",
        &mut aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert_eq!(
        repository
            .track(track_id)?
            .and_then(|track| track.summary().color())
            .map(TrackColor::rgb_u32),
        Some(0x32_d7_4b),
        "a monotone information revision must promote its Rekordbox color"
    );
    assert_eq!(
        repository.device_hot_cue_decision(track_id, "usb-fs:gray", "analysis-v1", "2026-08-13",)?,
        DeviceAnalysisDecision::Current
    );
    Ok(())
}

#[test]
fn device_sync_backfills_a_missing_color_for_the_same_metadata_revision()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let source_track = &baseline.tracks()[0];
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()[0]
        .id();
    let mut aliases = vec![DeviceAliasUpsert {
        device_track_id: 1_256,
        simulator_signature: 0,
        canonical_track_id: Some(track_id),
        match_kind: "metadata+file-size".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-v1".to_owned(),
        color_rgb: None,
        master_database_id: 1,
        master_content_id: 1_256,
        information_update_count: 1,
        analysis_revision: "analysis-v1".to_owned(),
        audio_signature: "audio:test:1256".to_owned(),
        analyzed_at: "2026-08-24".to_owned(),
        sync_disposition: "current".to_owned(),
    }];
    repository.sync_device_aliases(
        "usb-fs:gray",
        "DJ VIC GRAY",
        "database-v1",
        &mut aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert_eq!(
        repository
            .track(track_id)?
            .and_then(|track| track.summary().color()),
        None
    );

    // The authoritative metadata counter and revision are unchanged. The
    // only difference is that the current reader can finally resolve the
    // custom-labelled Rekordbox palette entry to its fixed RGB value.
    aliases[0].color_rgb = Some(0x32_80_ff);
    repository.sync_device_aliases(
        "usb-fs:gray",
        "DJ VIC GRAY",
        "database-v1",
        &mut aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert_eq!(
        repository
            .track(track_id)?
            .and_then(|track| track.summary().color())
            .map(TrackColor::rgb_u32),
        Some(0x32_80_ff)
    );
    Ok(())
}

#[test]
fn keeping_a_review_is_exact_revision_scoped_and_stale_safe()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let source_track = &baseline.tracks()[0];
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()[0]
        .id();
    let active_revision = repository
        .track(track_id)?
        .ok_or("stored track missing")?
        .summary()
        .source_revision()
        .as_str()
        .to_owned();
    let mut aliases = vec![DeviceAliasUpsert {
        device_track_id: 7,
        simulator_signature: 0,
        canonical_track_id: Some(track_id),
        match_kind: "metadata-exact".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-review".to_owned(),
        color_rgb: source_track.color().map(TrackColor::rgb_u32),
        master_database_id: 1,
        master_content_id: 7,
        information_update_count: 1,
        analysis_revision: "analysis-review".to_owned(),
        audio_signature: "audio:test:7".to_owned(),
        analyzed_at: "2026-08-23".to_owned(),
        sync_disposition: "held-conflict".to_owned(),
    }];
    repository.sync_device_aliases(
        "usb-fs:review",
        "Review USB",
        "database-review",
        &mut aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert!(matches!(
        repository.keep_active_device_analysis(
            "usb-fs:review",
            7,
            "analysis-review",
            "stale-active",
        ),
        Err(SqliteLibraryError::DeviceReviewChanged)
    ));
    repository.keep_active_device_analysis(
        "usb-fs:review",
        7,
        "analysis-review",
        &active_revision,
    )?;
    assert!(repository.device_review_tracks()?.is_empty());
    assert_eq!(repository.device_source_summaries()?[0].current_tracks, 1);
    Ok(())
}

#[test]
fn promoting_a_review_replaces_the_exact_source_projection_atomically()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let source_track = &baseline.tracks()[0];
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()[0]
        .id();
    let active_revision = repository
        .track(track_id)?
        .ok_or("stored track missing")?
        .summary()
        .source_revision()
        .as_str()
        .to_owned();
    let mut aliases = vec![DeviceAliasUpsert {
        device_track_id: 8,
        simulator_signature: 0,
        canonical_track_id: Some(track_id),
        match_kind: "metadata-exact".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-review".to_owned(),
        color_rgb: source_track.color().map(TrackColor::rgb_u32),
        master_database_id: 1,
        master_content_id: 8,
        information_update_count: 1,
        analysis_revision: "analysis-review".to_owned(),
        audio_signature: "audio:test:8".to_owned(),
        analyzed_at: "2026-08-23".to_owned(),
        sync_disposition: "held-conflict".to_owned(),
    }];
    repository.sync_device_aliases(
        "usb-fs:review",
        "Review USB",
        "database-review",
        &mut aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    let reviewed_phrases = vec![RawPhraseObservation::try_new(
        0,
        source_track.beat_grid().total_beats(),
        "Reviewed USB phrase",
    )?];
    repository.promote_reviewed_device_analysis(
        &DeviceAnalysisUpsert {
            track_id,
            source_id: "usb-fs:review".to_owned(),
            device_track_id: 8,
            analysis_revision: "device:review:analysis-review".to_owned(),
            source_analysis_revision: "analysis-review".to_owned(),
            analyzed_at: "2026-08-23".to_owned(),
            duration_millis: source_track.duration_millis() + 9,
            beat_grid: source_track.beat_grid().clone(),
            waveform: source_track.waveform().to_vec(),
            raw_phrases: reviewed_phrases.clone(),
            hot_cues: source_track.hot_cues().to_vec(),
        },
        &active_revision,
    )?;
    let promoted = repository
        .track(track_id)?
        .ok_or("promoted track missing")?;
    assert_eq!(
        promoted.summary().source_revision().as_str(),
        "device:review:analysis-review"
    );
    assert_eq!(promoted.raw_phrases(), reviewed_phrases);
    let promoted_workflow = repository
        .track_workflow_states(&[track_id])?
        .remove(&track_id)
        .ok_or("promoted workflow state missing")?;
    let promoted_attention = promoted_workflow
        .attention()
        .ok_or("promoted USB analysis must require a workflow review")?;
    assert_eq!(promoted_attention.source_id(), "usb-fs:review");
    assert!(
        promoted_attention
            .reasons()
            .contains(&TrackAttentionReason::SourcePhrasesChanged)
    );
    assert!(repository.device_review_tracks()?.is_empty());
    assert_eq!(repository.device_source_summaries()?[0].promoted_tracks, 1);

    let mut second_source = vec![DeviceAliasUpsert {
        device_track_id: 9,
        simulator_signature: 0,
        canonical_track_id: Some(track_id),
        match_kind: "metadata-exact".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Second/Track.mp3".to_owned(),
        metadata_revision: "metadata-second".to_owned(),
        color_rgb: source_track.color().map(TrackColor::rgb_u32),
        master_database_id: 2,
        master_content_id: 9,
        information_update_count: 1,
        analysis_revision: "analysis-second".to_owned(),
        audio_signature: "audio:test:9".to_owned(),
        analyzed_at: "2026-08-23".to_owned(),
        sync_disposition: "held-conflict".to_owned(),
    }];
    repository.sync_device_aliases(
        "usb-fs:second",
        "Second USB",
        "database-second",
        &mut second_source,
        &[],
        &[],
        &[],
        &[],
    )?;
    let reviews = repository.device_review_tracks()?;
    let review = &reviews["usb-fs:second"][0];
    assert_eq!(
        review.active_analysis_revision.as_deref(),
        Some("device:review:analysis-review"),
        "the UI action must carry the active canonical projection revision"
    );
    repository.keep_active_device_analysis(
        "usb-fs:second",
        9,
        "analysis-second",
        review
            .active_analysis_revision
            .as_deref()
            .ok_or("active review revision missing")?,
    )?;
    assert!(repository.device_review_tracks()?.is_empty());
    Ok(())
}

#[test]
fn device_alias_resolves_real_and_simulated_identity_and_archives_on_resync()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let source_track = &baseline.tracks()[0];
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == source_track.source_track_id())
        .ok_or("stored source track not found")?
        .id();
    let mut aliases = vec![DeviceAliasUpsert {
        device_track_id: 1_256,
        simulator_signature: 3_456_789_012,
        audio_signature: "audio:test:1256".to_owned(),
        canonical_track_id: Some(track_id),
        match_kind: "metadata+file-size".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-v1".to_owned(),
        color_rgb: Some(0x32_80_ff),
        master_database_id: 1,
        master_content_id: 1_256,
        information_update_count: 1,
        analysis_revision: "analysis-v1".to_owned(),
        analyzed_at: "2026-08-10".to_owned(),
        sync_disposition: "promoted-initial".to_owned(),
    }];
    let refreshed_duration = source_track.duration_millis() + 7;
    let refreshed_raw_phrases = vec![RawPhraseObservation::try_new(
        0,
        source_track.beat_grid().total_beats(),
        "Refreshed source phrase",
    )?];
    let analyses = vec![DeviceAnalysisUpsert {
        track_id,
        source_id: "rekordbox-device:test".to_owned(),
        device_track_id: 1_256,
        analysis_revision: "device:test:analysis-v1".to_owned(),
        source_analysis_revision: "analysis-v1".to_owned(),
        analyzed_at: "2026-08-10".to_owned(),
        duration_millis: refreshed_duration,
        beat_grid: source_track.beat_grid().clone(),
        waveform: source_track.waveform().to_vec(),
        raw_phrases: refreshed_raw_phrases.clone(),
        hot_cues: source_track.hot_cues().to_vec(),
    }];
    repository.sync_device_aliases(
        "rekordbox-device:test",
        "Test USB",
        "database-v1",
        &mut aliases,
        &[],
        &analyses,
        &[],
        &[DevicePlaylistUpsert {
            device_playlist_id: 77,
            path: "Sets/90s Dance/90s Club".to_owned(),
            device_track_ids: vec![1_256],
        }],
    )?;

    let refreshed = repository
        .track(track_id)?
        .ok_or("refreshed track missing")?;
    assert_eq!(refreshed.summary().duration_millis(), refreshed_duration);
    assert_eq!(refreshed.raw_phrases(), refreshed_raw_phrases);

    let device_playlist = repository
        .page_playlists(TrackPageRequest::try_new(0, 25)?)?
        .playlists()
        .iter()
        .find(|playlist| playlist.source_playlist_id().as_str() == "onelibrary:77")
        .cloned()
        .ok_or("device playlist not persisted")?;
    assert_eq!(device_playlist.name(), "Sets/90s Dance/90s Club");
    assert_eq!(device_playlist.track_count(), 1);

    assert_eq!(
        repository
            .resolve_device_alias(1_256, 0)?
            .ok_or("real device alias not resolved")?
            .canonical_track_id,
        track_id
    );
    assert_eq!(
        repository
            .resolve_device_alias(42, 3_456_789_012)?
            .ok_or("simulator alias not resolved")?
            .canonical_track_id,
        track_id
    );
    let summary = repository.device_source_summaries()?;
    assert_eq!(summary[0].active_tracks, 1);
    assert_eq!(summary[0].matched_tracks, 1);
    let relations = repository.device_track_source_relations(&[track_id])?;
    assert_eq!(relations[&track_id].len(), 1);
    assert_eq!(relations[&track_id][0].display_name, "Test USB");
    assert_eq!(relations[&track_id][0].sync_disposition, "promoted-initial");
    assert_eq!(
        repository.device_analysis_decision(
            track_id,
            "rekordbox-device:test",
            "analysis-v1",
            "2026-08-10",
        )?,
        DeviceAnalysisDecision::Current
    );
    assert_eq!(
        repository.device_analysis_decision(
            track_id,
            "rekordbox-device:backup",
            "analysis-older",
            "2026-08-09",
        )?,
        DeviceAnalysisDecision::ProtectOlder
    );
    assert_eq!(
        repository.device_analysis_decision(
            track_id,
            "rekordbox-device:test",
            "analysis-v2-content-change",
            "2026-08-10",
        )?,
        DeviceAnalysisDecision::PromoteNewer
    );
    assert_eq!(
        repository.device_analysis_decision(
            track_id,
            "rekordbox-device:primary",
            "analysis-newer",
            "2026-08-11",
        )?,
        DeviceAnalysisDecision::PromoteNewer
    );
    assert_eq!(
        repository.device_analysis_decision(
            track_id,
            "rekordbox-device:backup",
            "analysis-same-day-conflict",
            "2026-08-10",
        )?,
        DeviceAnalysisDecision::HoldConflict
    );

    repository.sync_device_aliases(
        "rekordbox-device:test",
        "Test USB",
        "database-v2",
        &mut [],
        &[],
        &[],
        &[],
        &[],
    )?;
    assert!(repository.resolve_device_alias(1_256, 0)?.is_none());
    assert!(
        repository
            .resolve_device_alias(42, 3_456_789_012)?
            .is_none()
    );
    assert!(
        repository
            .device_track_source_relations(&[track_id])?
            .is_empty()
    );
    Ok(())
}

#[test]
fn device_sync_imports_new_tracks_and_playlist_membership_atomically()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let source_track = &baseline.tracks()[0];
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let before = repository
        .page_tracks(TrackPageRequest::try_new(0, 100)?)?
        .total();
    let imported_analysis = ImportedTrackAnalysis::try_new(
        lumi_library::SourceTrackId::try_new("onelibrary:9001")?,
        SourceRevision::try_new("device:usb-fs:test:analysis-new")?,
        "A New USB Track",
        "USB Artist",
        source_track.bpm_milli(),
        source_track.musical_key(),
        source_track.duration_millis(),
        source_track.color(),
        "file://localhost/Volumes/Test/A%20New%20USB%20Track.wav",
        source_track.beat_grid().clone(),
        source_track.waveform().to_vec(),
        source_track.raw_phrases().to_vec(),
    )?;
    let mut aliases = vec![DeviceAliasUpsert {
        device_track_id: 9_001,
        simulator_signature: 123,
        audio_signature: "audio:test:9001".to_owned(),
        canonical_track_id: None,
        match_kind: "unmatched".to_owned(),
        title: "A New USB Track".to_owned(),
        artist: "USB Artist".to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 321,
        audio_uri: "file://localhost/Volumes/Test/A%20New%20USB%20Track.wav".to_owned(),
        metadata_revision: "metadata-new".to_owned(),
        color_rgb: Some(0xff_33_33),
        master_database_id: 1,
        master_content_id: 9_001,
        information_update_count: 1,
        analysis_revision: "analysis-new".to_owned(),
        analyzed_at: "2026-08-10".to_owned(),
        sync_disposition: "unmatched".to_owned(),
    }];
    repository.sync_device_aliases(
        "usb-fs:test",
        "Test USB",
        "database-new",
        &mut aliases,
        &[DeviceTrackImport {
            device_track_id: 9_001,
            source_analysis_revision: "analysis-new".to_owned(),
            analyzed_at: "2026-08-10".to_owned(),
            analysis: imported_analysis,
        }],
        &[],
        &[],
        &[DevicePlaylistUpsert {
            device_playlist_id: 77,
            path: "MainStage 140+".to_owned(),
            device_track_ids: vec![9_001],
        }],
    )?;

    let resolution = repository
        .resolve_device_alias(9_001, 0)?
        .ok_or("new USB track did not resolve")?;
    assert_eq!(
        aliases[0].canonical_track_id,
        Some(resolution.canonical_track_id)
    );
    assert_eq!(aliases[0].match_kind, "imported-device");
    assert_eq!(aliases[0].sync_disposition, "promoted-initial");
    assert_eq!(
        repository.track_workflow_summary()?.changed_after_usb_sync,
        0,
        "an initial USB import is preparation work, not a source-change review"
    );
    assert_eq!(
        repository
            .page_tracks(TrackPageRequest::try_new(0, 100)?)?
            .total(),
        before + 1
    );
    let playlist = repository
        .page_playlists(TrackPageRequest::try_new(0, 100)?)?
        .playlists()
        .iter()
        .find(|playlist| playlist.source_playlist_id().as_str() == "onelibrary:77")
        .cloned()
        .ok_or("imported USB playlist missing")?;
    assert_eq!(playlist.track_count(), 1);
    assert_eq!(
        repository
            .page_playlist_tracks(playlist.id(), TrackPageRequest::try_new(0, 100)?)?
            .tracks()[0]
            .title(),
        "A New USB Track"
    );

    let canonical_track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 100)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == source_track.source_track_id())
        .ok_or("provider canonical track missing")?
        .id();
    aliases[0].canonical_track_id = Some(canonical_track_id);
    aliases[0].match_kind = "metadata-canonical-repair".to_owned();
    aliases[0].sync_disposition = "current".to_owned();
    repository.sync_device_aliases(
        "usb-fs:test",
        "Test USB",
        "database-repaired",
        &mut aliases,
        &[],
        &[],
        &[],
        &[DevicePlaylistUpsert {
            device_playlist_id: 77,
            path: "MainStage 140+".to_owned(),
            device_track_ids: vec![9_001],
        }],
    )?;
    assert_eq!(
        repository
            .page_tracks(TrackPageRequest::try_new(0, 100)?)?
            .total(),
        before
    );
    let repaired_playlist = repository
        .stored_device_playlists()?
        .remove("usb-fs:test")
        .and_then(|mut playlists| playlists.pop())
        .ok_or("stored device playlist missing")?;
    assert_eq!(repaired_playlist.track_count, 1);
    assert_eq!(
        repository
            .page_playlist_tracks(
                repaired_playlist.playlist_id,
                TrackPageRequest::try_new(0, 100)?,
            )?
            .tracks()[0]
            .id(),
        canonical_track_id
    );
    Ok(())
}

#[test]
fn identical_playlists_from_two_usb_sources_are_presented_once()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let source_tracks = &baseline.tracks()[..2];
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_page = repository.page_tracks(TrackPageRequest::try_new(0, 25)?)?;
    let canonical_track_ids = source_tracks
        .iter()
        .map(|source_track| {
            track_page
                .tracks()
                .iter()
                .find(|track| track.source_track_id() == source_track.source_track_id())
                .map(lumi_library::TrackSummary::id)
                .ok_or("canonical track missing")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let aliases = source_tracks
        .iter()
        .zip(&canonical_track_ids)
        .enumerate()
        .map(
            |(offset, (source_track, canonical_track_id))| -> Result<_, std::num::TryFromIntError> {
                Ok(DeviceAliasUpsert {
                    device_track_id: 90 + u32::try_from(offset)?,
                    simulator_signature: 0,
                    audio_signature: format!("audio:shared:{}", 90 + offset),
                    canonical_track_id: Some(*canonical_track_id),
                    match_kind: "metadata+file-size".to_owned(),
                    title: source_track.title().to_owned(),
                    artist: source_track.artist().to_owned(),
                    bpm_milli: source_track.bpm_milli(),
                    duration_millis: source_track.duration_millis(),
                    file_size: 123,
                    audio_uri: format!("file://localhost/Volumes/Test/Track-{offset}.mp3"),
                    metadata_revision: "metadata-v1".to_owned(),
                    color_rgb: None,
                    master_database_id: 1,
                    master_content_id: 90 + u32::try_from(offset)?,
                    information_update_count: 1,
                    analysis_revision: "analysis-v1".to_owned(),
                    analyzed_at: "2026-08-13".to_owned(),
                    sync_disposition: "current".to_owned(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let playlist = DevicePlaylistUpsert {
        device_playlist_id: 77,
        path: "Genre 5 Stars/MainStage 140+".to_owned(),
        device_track_ids: vec![90, 91],
    };
    for (source_id, display_name) in [
        ("usb-fs:chrm", "DJ VIC CHRM"),
        ("usb-fs:gray", "DJ VIC GRAY"),
    ] {
        repository.sync_device_aliases(
            source_id,
            display_name,
            "database-v1",
            &mut aliases.clone(),
            &[],
            &[],
            &[],
            std::slice::from_ref(&playlist),
        )?;
    }

    for (device_track_id, canonical_track_id) in [90_u32, 91].into_iter().zip(&canonical_track_ids)
    {
        assert_eq!(
            repository
                .resolve_device_alias(device_track_id, 0)?
                .ok_or("shared backup USB alias did not resolve")?
                .canonical_track_id,
            *canonical_track_id
        );
    }

    let page = repository.page_playlists(TrackPageRequest::try_new(0, 25)?)?;
    let matching = page
        .playlists()
        .iter()
        .filter(|playlist| playlist.name() == "Genre 5 Stars/MainStage 140+")
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].track_count(), 2);
    let tracks =
        repository.page_playlist_tracks(matching[0].id(), TrackPageRequest::try_new(0, 25)?)?;
    assert_eq!(tracks.total(), 2);
    assert_eq!(
        tracks
            .tracks()
            .iter()
            .map(lumi_library::TrackSummary::id)
            .collect::<Vec<_>>(),
        canonical_track_ids
    );
    let relations = repository.device_track_source_relations(&canonical_track_ids)?;
    assert!(
        canonical_track_ids
            .iter()
            .all(|track_id| relations[track_id].len() == 2)
    );
    Ok(())
}

#[test]
fn conflicting_usb_aliases_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let canonical = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .take(2)
        .map(lumi_library::TrackSummary::id)
        .collect::<Vec<_>>();
    assert_eq!(canonical.len(), 2);

    for (source_id, canonical_track_id) in ["usb-fs:gray", "usb-fs:chrm"].into_iter().zip(canonical)
    {
        let mut aliases = vec![DeviceAliasUpsert {
            device_track_id: 1_256,
            simulator_signature: 0,
            audio_signature: "audio:ambiguous:1256".to_owned(),
            canonical_track_id: Some(canonical_track_id),
            match_kind: "metadata+file-size".to_owned(),
            title: "Ambiguous".to_owned(),
            artist: "Test".to_owned(),
            bpm_milli: 140_000,
            duration_millis: 180_000,
            file_size: 123,
            audio_uri: "file://localhost/Volumes/Test/Ambiguous.mp3".to_owned(),
            metadata_revision: "metadata-v1".to_owned(),
            color_rgb: None,
            master_database_id: 1,
            master_content_id: 1_256,
            information_update_count: 1,
            analysis_revision: "analysis-v1".to_owned(),
            analyzed_at: "2026-08-16".to_owned(),
            sync_disposition: "current".to_owned(),
        }];
        repository.sync_device_aliases(
            source_id,
            source_id,
            "database-v1",
            &mut aliases,
            &[],
            &[],
            &[],
            &[],
        )?;
    }

    assert!(repository.resolve_device_alias(1_256, 0)?.is_none());
    Ok(())
}

#[test]
fn stable_filesystem_identity_replaces_ephemeral_mount_records_atomically()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let source_track = &baseline.tracks()[0];
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == source_track.source_track_id())
        .ok_or("stored source track not found")?
        .id();
    let alias = DeviceAliasUpsert {
        device_track_id: 88,
        simulator_signature: 99,
        audio_signature: "audio:test:88".to_owned(),
        canonical_track_id: Some(track_id),
        match_kind: "metadata+file-size".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-v1".to_owned(),
        color_rgb: None,
        master_database_id: 1,
        master_content_id: 88,
        information_update_count: 1,
        analysis_revision: "analysis-v1".to_owned(),
        analyzed_at: "2026-08-10".to_owned(),
        sync_disposition: "current".to_owned(),
    };
    let playlist = DevicePlaylistUpsert {
        device_playlist_id: 24,
        path: "Genre 5 Stars/MainStage 140+".to_owned(),
        device_track_ids: vec![88],
    };
    let mut legacy_aliases = vec![alias.clone()];
    repository.sync_device_aliases(
        "usb-volume:{legacy-mount}",
        "DJ VIC CHRM",
        "database-v1",
        &mut legacy_aliases,
        &[],
        &[],
        &[],
        std::slice::from_ref(&playlist),
    )?;
    let mut stable_aliases = vec![alias];
    repository.sync_device_aliases(
        "usb-fs:stable-volume-uuid",
        "DJ VIC CHRM",
        "database-v1",
        &mut stable_aliases,
        &[],
        &[],
        &[],
        &[playlist],
    )?;

    let sources = repository.device_source_summaries()?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "usb-fs:stable-volume-uuid");
    let relation = repository.device_track_source_relations(&[track_id])?;
    assert_eq!(relation[&track_id].len(), 1);
    assert_eq!(
        relation[&track_id][0].source_id,
        "usb-fs:stable-volume-uuid"
    );
    let playlists = repository.page_playlists(TrackPageRequest::try_new(0, 25)?)?;
    assert_eq!(
        playlists
            .playlists()
            .iter()
            .filter(|value| value.source_playlist_id().as_str() == "onelibrary:24")
            .count(),
        1
    );
    Ok(())
}

#[test]
fn changed_filesystem_uuid_consolidates_only_an_equivalent_named_usb()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_ids = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .take(2)
        .map(lumi_library::TrackSummary::id)
        .collect::<Vec<_>>();
    let source_track = &baseline.tracks()[0];
    let alias_for = |track_id: TrackId, audio_uri: &str| DeviceAliasUpsert {
        device_track_id: 88,
        simulator_signature: 99,
        audio_signature: "audio:test:88".to_owned(),
        canonical_track_id: Some(track_id),
        match_kind: "metadata+file-size".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: audio_uri.to_owned(),
        metadata_revision: "metadata-v1".to_owned(),
        color_rgb: None,
        master_database_id: 1,
        master_content_id: 88,
        information_update_count: 1,
        analysis_revision: "analysis-v1".to_owned(),
        analyzed_at: "2026-08-22".to_owned(),
        sync_disposition: "current".to_owned(),
    };

    let mut old_aliases = vec![alias_for(
        track_ids[0],
        "file://localhost/Volumes/Old%20Gray/Track.mp3",
    )];
    repository.sync_device_aliases(
        "usb-fs:old-gray-uuid",
        "DJ VIC GRAY",
        "database-old",
        &mut old_aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    let mut equivalent_aliases = vec![alias_for(
        track_ids[0],
        "file://localhost/Volumes/DJ%20VIC%20GRAY/Track.mp3",
    )];
    repository.sync_device_aliases(
        "usb-fs:new-gray-uuid",
        "DJ VIC GRAY",
        "database-new",
        &mut equivalent_aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert_eq!(repository.device_source_summaries()?.len(), 1);
    assert_eq!(
        repository.device_audio_uris(track_ids[0])?,
        vec!["file://localhost/Volumes/DJ%20VIC%20GRAY/Track.mp3"]
    );

    let mut different_aliases = vec![alias_for(
        track_ids[1],
        "file://localhost/Volumes/Other%20Gray/Track.mp3",
    )];
    repository.sync_device_aliases(
        "usb-fs:different-gray-uuid",
        "DJ VIC GRAY",
        "database-different",
        &mut different_aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    assert_eq!(repository.device_source_summaries()?.len(), 2);
    Ok(())
}

#[test]
fn stable_filesystem_identity_replaces_reset_pending_provider_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let source_track = &baseline.tracks()[0];
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == source_track.source_track_id())
        .ok_or("stored source track not found")?
        .id();
    let alias = DeviceAliasUpsert {
        device_track_id: 1_256,
        simulator_signature: 99,
        audio_signature: "audio:test:1256".to_owned(),
        canonical_track_id: Some(track_id),
        match_kind: "metadata+file-size".to_owned(),
        title: source_track.title().to_owned(),
        artist: source_track.artist().to_owned(),
        bpm_milli: source_track.bpm_milli(),
        duration_millis: source_track.duration_millis(),
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".to_owned(),
        metadata_revision: "metadata-v1".to_owned(),
        color_rgb: None,
        master_database_id: 1,
        master_content_id: 1_256,
        information_update_count: 1,
        analysis_revision: "analysis-v1".to_owned(),
        analyzed_at: "2026-08-10".to_owned(),
        sync_disposition: "current".to_owned(),
    };
    let mut legacy_aliases = vec![alias.clone()];
    repository.sync_device_aliases(
        "rekordbox-device:dj-vic-gray",
        "DJ VIC GRAY",
        "database-before-reset",
        &mut legacy_aliases,
        &[],
        &[],
        &[],
        &[],
    )?;
    repository.connection.execute(
        "UPDATE device_library_sources
            SET database_revision = 'reset-pending'
          WHERE source_id = 'rekordbox-device:dj-vic-gray'",
        [],
    )?;

    let mut stable_aliases = vec![alias];
    repository.sync_device_aliases(
        "usb-fs:stable-gray-volume-uuid",
        "DJ VIC GRAY",
        "database-after-reset",
        &mut stable_aliases,
        &[],
        &[],
        &[],
        &[],
    )?;

    let sources = repository.device_source_summaries()?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "usb-fs:stable-gray-volume-uuid");
    let relations = repository.device_track_source_relations(&[track_id])?;
    assert_eq!(relations[&track_id].len(), 1);
    assert_eq!(relations[&track_id][0].display_name, "DJ VIC GRAY");
    assert_eq!(
        repository
            .resolve_device_alias(1_256, 0)?
            .ok_or("stable GRAY alias did not resolve")?
            .source_id,
        "usb-fs:stable-gray-volume-uuid"
    );
    Ok(())
}

#[test]
fn light_planning_policy_is_revisioned_and_persistent() -> Result<(), Box<dyn std::error::Error>> {
    let mut repository = SqliteLibraryRepository::in_memory()?;
    let initial = repository.light_planning_policy()?;
    assert_eq!(initial.revision, 1);
    let mut replacement = initial.clone();
    replacement.theme_cooldown_tracks = 2;
    replacement.bank_organization = BankOrganization::Themes;
    replacement.default_theme_id = Some(2);
    replacement.automatic_mid_track_theme_changes = false;
    replacement.theme_rules = vec![
        ThemeRule {
            theme_id: 1,
            enabled: true,
            selection_weight: 2,
            color_behavior: ColorBehavior::Neutral,
            color_rgb: Vec::new(),
        },
        ThemeRule {
            theme_id: 2,
            enabled: true,
            selection_weight: 4,
            color_behavior: ColorBehavior::Prefer,
            color_rgb: vec![0xff_00_00],
        },
    ];
    let stored = repository.replace_light_planning_policy(1, replacement)?;
    assert_eq!(stored.revision, 2);
    assert_eq!(stored.default_theme_id, Some(2));
    assert_eq!(stored.theme_rules.len(), 2);
    assert_eq!(repository.light_planning_policy()?, stored);
    assert!(matches!(
        repository.replace_light_planning_policy(1, initial),
        Err(SqliteLibraryError::LightPlanningRevisionConflict {
            expected: 1,
            actual: 2
        })
    ));
    let schema: u32 = repository
        .connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(schema, 18);
    Ok(())
}
