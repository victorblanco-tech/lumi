use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use lumi_domain::ThemeId;
use lumi_library::{
    AUTOLOOP_CATALOG_DEFAULTS_VERSION, AutoloopCatalog, AutoloopEntryId, AutoloopMatrixCell,
    AutoloopTheme, AutoloopVariant, ImportedLibraryBaseline, ImportedTrackAnalysis,
    LibraryRepository, LibraryTrackQuery, LumiPhraseTimeline, PHRASE_ROLE_DEFAULTS_VERSION,
    PhraseInstance, PhraseLoopStrategy, PhraseRole, PhraseRoleCatalog, PhraseRoleId,
    PhraseRoleMove, ReconcileStrategy, SourcePhraseMapping, SourceRevision, TimelineEditCommand,
    TimelineRevision, TimelineRevisionOrigin, TimelineRevisionReason, TrackPageRequest, VariantId,
    reconcile_timeline,
};
use lumi_library_demo::{DemoLibraryRevision, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider;
use lumi_library_sqlite::{SqliteLibraryError, SqliteLibraryRepository};
use rusqlite::Connection;

#[test]
fn migrates_an_empty_database() -> Result<(), Box<dyn Error>> {
    let repository = SqliteLibraryRepository::in_memory()?;
    assert_eq!(repository.schema_version()?, 5);
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
            CREATE TABLE beat_grids (
                track_id INTEGER PRIMARY KEY,
                beats_per_bar INTEGER NOT NULL
            );
            INSERT INTO beat_grids VALUES (1, 4);
            INSERT INTO timeline_revisions VALUES (1, 1, 'v1', 8, 'source-import');
            INSERT INTO phrase_instances VALUES (1, 1, 0, 0, 8, 'intro');
            PRAGMA user_version = 1;
            ",
        )?;
    }

    let repository = SqliteLibraryRepository::open(&path)?;
    assert_eq!(repository.schema_version()?, 5);
    drop(repository);
    let connection = Connection::open(&path)?;
    let reason: String = connection.query_row(
        "SELECT reason FROM timeline_revisions WHERE track_id = 1 AND revision = 1",
        [],
        |row| row.get(0),
    )?;
    let loop_strategy: String = connection.query_row(
        "SELECT loop_strategy FROM phrase_points WHERE track_id = 1 AND revision = 1",
        [],
        |row| row.get(0),
    )?;
    let total_beats: i64 = connection.query_row(
        "SELECT total_beats FROM timeline_revisions WHERE track_id = 1 AND revision = 1",
        [],
        |row| row.get(0),
    )?;
    let point_beat: i64 = connection.query_row(
        "SELECT beat FROM phrase_points WHERE track_id = 1 AND revision = 1",
        [],
        |row| row.get(0),
    )?;
    let mut point_column_statement = connection.prepare("PRAGMA table_info(phrase_points)")?;
    let point_columns = point_column_statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(reason, "initial-source-mapping");
    assert_eq!(loop_strategy, "auto");
    assert_eq!(total_beats, 32);
    assert_eq!(point_beat, 0);
    assert!(point_columns.contains(&"beat".to_owned()));
    assert!(!point_columns.iter().any(|column| column.contains("end")));
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn migrates_version_two_phrase_roles_into_an_unseeded_catalog() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    {
        let connection = Connection::open(&path)?;
        connection.execute_batch(
            "
            CREATE TABLE phrase_roles (
                role_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                sort_order INTEGER NOT NULL,
                archived INTEGER NOT NULL
            );
            INSERT INTO phrase_roles VALUES ('synth', 'Synth', 1, 0);
            PRAGMA user_version = 2;
            ",
        )?;
    }

    let repository = SqliteLibraryRepository::open(&path)?;
    assert_eq!(repository.schema_version()?, 5);
    let catalog = repository.phrase_role_catalog()?;
    assert_eq!(catalog.revision(), 0);
    assert_eq!(catalog.defaults_version(), 0);
    assert_eq!(catalog.roles()[0].id().as_str(), "synth");
    drop(repository);
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn migrates_version_three_into_an_unseeded_autoloop_catalog() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    {
        let connection = Connection::open(&path)?;
        connection.execute_batch(
            "
            CREATE TABLE phrase_roles (
                role_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                sort_order INTEGER NOT NULL,
                archived INTEGER NOT NULL
            );
            CREATE TABLE library_settings (
                key TEXT PRIMARY KEY,
                value INTEGER NOT NULL
            );
            INSERT INTO library_settings(key, value)
                VALUES ('phrase-role-defaults-version', 1),
                       ('phrase-role-catalog-revision', 1);
            CREATE TABLE source_phrase_mappings (
                provider_kind TEXT NOT NULL,
                normalized_label TEXT NOT NULL,
                raw_label TEXT NOT NULL,
                role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
                PRIMARY KEY(provider_kind, normalized_label)
            );
            PRAGMA user_version = 3;
            ",
        )?;
    }

    let repository = SqliteLibraryRepository::open(&path)?;
    assert_eq!(repository.schema_version()?, 5);
    let catalog = repository.autoloop_catalog()?;
    assert_eq!(catalog.revision(), 0);
    assert_eq!(catalog.defaults_version(), 0);
    assert!(catalog.themes().is_empty());
    drop(repository);
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn autoloop_catalog_mutation_and_conflict_survive_restart() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    {
        let mut repository = SqliteLibraryRepository::open(&path)?;
        let roles = PhraseRoleCatalog::try_new(
            1,
            PHRASE_ROLE_DEFAULTS_VERSION,
            vec![PhraseRole::try_new(
                PhraseRoleId::try_new("synth")?,
                "Synth",
                1,
                false,
            )?],
            vec![],
        )?;
        repository.initialize_phrase_role_catalog(&roles)?;
        let initial = test_autoloop_catalog()?;
        repository.initialize_autoloop_catalog(&initial)?;
        let renamed = initial.rename_theme(ThemeId::new(1), "Electric Garden")?;
        repository.replace_autoloop_catalog(&renamed, 1)?;
        let added = renamed.add_variant(PhraseRoleId::try_new("synth")?, "Variant 2")?;
        repository.replace_autoloop_catalog(&added, 2)?;
        assert!(matches!(
            repository.replace_autoloop_catalog(&added, 2),
            Err(SqliteLibraryError::AutoloopCatalogRevisionConflict {
                expected: 2,
                actual: 3,
            })
        ));
    }
    let repository = SqliteLibraryRepository::open(&path)?;
    let restarted = repository.autoloop_catalog()?;
    assert_eq!(restarted.revision(), 3);
    assert_eq!(restarted.themes()[0].display_name(), "Electric Garden");
    assert_eq!(restarted.variants().len(), 2);
    assert_eq!(restarted.cells().len(), 4);
    drop(repository);
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
fn source_reconcile_updates_analysis_and_timeline_in_one_transaction() -> Result<(), Box<dyn Error>>
{
    let first =
        DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V1).load_baseline()?;
    let second =
        DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2).load_baseline()?;
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&first)?;
    let stored = repository
        .page_tracks(TrackPageRequest::try_new(0, 200)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id().as_str() == "afterglow-drive")
        .cloned()
        .ok_or("demo track not found")?;
    let initial = source_timeline(stored.id(), &repository.track(stored.id())?.ok_or("track")?)?;
    repository.append_timeline_revision(&initial, None)?;
    let incoming = second
        .tracks()
        .iter()
        .find(|track| track.source_track_id().as_str() == "afterglow-drive")
        .ok_or("incoming demo track not found")?;
    let reconciled = reconcile_timeline(
        &initial,
        incoming.analysis_revision().clone(),
        initial.total_beats(),
        initial.phrases(),
        &ReconcileStrategy::KeepLumi,
    )?;
    repository.reconcile_track(&second, incoming, &reconciled, initial.revision())?;

    let updated = repository.track(stored.id())?.ok_or("updated track")?;
    assert_eq!(updated.summary().title(), "Afterglow Drive (Extended)");
    assert_eq!(repository.timeline_head(stored.id())?, Some(reconciled));
    assert_eq!(
        repository
            .library_source(first.source_id())?
            .ok_or("source summary before completion")?
            .revision(),
        first.source_revision()
    );
    assert!(matches!(
        repository.complete_source_refresh(&second),
        Err(SqliteLibraryError::IncompleteSourceRefresh(_))
    ));
    assert_eq!(
        repository
            .library_source(second.source_id())?
            .ok_or("source summary after rejected completion")?
            .revision(),
        first.source_revision()
    );
    Ok(())
}

#[test]
fn failed_reconcile_rolls_back_source_analysis_and_timeline_together() -> Result<(), Box<dyn Error>>
{
    let first =
        DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V1).load_baseline()?;
    let second =
        DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2).load_baseline()?;
    let path = temporary_database_path()?;
    let mut repository = SqliteLibraryRepository::open(&path)?;
    repository.import_baseline(&first)?;
    let stored = repository
        .page_tracks(TrackPageRequest::try_new(0, 200)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id().as_str() == "horizon-lines")
        .cloned()
        .ok_or("demo track not found")?;
    let imported_track = repository.track(stored.id())?.ok_or("track")?;
    let initial = source_timeline(stored.id(), &imported_track)?;
    repository.append_timeline_revision(&initial, None)?;
    let original_track = repository.track(stored.id())?.ok_or("track")?;
    let incoming = second
        .tracks()
        .iter()
        .find(|track| track.source_track_id().as_str() == "horizon-lines")
        .ok_or("incoming demo track not found")?;
    let reconciled = reconcile_timeline(
        &initial,
        incoming.analysis_revision().clone(),
        initial.total_beats(),
        initial.phrases(),
        &ReconcileStrategy::KeepLumi,
    )?;
    Connection::open(&path)?.execute_batch(
        "CREATE TRIGGER fail_reconcile_waveform BEFORE INSERT ON waveform_points
         BEGIN SELECT RAISE(ABORT, 'injected reconcile failure'); END;",
    )?;
    assert!(
        repository
            .reconcile_track(&second, incoming, &reconciled, initial.revision())
            .is_err()
    );
    assert_eq!(repository.track(stored.id())?, Some(original_track));
    assert_eq!(repository.timeline_head(stored.id())?, Some(initial));
    drop(repository);
    std::fs::remove_file(path)?;
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
        let intro = PhraseRole::try_new(PhraseRoleId::try_new("intro")?, "Intro", 1, false)?;
        let drop = PhraseRole::try_new(PhraseRoleId::try_new("drop")?, "Drop", 2, false)?;
        repository.initialize_phrase_role_catalog(&PhraseRoleCatalog::try_new(
            1,
            PHRASE_ROLE_DEFAULTS_VERSION,
            vec![intro, drop],
            vec![],
        )?)?;
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
            at_beat: 4,
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
        let catalog = repository.phrase_role_catalog()?;
        assert_eq!(catalog.roles().len(), 2);
        assert_eq!(catalog.revision(), 1);
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
fn phrase_role_catalog_mutation_usage_and_conflict_survive_restart() -> Result<(), Box<dyn Error>> {
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
        let initial = PhraseRoleCatalog::try_new(
            1,
            PHRASE_ROLE_DEFAULTS_VERSION,
            vec![
                PhraseRole::try_new(PhraseRoleId::try_new("bridge")?, "Bridge", 1, false)?,
                PhraseRole::try_new(PhraseRoleId::try_new("synth")?, "Synth", 2, false)?,
            ],
            vec![SourcePhraseMapping::try_new(
                "rekordbox7",
                "Verse",
                PhraseRoleId::try_new("bridge")?,
            )?],
        )?;
        repository.initialize_phrase_role_catalog(&initial)?;
        repository.append_timeline_revision(
            &LumiPhraseTimeline::try_new(
                track_id,
                TimelineRevision::initial(),
                SourceRevision::try_new("analysis-v1")?,
                8,
                TimelineRevisionOrigin::UserEdit,
                vec![PhraseInstance::new(
                    0,
                    0,
                    8,
                    PhraseRoleId::try_new("synth")?,
                )],
            )?,
            None,
        )?;

        let renamed = initial.rename_role(&PhraseRoleId::try_new("synth")?, "Lead Synth")?;
        repository.replace_phrase_role_catalog(&renamed, 1)?;
        let moved = renamed.move_role(&PhraseRoleId::try_new("synth")?, PhraseRoleMove::Earlier)?;
        repository.replace_phrase_role_catalog(&moved, 2)?;
        let conflict = repository.replace_phrase_role_catalog(&moved, 2);
        assert!(matches!(
            conflict,
            Err(SqliteLibraryError::PhraseRoleCatalogRevisionConflict {
                expected: 2,
                actual: 3,
            })
        ));
    }

    let repository = SqliteLibraryRepository::open(&path)?;
    let catalog = repository.phrase_role_catalog()?;
    assert_eq!(catalog.revision(), 3);
    assert_eq!(catalog.roles()[0].id().as_str(), "synth");
    assert_eq!(catalog.roles()[0].display_name(), "Lead Synth");
    let usages = repository.phrase_role_usages()?;
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].role_id().as_str(), "synth");
    assert_eq!(usages[0].phrase_count(), 1);
    assert_eq!(usages[0].tracks()[0].track_id(), track_id);
    drop(repository);
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
fn ten_thousand_track_fixture_meets_epic_two_a_budgets() -> Result<(), Box<dyn Error>> {
    const FIXTURE_BUDGET: Duration = Duration::from_secs(1);
    const IMPORT_BUDGET: Duration = Duration::from_secs(5);
    const PAGE_BUDGET: Duration = Duration::from_millis(100);
    const SEARCH_BUDGET: Duration = Duration::from_millis(250);

    let fixture_started = Instant::now();
    let baseline = DemoLibrarySourceProvider::scaled(10_000)?.load_baseline()?;
    let fixture_elapsed = fixture_started.elapsed();
    assert!(
        fixture_elapsed <= FIXTURE_BUDGET,
        "10,000-track fixture generation took {fixture_elapsed:?}, budget {FIXTURE_BUDGET:?}"
    );

    let mut repository = SqliteLibraryRepository::in_memory()?;
    let import_started = Instant::now();
    let result = repository.import_baseline(&baseline)?;
    let import_elapsed = import_started.elapsed();
    assert_eq!(result.inserted, 10_000);
    assert!(
        import_elapsed <= IMPORT_BUDGET,
        "10,000-track import took {import_elapsed:?}, budget {IMPORT_BUDGET:?}"
    );

    let page_started = Instant::now();
    let final_page = repository.page_tracks(TrackPageRequest::try_new(9_950, 50)?)?;
    assert_eq!(final_page.total(), 10_000);
    assert_eq!(final_page.offset(), 9_950);
    assert_eq!(final_page.tracks().len(), 50);
    let playlists = repository.page_playlists(TrackPageRequest::try_new(0, 1)?)?;
    let final_playlist_page = repository.page_playlist_tracks(
        playlists.playlists()[0].id(),
        TrackPageRequest::try_new(9_950, 50)?,
    )?;
    let page_elapsed = page_started.elapsed();
    assert_eq!(final_playlist_page.total(), 10_000);
    assert_eq!(final_playlist_page.tracks().len(), 50);
    assert!(
        page_elapsed <= PAGE_BUDGET,
        "last collection and playlist pages took {page_elapsed:?}, budget {PAGE_BUDGET:?}"
    );

    let search_started = Instant::now();
    let search = repository.query_tracks(&LibraryTrackQuery::try_new(
        "Demo Track 09999",
        None,
        TrackPageRequest::try_new(0, 50)?,
    )?)?;
    let search_elapsed = search_started.elapsed();
    assert_eq!(search.total(), 1);
    assert_eq!(search.tracks()[0].title(), "Demo Track 09999");
    assert!(
        search_elapsed <= SEARCH_BUDGET,
        "exact 10,000-track search took {search_elapsed:?}, budget {SEARCH_BUDGET:?}"
    );
    let literal_wildcard = repository.query_tracks(&LibraryTrackQuery::try_new(
        "%",
        None,
        TrackPageRequest::try_new(0, 50)?,
    )?)?;
    assert_eq!(literal_wildcard.total(), 0);
    eprintln!(
        "Epic 2A 10k benchmark: fixture={fixture_elapsed:?}, import={import_elapsed:?}, pages={page_elapsed:?}, search={search_elapsed:?}"
    );
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

fn test_autoloop_catalog() -> Result<AutoloopCatalog, Box<dyn Error>> {
    let themes = ["Electric Bloom", "Deep Ocean", "Solar Flare", "Ultraviolet"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            Ok(AutoloopTheme::try_new(
                ThemeId::new(u64::try_from(index + 1)?),
                name,
                u16::try_from(index + 1)?,
            )?)
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let variant = AutoloopVariant::try_new(
        PhraseRoleId::try_new("synth")?,
        VariantId::try_new("variant-1")?,
        "Variant 1",
        1,
        false,
    )?;
    let cells = themes
        .iter()
        .map(|theme| {
            Ok(AutoloopMatrixCell::try_new(
                theme.id(),
                variant.role_id().clone(),
                variant.id().clone(),
                AutoloopEntryId::try_new(format!(
                    "theme-{}--synth--variant-1",
                    theme.id().value()
                ))?,
                format!("{} · Synth · Variant 1", theme.display_name()),
            )?)
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(AutoloopCatalog::try_new(
        1,
        AUTOLOOP_CATALOG_DEFAULTS_VERSION,
        themes,
        vec![variant],
        cells,
    )?)
}

fn source_timeline(
    track_id: lumi_domain::TrackId,
    track: &lumi_library::StoredTrack,
) -> Result<LumiPhraseTimeline, Box<dyn Error>> {
    let total_beats = u32::try_from(track.beat_grid().markers().len())?;
    let phrases = track
        .raw_phrases()
        .iter()
        .enumerate()
        .map(|(index, phrase)| {
            Ok(PhraseInstance::new(
                u16::try_from(index)?,
                phrase.start_beat(),
                phrase.end_beat(),
                PhraseRoleId::try_new("source")?,
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(LumiPhraseTimeline::try_new(
        track_id,
        TimelineRevision::initial(),
        track.summary().source_revision().clone(),
        total_beats,
        TimelineRevisionOrigin::SourceImport,
        phrases,
    )?)
}

fn temporary_database_path() -> Result<PathBuf, Box<dyn Error>> {
    static NEXT_TEMPORARY_DATABASE: AtomicU64 = AtomicU64::new(1);
    let unique = NEXT_TEMPORARY_DATABASE.fetch_add(1, Ordering::Relaxed);
    Ok(std::env::temp_dir().join(format!(
        "lumi-library-{}-{unique}.sqlite3",
        std::process::id()
    )))
}
