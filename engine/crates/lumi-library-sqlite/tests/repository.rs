use std::error::Error;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use lumi_library::{
    ImportedLibraryBaseline, ImportedTrackAnalysis, LibraryRepository, LibraryTrackQuery,
    LumiPhraseTimeline, PhraseInstance, PhraseLoopStrategy, PhraseRole, PhraseRoleId,
    SourceRevision, TimelineEditCommand, TimelineRevision, TimelineRevisionOrigin,
    TimelineRevisionReason, TrackPageRequest, VariantId,
};
use lumi_library_demo::DemoLibrarySourceProvider;
use lumi_library_source::MusicLibrarySourceProvider;
use lumi_library_sqlite::{SqliteLibraryError, SqliteLibraryRepository};
use rusqlite::Connection;

#[test]
fn migrates_an_empty_database() -> Result<(), Box<dyn Error>> {
    let repository = SqliteLibraryRepository::in_memory()?;
    assert_eq!(repository.schema_version()?, 2);
    assert_eq!(
        repository
            .page_tracks(TrackPageRequest::try_new(0, 25)?)?
            .total(),
        0
    );
    Ok(())
}

#[test]
fn migrates_version_one_timeline_history_without_losing_rows() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    {
        let connection = Connection::open(&path)?;
        connection.execute_batch(
            "
            CREATE TABLE timeline_revisions (
                track_id INTEGER NOT NULL,
                revision INTEGER NOT NULL,
                baseline_revision TEXT NOT NULL,
                total_bars INTEGER NOT NULL,
                origin TEXT NOT NULL,
                PRIMARY KEY(track_id, revision)
            );
            CREATE TABLE phrase_instances (
                track_id INTEGER NOT NULL,
                revision INTEGER NOT NULL,
                phrase_index INTEGER NOT NULL,
                start_bar INTEGER NOT NULL,
                end_bar INTEGER NOT NULL,
                role_id TEXT NOT NULL,
                PRIMARY KEY(track_id, revision, phrase_index)
            );
            INSERT INTO timeline_revisions VALUES (1, 1, 'v1', 8, 'source-import');
            INSERT INTO phrase_instances VALUES (1, 1, 0, 0, 8, 'intro');
            PRAGMA user_version = 1;
            ",
        )?;
    }

    let repository = SqliteLibraryRepository::open(&path)?;
    assert_eq!(repository.schema_version()?, 2);
    drop(repository);
    let connection = Connection::open(&path)?;
    let reason: String = connection.query_row(
        "SELECT reason FROM timeline_revisions WHERE track_id = 1 AND revision = 1",
        [],
        |row| row.get(0),
    )?;
    let loop_strategy: String = connection.query_row(
        "SELECT loop_strategy FROM phrase_instances WHERE track_id = 1 AND revision = 1",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(reason, "initial-source-mapping");
    assert_eq!(loop_strategy, "auto");
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn import_is_idempotent_and_track_ids_are_stable() -> Result<(), Box<dyn Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;

    let first = repository.import_baseline(&baseline)?;
    assert_eq!(first.inserted, 3);
    assert_eq!(first.updated, 0);
    assert_eq!(first.unchanged, 0);
    let first_page = repository.page_tracks(TrackPageRequest::try_new(0, 25)?)?;
    let playlists = repository.page_playlists(TrackPageRequest::try_new(0, 25)?)?;
    assert_eq!(playlists.total(), 2);
    assert_eq!(playlists.playlists()[0].name(), "All Demo Tracks");
    assert_eq!(playlists.playlists()[0].track_count(), 3);
    let playlist_tracks = repository.page_playlist_tracks(
        playlists.playlists()[0].id(),
        TrackPageRequest::try_new(0, 25)?,
    )?;
    assert_eq!(playlist_tracks.total(), 3);
    let first_ids = first_page
        .tracks()
        .iter()
        .map(|track| track.id())
        .collect::<Vec<_>>();

    let second = repository.import_baseline(&baseline)?;
    assert_eq!(second.inserted, 0);
    assert_eq!(second.updated, 0);
    assert_eq!(second.unchanged, 3);
    let second_page = repository.page_tracks(TrackPageRequest::try_new(0, 25)?)?;
    let second_ids = second_page
        .tracks()
        .iter()
        .map(|track| track.id())
        .collect::<Vec<_>>();
    assert_eq!(first_ids, second_ids);

    let stored = repository
        .track(first_ids[0])?
        .ok_or("imported track not found")?;
    assert!(!stored.waveform().is_empty());
    assert!(!stored.beat_grid().markers().is_empty());
    assert!(!stored.raw_phrases().is_empty());
    Ok(())
}

#[test]
fn changed_source_analysis_updates_without_replacing_identity() -> Result<(), Box<dyn Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let first_track = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()[0]
        .clone();
    let source_track = baseline
        .tracks()
        .iter()
        .find(|track| track.source_track_id() == first_track.source_track_id())
        .ok_or("source track not found")?;
    let changed_track = ImportedTrackAnalysis::try_new(
        source_track.source_track_id().clone(),
        SourceRevision::try_new("analysis-v2")?,
        format!("{} (remastered)", source_track.title()),
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
    let changed_baseline = ImportedLibraryBaseline::try_new(
        baseline.source_id().clone(),
        baseline.source_kind(),
        baseline.display_name(),
        SourceRevision::try_new("demo-v2")?,
        vec![changed_track],
        Vec::new(),
    )?;

    let result = repository.import_baseline(&changed_baseline)?;
    assert_eq!(result.inserted, 0);
    assert_eq!(result.updated, 1);
    assert_eq!(result.unchanged, 0);
    let updated = repository
        .track(first_track.id())?
        .ok_or("updated track not found")?;
    assert_eq!(updated.summary().id(), first_track.id());
    assert!(updated.summary().title().ends_with("(remastered)"));
    Ok(())
}

#[test]
fn phrase_roles_and_timeline_revisions_survive_a_restart() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let track_id;
    {
        let mut repository = SqliteLibraryRepository::open(&path)?;
        repository.import_baseline(&baseline)?;
        track_id = repository
            .page_tracks(TrackPageRequest::try_new(0, 1)?)?
            .tracks()[0]
            .id();
        let intro = PhraseRole::try_new(PhraseRoleId::try_new("intro")?, "Intro", 10, false)?;
        let drop = PhraseRole::try_new(PhraseRoleId::try_new("drop")?, "Drop", 20, false)?;
        repository.save_phrase_roles(&[intro, drop])?;
        let timeline = LumiPhraseTimeline::try_new(
            track_id,
            TimelineRevision::initial(),
            SourceRevision::try_new("analysis-v1")?,
            16,
            TimelineRevisionOrigin::UserEdit,
            vec![
                PhraseInstance::new(0, 0, 8, PhraseRoleId::try_new("intro")?),
                PhraseInstance::new(1, 8, 16, PhraseRoleId::try_new("drop")?),
            ],
        )?;
        repository.append_timeline_revision(&timeline, None)?;
        let second_timeline = timeline.edit(TimelineEditCommand::Split {
            phrase_index: 0,
            at_bar: 4,
        })?;
        repository.append_timeline_revision(&second_timeline, Some(TimelineRevision::initial()))?;
    }

    {
        let repository = SqliteLibraryRepository::open(&path)?;
        assert_eq!(
            repository
                .page_tracks(TrackPageRequest::try_new(0, 25)?)?
                .total(),
            3
        );
        let playlists = repository.page_playlists(TrackPageRequest::try_new(0, 25)?)?;
        assert_eq!(playlists.total(), 2);
        assert_eq!(
            repository
                .page_playlist_tracks(
                    playlists.playlists()[0].id(),
                    TrackPageRequest::try_new(0, 25)?,
                )?
                .total(),
            3
        );
        let roles = repository.phrase_roles()?;
        assert_eq!(roles.len(), 2);
        let timeline = repository
            .timeline_head(track_id)?
            .ok_or("timeline head not found after restart")?;
        assert_eq!(timeline.revision(), TimelineRevision::try_new(2)?);
        assert_eq!(timeline.phrases().len(), 3);
        assert_eq!(timeline.reason(), TimelineRevisionReason::SplitPhrase);
        assert_eq!(
            timeline.parent_revision(),
            Some(TimelineRevision::initial())
        );
        let revisions =
            repository.timeline_revisions(track_id, TrackPageRequest::try_new(0, 25)?)?;
        assert_eq!(revisions.total(), 2);
        assert_eq!(
            revisions.revisions()[0].revision(),
            TimelineRevision::try_new(2)?
        );
        assert_eq!(
            revisions.revisions()[0].reason(),
            TimelineRevisionReason::SplitPhrase
        );
        assert_eq!(
            revisions.revisions()[1].revision(),
            TimelineRevision::initial()
        );
    }
    {
        let connection = Connection::open(&path)?;
        let stored_baselines =
            connection.query_row("SELECT COUNT(*) FROM import_baselines", [], |row| {
                row.get::<_, u32>(0)
            })?;
        assert_eq!(stored_baselines, 1);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn loop_strategy_survives_a_revision_round_trip() -> Result<(), Box<dyn Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 1)?)?
        .tracks()[0]
        .id();
    let timeline = LumiPhraseTimeline::try_new_with_history(
        track_id,
        TimelineRevision::initial(),
        SourceRevision::try_new("analysis-v1")?,
        8,
        TimelineRevisionOrigin::SourceImport,
        TimelineRevisionReason::InitialSourceMapping,
        None,
        None,
        vec![
            PhraseInstance::new(0, 0, 8, PhraseRoleId::try_new("drop")?).with_loop_strategy(
                PhraseLoopStrategy::FixedVariant(VariantId::try_new("drop-2")?),
            ),
        ],
    )?;
    repository.append_timeline_revision(&timeline, None)?;

    assert_eq!(repository.timeline_head(track_id)?, Some(timeline));
    Ok(())
}

#[test]
fn timeline_writes_use_optimistic_concurrency() -> Result<(), Box<dyn Error>> {
    let baseline = DemoLibrarySourceProvider::curated().load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&baseline)?;
    let track_id = repository
        .page_tracks(TrackPageRequest::try_new(0, 1)?)?
        .tracks()[0]
        .id();
    let timeline = LumiPhraseTimeline::try_new(
        track_id,
        TimelineRevision::initial(),
        SourceRevision::try_new("analysis-v1")?,
        8,
        TimelineRevisionOrigin::UserEdit,
        vec![PhraseInstance::new(
            0,
            0,
            8,
            PhraseRoleId::try_new("breakdown-1")?,
        )],
    )?;
    repository.append_timeline_revision(&timeline, None)?;

    let conflict = repository.append_timeline_revision(&timeline, None);
    assert!(matches!(
        conflict,
        Err(SqliteLibraryError::RevisionConflict {
            expected: None,
            actual: Some(_),
        })
    ));
    Ok(())
}

#[test]
fn ten_thousand_track_fixture_is_pageable() -> Result<(), Box<dyn Error>> {
    let baseline = DemoLibrarySourceProvider::scaled(10_000)?.load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    let result = repository.import_baseline(&baseline)?;
    assert_eq!(result.inserted, 10_000);

    let final_page = repository.page_tracks(TrackPageRequest::try_new(9_950, 50)?)?;
    assert_eq!(final_page.total(), 10_000);
    assert_eq!(final_page.offset(), 9_950);
    assert_eq!(final_page.tracks().len(), 50);
    let playlists = repository.page_playlists(TrackPageRequest::try_new(0, 1)?)?;
    let final_playlist_page = repository.page_playlist_tracks(
        playlists.playlists()[0].id(),
        TrackPageRequest::try_new(9_950, 50)?,
    )?;
    assert_eq!(final_playlist_page.total(), 10_000);
    assert_eq!(final_playlist_page.tracks().len(), 50);
    let search = repository.query_tracks(&LibraryTrackQuery::try_new(
        "Demo Track 09999",
        None,
        TrackPageRequest::try_new(0, 50)?,
    )?)?;
    assert_eq!(search.total(), 1);
    assert_eq!(search.tracks()[0].title(), "Demo Track 09999");
    let literal_wildcard = repository.query_tracks(&LibraryTrackQuery::try_new(
        "%",
        None,
        TrackPageRequest::try_new(0, 50)?,
    )?)?;
    assert_eq!(literal_wildcard.total(), 0);
    Ok(())
}

#[test]
fn failed_migration_rolls_back_every_schema_change() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    {
        let connection = Connection::open(&path)?;
        connection.execute("CREATE TABLE tracks (conflict INTEGER)", [])?;
    }

    assert!(SqliteLibraryRepository::open(&path).is_err());
    {
        let connection = Connection::open(&path)?;
        let created_by_failed_migration = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'library_sources'",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        assert_eq!(created_by_failed_migration, 0);
        let original_table = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'tracks'",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        assert_eq!(original_table, 1);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

fn temporary_database_path() -> Result<PathBuf, Box<dyn Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "lumi-library-{}-{unique}.sqlite3",
        std::process::id()
    )))
}
